//! A caller's deadline reaches the host main-thread queue: a call queued
//! behind a slow one is dropped, never run late, when its deadline passes.

use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;

use meshy_dcc_actions::{ToolDispatcher, ToolRegistry, registry::ToolMeta};
use meshy_dcc_host::{DccDispatcher, QueueDispatcher};
use meshy_dcc_http::{McpHttpConfig, McpHttpServer, host_bridge::dispatcher_to_executor_handle};
use meshy_dcc_models::ThreadAffinity;
use serde_json::{Value, json};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn queued_call_past_its_deadline_is_not_applied_and_never_runs() {
    let registry = Arc::new(ToolRegistry::new());
    let dispatcher = Arc::new(ToolDispatcher::new((*registry).clone()));
    for name in ["slow_a", "write_b"] {
        registry.register_action(ToolMeta {
            name: name.into(),
            dcc: "blender".into(),
            description: "deadline probe".into(),
            input_schema: json!({"type": "object"}),
            thread_affinity: ThreadAffinity::Main,
            ..Default::default()
        });
    }
    let a_started = Arc::new(AtomicBool::new(false));
    let started = a_started.clone();
    dispatcher.register_handler("slow_a", move |_| {
        started.store(true, Ordering::SeqCst);
        std::thread::sleep(Duration::from_secs(2));
        Ok(json!({"done": "a"}))
    });
    let b_runs = Arc::new(AtomicUsize::new(0));
    let runs = b_runs.clone();
    dispatcher.register_handler("write_b", move |_| {
        runs.fetch_add(1, Ordering::SeqCst);
        Ok(json!({"done": "b"}))
    });

    // A fake host main thread pumping the Rust dispatcher.
    let host: Arc<dyn DccDispatcher> = Arc::new(QueueDispatcher::new());
    let executor = dispatcher_to_executor_handle(host.clone(), &tokio::runtime::Handle::current());
    let stop = Arc::new(AtomicBool::new(false));
    let pump = {
        let (host, stop) = (host.clone(), stop.clone());
        std::thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                host.tick(16);
                std::thread::sleep(Duration::from_millis(5));
            }
        })
    };

    let mut config = McpHttpConfig::default();
    config.server.port = 0;
    config.gateway.gateway_port = 0;
    let handle = McpHttpServer::new(registry, config)
        .with_dispatcher(dispatcher)
        .with_executor(executor)
        .start()
        .await
        .expect("server");
    let base = format!("http://127.0.0.1:{}", handle.port);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();

    // A: no deadline, holds the main thread for 2 s.
    let call_a = tokio::spawn({
        let (client, base) = (client.clone(), base.clone());
        async move {
            client
                .post(format!("{base}/v1/call"))
                .json(&json!({"tool_slug": "blender.core.slow_a", "params": {}}))
                .send()
                .await
                .expect("A response")
        }
    });
    while !a_started.load(Ordering::SeqCst) {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    // B over REST (the gateway's route) and C over MCP, each with 500 ms.
    let b = client
        .post(format!("{base}/v1/call"))
        .json(&json!({
            "tool_slug": "blender.core.write_b",
            "params": {},
            "meta": {"meshy-dcc/timeout_ms": 500},
        }))
        .send()
        .await
        .expect("B response");
    assert_eq!(b.status().as_u16(), 503);
    let b: Value = b.json().await.unwrap();
    assert_eq!(b["kind"], "not-applied", "{b}");
    assert_eq!(b["context"]["status"], "not_applied", "{b}");

    let c: Value = client
        .post(format!("{base}/mcp"))
        .header("Accept", "application/json, text/event-stream")
        .header("MCP-Protocol-Version", "2025-06-18")
        .json(
            &json!({"jsonrpc": "2.0", "id": "c", "method": "tools/call", "params": {
                "name": "write_b", "arguments": {}, "_meta": {"meshy-dcc/timeout_ms": 500},
            }}),
        )
        .send()
        .await
        .expect("C response")
        .json()
        .await
        .unwrap();
    assert_eq!(c["result"]["isError"], true, "{c}");
    let c = &c["result"]["structuredContent"];
    assert_eq!(c["code"], "NOT_APPLIED", "{c}");
    assert_eq!(c["details"]["retryable"], true, "{c}");

    // A completes normally, and the dropped calls never run afterwards.
    let a = call_a.await.unwrap();
    assert_eq!(a.status().as_u16(), 200);
    let a: Value = a.json().await.unwrap();
    assert_eq!(a["output"]["done"], "a", "{a}");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(b_runs.load(Ordering::SeqCst), 0, "a dropped call ran late");

    // Without a deadline B runs as before (old gateways send none).
    let b: Value = client
        .post(format!("{base}/v1/call"))
        .json(&json!({"tool_slug": "blender.core.write_b", "params": {}}))
        .send()
        .await
        .expect("B response")
        .json()
        .await
        .unwrap();
    assert_eq!(b["output"]["done"], "b", "{b}");
    assert_eq!(b_runs.load(Ordering::SeqCst), 1);

    // A call still running when its deadline passes is unknown, not retryable.
    let d = client
        .post(format!("{base}/v1/call"))
        .json(&json!({
            "tool_slug": "blender.core.slow_a",
            "params": {},
            "meta": {"meshy-dcc/timeout_ms": 500},
        }))
        .send()
        .await
        .expect("D response");
    assert_eq!(d.status().as_u16(), 504);
    let d: Value = d.json().await.unwrap();
    assert_eq!(d["kind"], "outcome-unknown", "{d}");
    assert_eq!(d["context"]["status"], "unknown", "{d}");

    stop.store(true, Ordering::SeqCst);
    pump.join().unwrap();
    handle.shutdown().await;
}
