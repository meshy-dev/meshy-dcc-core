use super::tools::*;
use crate::gateway::capability::InstanceFingerprint;
use crate::gateway::capability_service;
use crate::gateway::state::GatewayState;
use meshy_dcc_transport::discovery::file_registry::FileRegistry;
use serde_json::{Map, Value, json};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, broadcast, watch};
use uuid::Uuid;

fn test_gateway_state() -> GatewayState {
    let dir = tempfile::tempdir().unwrap();
    let (yield_tx, _) = watch::channel(false);
    let (events_tx, _) = broadcast::channel::<String>(8);
    GatewayState {
        ingress: std::sync::Arc::new(crate::gateway::http_limits::GatewayIngressState::from_env()),
        resilience: std::sync::Arc::new(Default::default()),
        registry: std::sync::Arc::new(FileRegistry::new(dir.path()).unwrap()),
        http_instance_registry: Arc::new(parking_lot::RwLock::new(
            crate::gateway::http_registration::HttpInstanceRegistry::default(),
        )),
        mdns_instance_registry: Arc::new(parking_lot::RwLock::new(
            crate::gateway::mdns_registration::MdnsInstanceRegistry::default(),
        )),
        relay_instance_registry: Arc::new(parking_lot::RwLock::new(
            crate::gateway::relay_registration::RelayInstanceRegistry::default(),
        )),
        stale_timeout: Duration::from_secs(30),
        backend_timeout: Duration::from_secs(10),
        async_dispatch_timeout: Duration::from_secs(60),
        wait_terminal_timeout: Duration::from_secs(600),
        server_name: "test".into(),
        server_version: env!("CARGO_PKG_VERSION").into(),
        own_host: "127.0.0.1".into(),
        own_port: 0,
        http_client: reqwest::Client::new(),
        yield_tx: Arc::new(yield_tx),
        events_tx: Arc::new(events_tx),
        protocol_version: Arc::new(RwLock::new(None)),
        resource_subscriptions: Arc::new(RwLock::new(std::collections::HashMap::new())),
        client_attribution: Arc::new(
            crate::gateway::caller_attribution::ClientAttributionStore::default(),
        ),
        pending_calls: Arc::new(RwLock::new(std::collections::HashMap::new())),
        subscriber: crate::gateway::sse_subscriber::SubscriberManager::default(),
        allow_unknown_tools: false,
        policy: Arc::new(crate::gateway::GatewayPolicy::default()),
        adapter_version: None,
        adapter_dcc: None,
        capability_index: Arc::new(crate::gateway::capability::CapabilityIndex::new()),
        search_cache: Arc::new(crate::gateway::capability::search_cache::SearchCache::new(
            Default::default(),
        )),
        event_log: Arc::new(crate::gateway::event_log::EventLog::new()),
        #[cfg(feature = "prometheus")]
        gateway_metrics: Arc::new(crate::gateway::event_log::GatewayMetrics::new()),
        middleware_chain: Arc::new(crate::gateway::middleware::MiddlewareChain::new()),
        instance_diagnostics: Arc::new(
            crate::gateway::instance_diagnostics::InstanceDiagnosticsStore::new(),
        ),
        traffic_capture: Arc::new(crate::gateway::traffic::TrafficCapture::disabled()),
        search_telemetry: Arc::new(crate::gateway::search_telemetry::SearchTelemetryStore::new()),
        debug_routes_enabled: false,
        auth: Arc::new(crate::gateway::security::GatewayAuth::disabled()),
        update_manifest_url: None,
        gateway_persist: false,
        gateway_idle_timeout_secs: 30,
        semantic_search_enabled: false,
        #[cfg(feature = "admin-persist-sqlite")]
        admin_sqlite_lane: None,
    }
}

fn annotations_by_tool() -> Map<String, Value> {
    gateway_tool_defs()
        .as_array()
        .expect("gateway_tool_defs returns an array")
        .iter()
        .map(|tool| {
            let name = tool
                .get("name")
                .and_then(Value::as_str)
                .expect("gateway tool has a name")
                .to_string();
            let annotations = tool
                .get("annotations")
                .cloned()
                .expect("gateway tool has annotations");
            (name, annotations)
        })
        .collect()
}

