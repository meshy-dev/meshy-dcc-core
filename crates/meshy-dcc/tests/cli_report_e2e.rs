//! End-to-end coverage for `meshy-dcc report`: the preview with findings from a
//! gateway that is already running, and what a send does and reports.

mod support;

use std::collections::HashMap;

use axum::Router;
use axum::extract::{Json, Query};
use axum::http::StatusCode;
use axum::routing::{get, post};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::sync::oneshot;

use support::*;

/// Answers the feedback query (one finding and one legacy row, for `24h` only)
/// and accepts reports at `/report`.
struct Fixture {
    base_url: String,
    shutdown: Option<oneshot::Sender<()>>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
    }
}

fn spawn_fixture() -> Fixture {
    let app = Router::new()
        .route(
            "/admin/api/feedback",
            get(|Query(query): Query<HashMap<String, String>>| async move {
                let entries = if query["range"] == "24h" {
                    json!([
                        {"id": "legacy", "timestamp": 1.0, "blocker": "old row"},
                        {
                            "id": "finding", "timestamp": 2.0,
                            "recorded_at": "2026-10-27T03:15:00.000Z",
                            "schema_version": 1, "fingerprint": "sha256:private",
                            "dcc_type": "maya", "adapter_version": "0.3.0",
                            "host_version": "2026.1", "phase": "dispatch",
                            "severity": "blocker", "intent": "Open /Users/artist/rig.ma",
                            "observed": "It failed", "expected": "It opens",
                            "evidence": {"error_kind": "tool_failed", "instance_id": "private"}
                        }
                    ])
                } else {
                    json!([])
                };
                Json(json!({"success": true, "entries": entries}))
            }),
        )
        .route(
            "/report",
            post(|| async {
                (
                    StatusCode::ACCEPTED,
                    Json(json!({"report_id": "20261027T031500Z-1a2b3c4d"})),
                )
            }),
        );
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                axum::serve(listener, app)
                    .with_graceful_shutdown(async {
                        let _ = shutdown_rx.await;
                    })
                    .await
                    .unwrap();
            });
    });
    Fixture {
        base_url: format!("http://{addr}"),
        shutdown: Some(shutdown_tx),
    }
}

/// Run `report --summary "it broke" [extra]` against `base_url` with a private registry.
fn report(base_url: &str, report_url: &str, extra: &[&str]) -> (Option<i32>, Value) {
    let registry = TempDir::new().unwrap();
    let profiles = TempDir::new().unwrap();
    let output = cli_command()
        .args(["--base-url", base_url, "--output", "json"])
        .args(["report", "--summary", "it broke"])
        .args(extra)
        .env("MESHY_DCC_REGISTRY_DIR", registry.path())
        .env(
            "MESHY_DCC_GATEWAY_PROFILES_FILE",
            profiles.path().join("profiles.json"),
        )
        .env("MESHY_DCC_REPORT_URL", report_url)
        .output()
        .unwrap();
    (
        output.status.code(),
        serde_json::from_slice(&output.stdout).unwrap(),
    )
}

#[test]
fn report_previews_findings_from_a_running_gateway_without_sending() {
    let fixture = spawn_fixture();

    let (code, preview) = report(&fixture.base_url, "", &[]);

    assert_eq!(code, Some(0));
    assert_eq!(preview["status"], "preview");
    assert!(preview["bytes"].as_u64().unwrap() > 0);
    assert_eq!(
        preview["send_command"],
        json!(["meshy-dcc", "report", "--summary", "it broke", "--yes"])
    );
    let findings = preview["report"]["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 1, "the legacy row is not a finding");
    assert_eq!(findings[0]["error_kind"], "tool_failed");
    assert_eq!(findings[0]["intent"], "Open [path]");
    assert!(!preview.to_string().contains("private"));
}

#[test]
fn report_send_prints_the_report_id_or_exits_unavailable() {
    let fixture = spawn_fixture();
    let intake = format!("{}/report", fixture.base_url);

    let (code, sent) = report(&fixture.base_url, &intake, &["--yes"]);
    assert_eq!(code, Some(0));
    assert_eq!(sent["status"], "sent");
    assert_eq!(sent["report_id"], "20261027T031500Z-1a2b3c4d");

    let closed = format!("http://127.0.0.1:{}/report", unused_loopback_port());
    let (code, failed) = report(&fixture.base_url, &closed, &["--yes"]);
    assert_eq!(code, Some(3));
    assert_eq!(failed["status"], "failed");
}