#[test]
fn gateway_tool_defs_advertise_canonical_workflow_tools_only() {
    let defs = gateway_tool_defs();
    let names: Vec<&str> = defs
        .as_array()
        .expect("gateway_tool_defs returns an array")
        .iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str))
        .collect();

    assert_eq!(names, ["search", "describe", "load_skill", "call"]);
}

#[test]
fn gateway_tool_defs_all_have_annotations() {
    let annotations = annotations_by_tool();
    assert_eq!(annotations.len(), 4);

    for (name, value) in annotations {
        let hints = value
            .as_object()
            .unwrap_or_else(|| panic!("{name} annotations must be an object"));
        assert!(
            [
                "readOnlyHint",
                "destructiveHint",
                "idempotentHint",
                "openWorldHint"
            ]
            .iter()
            .any(|key| hints.contains_key(*key)),
            "{name} annotations must include at least one MCP ToolAnnotations hint"
        );
    }
}

#[test]
fn gateway_call_schema_keeps_compatibility_shape() {
    let defs = gateway_tool_defs();
    let call = defs
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "call")
        .expect("call tool advertised");

    assert_eq!(call["inputSchema"]["type"], "object");
    assert!(call["inputSchema"].get("anyOf").is_none());
    assert!(call["inputSchema"].get("oneOf").is_none());
    assert!(call["inputSchema"].get("allOf").is_none());
    assert!(call["inputSchema"].get("not").is_none());
    assert!(call["inputSchema"].get("required").is_none());
    assert!(call["inputSchema"]["properties"]["tool_slug"].is_object());
    assert_eq!(call["inputSchema"]["properties"]["calls"]["maxItems"], 25);
    assert_eq!(call["annotations"]["destructiveHint"], true);
}

#[test]
fn gateway_search_schema_exposes_instance_filter() {
    let defs = gateway_tool_defs();
    let search = defs
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "search")
        .expect("search tool advertised");

    assert_eq!(
        search["inputSchema"]["properties"]["instance_id"]["type"],
        "string"
    );
}

#[test]
fn gateway_tool_defs_use_expected_annotations() {
    let annotations = annotations_by_tool();

    assert_eq!(
        annotations.get("search"),
        Some(&json!({"readOnlyHint": true, "openWorldHint": true}))
    );
    assert_eq!(
        annotations.get("describe"),
        Some(&json!({"readOnlyHint": true, "openWorldHint": true}))
    );
}

#[test]
fn describe_refresh_depends_only_on_index_hit() {
    let gs = test_gateway_state();
    let instance_id = Uuid::from_u128(0x1234);
    let record = crate::gateway::capability::CapabilityRecord::new(
        crate::gateway::capability::tool_slug("maya", &instance_id, "maya_scene__list_objects"),
        "maya_scene__list_objects".to_string(),
        "maya_scene__list_objects".to_string(),
        Some("maya-scene".into()),
        "List scene objects",
        vec!["scene".into()],
        "maya".into(),
        instance_id,
        false,
        true,
        None,
    );
    let fingerprint = InstanceFingerprint(1);
    gs.capability_index
        .upsert_instance(instance_id, vec![record.clone()], fingerprint);

    let current_generation = capability_service::index_generation(&gs.capability_index);
    let args = json!({
        "tool_slug": record.tool_slug,
        "meta": {"index_generation": current_generation}
    });
    assert!(!describe_needs_refresh(&gs, &record.tool_slug, &args, None));

    let stale_args = json!({
        "tool_slug": record.tool_slug,
        "meta": {"index_generation": "stale"}
    });
    assert!(
        !describe_needs_refresh(&gs, &record.tool_slug, &stale_args, None),
        "a present slug must ignore unrelated generation drift"
    );

    assert!(describe_needs_refresh(
        &gs,
        "maya.abcdef01.__missing__",
        &json!({}),
        None
    ));
}

#[test]
fn calls_refresh_only_after_route_index_errors() {
    use crate::gateway::capability_service::ServiceError;

    assert!(call_error_needs_refresh(&ServiceError::new(
        "unknown-slug",
        "missing capability",
    )));
    assert!(call_error_needs_refresh(&ServiceError::new(
        "instance-offline",
        "live instance not indexed yet",
    )));
    assert!(!call_error_needs_refresh(
        &ServiceError::new("instance-offline", "instance exited")
            .with_instance_provenance("exited", Some(Uuid::from_u128(1)))
    ));
    assert!(!call_error_needs_refresh(&ServiceError::new(
        "backend-error",
        "tool execution failed",
    )));
    assert!(!call_error_needs_refresh(&ServiceError::new(
        "policy-denied",
        "request rejected",
    )));
}

/// Capture both gateway dispatch contracts without invoking a host.
async fn metadata_backend(
    sidecar: bool,
    job: bool,
) -> (
    GatewayState,
    String,
    Arc<std::sync::Mutex<Vec<Value>>>,
    tempfile::TempDir,
    tokio::task::JoinHandle<()>,
) {
    use axum::{
        Json, Router,
        http::HeaderMap,
        routing::{get, post},
    };
    let received = Arc::new(std::sync::Mutex::new(Vec::new()));
    let rest_received = received.clone();
    let mcp_received = received.clone();
    let app = Router::new()
        .route("/v1/readyz", get(|| async { Json(json!({"process": true, "dcc": true,
            "dispatcher": true, "skill_catalog": true, "host_execution_bridge": true,
            "main_thread_executor": true})) }))
        .route("/v1/call", post(move |headers: HeaderMap, Json(body): Json<Value>| {
            let received = rest_received.clone();
            async move {
                received.lock().unwrap().push(body.clone());
                if let Some(ms) = body["arguments"]["delay_ms"].as_u64() {
                    tokio::time::sleep(Duration::from_millis(ms)).await;
                }
                Json(json!({"request_id": headers.get("x-request-id").and_then(|v| v.to_str().ok()),
                    "received_meta": body["meta"], "arguments": body["arguments"]}))
            }
        }))
        .route("/mcp", post(move |Json(body): Json<Value>| {
            let received = mcp_received.clone();
            async move {
                received.lock().unwrap().push(body.clone());
                if let Some(ms) = body["params"]["arguments"]["delay_ms"].as_u64() {
                    tokio::time::sleep(Duration::from_millis(ms)).await;
                }
                Json(json!({"jsonrpc": "2.0", "id": body["id"], "result": {
                    "received_meta": body["params"]["_meta"], "arguments": body["params"]["arguments"],
                    "job_id": body["params"]["arguments"]["job_id"]}}))
            }
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let dir = tempfile::tempdir().unwrap();
    let mut gs = test_gateway_state();
    gs.registry = Arc::new(FileRegistry::new(dir.path()).unwrap());
    let mut entry =
        meshy_dcc_transport::discovery::types::ServiceEntry::new("maya", "127.0.0.1", port);
    entry.lease_owner = Some("owner-1".into());
    if sidecar {
        entry
            .metadata
            .insert("meshy_dcc_role".into(), "per-dcc-sidecar".into());
    }
    let id = entry.instance_id;
    gs.registry.register(entry).unwrap();
    let name = if job {
        "jobs_get_status"
    } else {
        "scene__inspect"
    };
    let slug = crate::gateway::capability::tool_slug("maya", &id, name);
    let record = crate::gateway::capability::CapabilityRecord::new(
        slug.clone(),
        name.into(),
        name.into(),
        None,
        "Inspect test fixture",
        vec![],
        "maya".into(),
        id,
        true,
        true,
        None,
    );
    gs.capability_index
        .upsert_instance(id, vec![record], InstanceFingerprint(1));
    (gs, slug, received, dir, server)
}

#[tokio::test]
async fn single_calls_preserve_metadata_and_bound_normal_sidecar_and_job_dispatch() {
    use crate::gateway::admin::trace::{AgentContext, TraceContext};
    for (sidecar, job) in [(false, false), (true, false), (false, true)] {
        let (gs, slug, received, _dir, server) = metadata_backend(sidecar, job).await;
        let outer = json!({"agent_context": {"actor_id": "protocol-actor"},
            "meshy-dcc/timeout_ms": 5000, "search_id": "outer-search"});
        let canonical = AgentContext {
            actor_id: Some("canonical-actor".into()),
            ..Default::default()
        };
        let trace =
            TraceContext::from_headers_with_request_id(&Default::default(), "call-identity");
        let args = json!({"tool_slug": slug, "arguments": {"job_id": "existing-job"},
            "meta": {"lease_owner": "owner-1", "permission_hint": "read",
                "agent_context": {"actor_id": "fake"}, "meshy-dcc/timeout_ms": 60000}});
        let (text, failed) =
            tool_call_tool(&gs, &args, Some(&outer), Some(&trace), Some(&canonical)).await;
        assert!(!failed, "{text}");
        let result: Value = serde_json::from_str(&text).unwrap();
        let meta = &result["received_meta"];
        assert_eq!(meta["agent_context"]["actor_id"], "canonical-actor");
        assert_eq!(meta["search_id"], "outer-search");
        assert_eq!(meta["permission_hint"], "read");
        let forwarded = meta["meshy-dcc/timeout_ms"].as_u64().unwrap();
        assert!(forwarded > 0 && forwarded <= 4500, "{meta}");
        assert_eq!(result["arguments"], json!({"job_id": "existing-job"}));
        let captured = received.lock().unwrap()[0].clone();
        if sidecar || job {
            assert_eq!(captured["id"], "call-identity");
            assert_eq!(
                captured["params"]["name"],
                if job {
                    "jobs_get_status"
                } else {
                    "scene__inspect"
                }
            );
            assert_eq!(result["job_id"], "existing-job");
        }
        let expired = json!({"tool_slug": slug, "meta": {"lease_owner": "owner-1", "meshy-dcc/timeout_ms": 0}});
        let (text, failed) =
            tool_call_tool(&gs, &expired, Some(&outer), None, Some(&canonical)).await;
        assert!(failed && text.contains("not-applied"), "{text}");
        assert_eq!(
            received.lock().unwrap().len(),
            1,
            "expired call must not dispatch"
        );
        let (text, failed) = tool_call_tool(
            &gs,
            &json!({"tool_slug": slug, "arguments": {"delay_ms": 1500},
            "meta": {"lease_owner": "owner-1", "meshy-dcc/timeout_ms": 700}}),
            None,
            None,
            None,
        )
        .await;
        assert!(failed && text.contains("outcome-unknown"), "{text}");
        server.abort();
    }
}

#[tokio::test]
async fn batch_item_metadata_cannot_reset_context_or_extend_either_budget() {
    use crate::gateway::admin::trace::AgentContext;
    let (gs, slug, received, _dir, server) = metadata_backend(false, false).await;
    let canonical = AgentContext {
        actor_id: Some("canonical".into()),
        ..Default::default()
    };
    let outer = json!({"meshy-dcc/timeout_ms": 10000, "lease_owner": "owner-1", "search_id": "batch-search"});
    let result = gateway_call_batch_inner(
        &gs,
        &json!({"calls": [
            {"tool_slug": slug, "meta": {"meshy-dcc/timeout_ms": 2000, "permission_hint": "read"}},
            {"tool_slug": slug, "meta": {"meshy-dcc/timeout_ms": 40000, "agent_context": null}},
            {"tool_slug": slug, "meta": {"meshy-dcc/timeout_ms": 0}},
            {"tool_slug": slug, "meta": null}
        ]}),
        Some(&outer),
        None,
        Some(&canonical),
    )
    .await
    .unwrap();
    for (index, limit) in [(0, 1500), (1, 9500), (3, 9500)] {
        let meta = &result["results"][index]["result"]["received_meta"];
        assert_eq!(meta["agent_context"]["actor_id"], "canonical");
        assert_eq!(meta["search_id"], "batch-search");
        assert!(
            meta["meshy-dcc/timeout_ms"].as_u64().unwrap() <= limit,
            "{meta}"
        );
    }
    assert_eq!(
        result["results"][2]["error"]["error"]["kind"],
        "not-applied"
    );
    assert_eq!(received.lock().unwrap().len(), 3);
    server.abort();
}
