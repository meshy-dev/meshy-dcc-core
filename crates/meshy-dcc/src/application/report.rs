//! `meshy-dcc report`: a bug report built from public-safe local facts, shown
//! to the user first, and sent to the Meshy intake only when they say yes.
//!
//! Nothing here starts a gateway or a host: the doctor snapshot and the
//! registry read are passive, and findings are read only from a gateway that is
//! already running.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Map, Value, json};

use crate::application::control_plane::DccControlPlane;
use crate::application::doctor::{DoctorContext, run_doctor};
use crate::application::update::CLI_VERSION;
use crate::application::{gateway_ensure, install, local_registry};
use crate::domain::feedback_bundle::{
    project_public_doctor, sanitize_public_prose, sanitize_public_text,
};
use crate::domain::refusal::{Refusal, RefusalKind};
use crate::domain::rest::FeedbackQueryRequest;

pub const REPORT_SCHEMA: &str = "meshy-dcc.report.v1";
/// Where reports go once the Meshy intake exists; `MESHY_DCC_REPORT_URL` overrides it.
pub const REPORT_INTAKE_URL: &str = "";
pub const REPORT_URL_ENV: &str = "MESHY_DCC_REPORT_URL";
pub const MAX_SUMMARY_CHARS: usize = 4000;
const ISSUES_URL: &str = "https://github.com/meshy-dev/meshy-dcc-core/issues?q=label%3Auser-report";
const MAX_REPORT_BYTES: usize = 256 * 1024;
const MAX_FINDINGS: usize = 20;
const MAX_FINDING_TEXT_CHARS: usize = 2000;
const SEND_TIMEOUT: Duration = Duration::from_secs(20);
const FINDINGS_TIMEOUT: Duration = Duration::from_secs(5);

pub fn validate_summary(summary: &str) -> anyhow::Result<()> {
    if summary.trim().is_empty() || summary.chars().count() > MAX_SUMMARY_CHARS {
        return Err(Refusal::new(
            RefusalKind::InvalidInput,
            "invalid_summary",
            format!("--summary must be 1 to {MAX_SUMMARY_CHARS} characters"),
        )
        .into());
    }
    Ok(())
}

/// The intake URL: the environment override, else the compiled-in one.
#[must_use]
pub fn intake_url() -> Option<String> {
    resolve_intake_url(
        &std::env::var(REPORT_URL_ENV).unwrap_or_default(),
        REPORT_INTAKE_URL,
    )
}

fn resolve_intake_url(from_env: &str, built_in: &str) -> Option<String> {
    let url = from_env.trim();
    let url = if url.is_empty() { built_in } else { url };
    (!url.is_empty()).then(|| url.to_string())
}

/// Gather the local facts and build the exact payload that `send` would post.
pub async fn collect(
    summary: &str,
    source: &str,
    control: &DccControlPlane,
    doctor: &DoctorContext,
) -> anyhow::Result<Value> {
    let environment = environment(doctor).await?;
    build_payload(summary, source, environment, recent_findings(control).await)
}

/// The report document. `findings` are newest first; the oldest are dropped
/// until the serialized payload fits the intake's size limit.
pub fn build_payload(
    summary: &str,
    source: &str,
    environment: Value,
    mut findings: Vec<Value>,
) -> anyhow::Result<Value> {
    findings.truncate(MAX_FINDINGS);
    loop {
        let payload = json!({
            "schema": REPORT_SCHEMA,
            "source": source,
            "summary": summary,
            "client": {
                "version": CLI_VERSION,
                "os": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
            },
            "environment": environment,
            "findings": findings,
        });
        if serde_json::to_vec(&payload)?.len() <= MAX_REPORT_BYTES {
            return Ok(payload);
        }
        if findings.pop().is_none() {
            anyhow::bail!("the report exceeds {MAX_REPORT_BYTES} bytes without any findings");
        }
    }
}

/// What the user reviews before anything is sent.
pub fn preview(payload: &Value, send_command: Vec<String>) -> anyhow::Result<Value> {
    Ok(json!({
        "status": "preview",
        "bytes": serde_json::to_vec(payload)?.len(),
        "report": payload,
        "send_command": send_command,
    }))
}

/// POST the payload once. A timeout after the request went out is `unknown`:
/// the intake may have it, so the report is never resent.
pub async fn send(payload: &Value, url: Option<&str>) -> Value {
    let Some(url) = url else {
        return failed("report intake is not configured in this build");
    };
    let Ok(client) = reqwest::Client::builder()
        .timeout(SEND_TIMEOUT)
        .user_agent(format!("meshy-dcc/{CLI_VERSION}"))
        .build()
    else {
        return failed("could not start the report request");
    };
    let response = match client.post(url).json(payload).send().await {
        Ok(response) => response,
        Err(error) if error.is_connect() || error.is_builder() => {
            return failed("could not reach the report intake");
        }
        Err(_) => {
            return json!({
                "status": "unknown",
                "error": "the report intake did not answer; it may have received the report, so it was not resent",
            });
        }
    };
    let status = response.status();
    let body: Value = response.json().await.unwrap_or(Value::Null);
    if status.is_success() {
        return json!({
            "status": "sent",
            "report_id": body["report_id"].as_str(),
            "issues_url": ISSUES_URL,
        });
    }
    let reason: String = body["error"]
        .as_str()
        .unwrap_or_default()
        .chars()
        .take(200)
        .collect();
    failed(
        format!(
            "the report intake refused it (HTTP {}) {reason}",
            status.as_u16()
        )
        .trim(),
    )
}

fn failed(error: impl Into<String>) -> Value {
    json!({"status": "failed", "error": error.into()})
}

/// The public projection of `doctor`, plus the installed connectors and live
/// hosts that doctor only counts.
async fn environment(doctor: &DoctorContext) -> anyhow::Result<Value> {
    let mut snapshot = run_doctor(doctor.request(None, None, None)).await?;
    // Gateway profile names are the user's own labels, and these rows carry
    // window and project names; `instances` below replaces them.
    snapshot["profile"] = json!({"mode": snapshot.pointer("/profile/selected/mode")});
    if let Some(direct) = snapshot
        .pointer_mut("/local/inventory/direct_control")
        .and_then(Value::as_object_mut)
    {
        direct.remove("not_ready_instances");
    }
    snapshot["connectors"] = installed_connectors();
    snapshot["instances"] = live_instances(gateway_ensure::default_registry_dir());
    Ok(project_public_doctor(&snapshot))
}

fn installed_connectors() -> Value {
    let connectors: BTreeSet<_> = install::receipt::list()
        .into_iter()
        .map(|receipt| (receipt.dcc_type, receipt.adapter_version))
        .collect();
    connectors
        .into_iter()
        .map(|(dcc_type, version)| json!({"dcc_type": dcc_type, "version": version}))
        .collect()
}

fn live_instances(registry_dir: PathBuf) -> Value {
    let listing = local_registry::list_local_instances(registry_dir).unwrap_or(Value::Null);
    listing["instances"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|row| {
            json!({
                "dcc_type": row["dcc_type"],
                "host_version": row["version"],
                "status": row["status"],
            })
        })
        .collect()
}

/// Finding v1 rows of the last 24 hours from a gateway that is already up;
/// none when it is not. Asking never starts one.
async fn recent_findings(control: &DccControlPlane) -> Vec<Value> {
    let request = FeedbackQueryRequest {
        range: "24h".into(),
        dcc_type: None,
        severity: None,
        // Headroom for legacy rows, which are skipped.
        limit: 100,
    };
    let Ok(Ok(listing)) =
        tokio::time::timeout(FINDINGS_TIMEOUT, control.feedback_entries(request)).await
    else {
        return Vec::new();
    };
    listing["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(project_finding)
        .take(MAX_FINDINGS)
        .collect()
}

/// The fields a reader needs, redacted and cut. Legacy feedback rows have no
/// `schema_version` and are not findings.
pub fn project_finding(row: &Value) -> Option<Value> {
    row.get("schema_version")?;
    let mut finding = Map::new();
    for (key, value) in [
        ("recorded_at", row.get("recorded_at")),
        ("dcc_type", row.get("dcc_type")),
        ("adapter_version", row.get("adapter_version")),
        ("host_version", row.get("host_version")),
        ("phase", row.get("phase")),
        ("severity", row.get("severity")),
        ("tool_slug", row.get("tool_slug")),
        ("error_kind", row.pointer("/evidence/error_kind")),
        ("intent", row.get("intent")),
        ("observed", row.get("observed")),
        ("expected", row.get("expected")),
    ] {
        if let Some(text) = value.and_then(Value::as_str) {
            // Prose keeps its words; short fields such as a tool slug go whole.
            let text = if matches!(key, "intent" | "observed" | "expected") {
                sanitize_public_prose(text)
            } else {
                sanitize_public_text(text)
            };
            let text: String = text.chars().take(MAX_FINDING_TEXT_CHARS).collect();
            finding.insert(key.to_string(), Value::String(text));
        }
    }
    Some(Value::Object(finding))
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::Router;
    use axum::extract::{Json, State};
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::post;

    use super::*;

    fn row(index: usize) -> Value {
        json!({
            "id": format!("feedback-{index}"),
            "timestamp": 1787536800.0 + index as f64,
            "recorded_at": format!("2026-10-27T03:15:{index:02}.000Z"),
            "schema_version": 1,
            "fingerprint": format!("sha256:{}", "a".repeat(64)),
            "dcc_type": "maya",
            "adapter": "meshy-dcc-maya",
            "adapter_version": "0.3.0",
            "core_version": "0.2.0",
            "host_version": "2026.1",
            "os": "macos",
            "phase": "dispatch",
            "severity": "blocker",
            "tool_slug": "maya.create_sphere",
            "intent": "Make a sphere",
            "observed": "x".repeat(5000),
            "expected": "A sphere",
            "repro": {"argv": ["meshy-dcc", "status"], "steps": []},
            "evidence": {"error_kind": "tool_failed", "instance_id": "private-instance"},
            "redaction_status": {"mode": "public-safe"},
        })
    }

    #[test]
    fn findings_keep_only_public_fields_cut_and_redacted() {
        let mut private = row(1);
        private["intent"] = json!("Open /Users/artist/rig.ma");
        let finding = project_finding(&private).unwrap();
        let keys: Vec<_> = finding.as_object().unwrap().keys().cloned().collect();

        assert_eq!(
            keys.iter().map(String::as_str).collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "recorded_at",
                "dcc_type",
                "adapter_version",
                "host_version",
                "phase",
                "severity",
                "tool_slug",
                "error_kind",
                "intent",
                "observed",
                "expected",
            ])
        );
        assert_eq!(finding["error_kind"], "tool_failed");
        assert_eq!(finding["intent"], "Open [path]");
        assert_eq!(finding["observed"].as_str().unwrap().chars().count(), 2000);
        assert!(!finding.to_string().contains("private-instance"));
        // A legacy feedback row is not a finding.
        assert!(project_finding(&json!({"id": "x", "blocker": "y"})).is_none());
    }

    #[test]
    fn payload_drops_the_oldest_findings_to_fit_the_size_limit() {
        // Newest first; each finding is ~16 KiB, so 20 of them cannot fit.
        let findings: Vec<_> = (0..20)
            .map(|index| json!({"recorded_at": index, "observed": "x".repeat(16 * 1024)}))
            .collect();

        let payload = build_payload("it broke", "cli", json!({}), findings).unwrap();
        let kept = payload["findings"].as_array().unwrap();

        assert!(serde_json::to_vec(&payload).unwrap().len() <= MAX_REPORT_BYTES);
        assert!((1..20).contains(&kept.len()));
        // The newest survive, in order.
        assert_eq!(kept[0]["recorded_at"], 0);
        assert_eq!(kept.last().unwrap()["recorded_at"], kept.len() - 1);
        assert_eq!(payload["schema"], REPORT_SCHEMA);
        assert_eq!(payload["client"]["version"], CLI_VERSION);
    }

    #[test]
    fn live_instances_name_only_the_app_its_version_and_its_status() {
        use meshy_dcc_transport::discovery::file_registry::FileRegistry;
        use meshy_dcc_transport::discovery::types::ServiceEntry;

        let dir = tempfile::tempdir().unwrap();
        let registry = FileRegistry::new(dir.path()).unwrap();
        let mut entry = ServiceEntry::new("maya", "127.0.0.1", 18080);
        entry.display_name = Some("Maya-Rig".into());
        entry.version = Some("2026.1".into());
        registry.register(entry).unwrap();

        assert_eq!(
            live_instances(dir.path().to_path_buf()),
            json!([{"dcc_type": "maya", "host_version": "2026.1", "status": "available"}])
        );
    }

    #[tokio::test]
    async fn sending_without_an_intake_url_fails_without_a_request() {
        let payload = build_payload("it broke", "cli", json!({}), Vec::new()).unwrap();

        assert_eq!(
            send(&payload, None).await,
            json!({"status": "failed", "error": "report intake is not configured in this build"})
        );
    }

    #[tokio::test]
    async fn send_posts_the_payload_and_returns_the_intake_report_id() {
        type Seen = Arc<Mutex<Vec<(String, Value)>>>;
        async fn intake(
            State(seen): State<Seen>,
            headers: HeaderMap,
            Json(body): Json<Value>,
        ) -> (StatusCode, Json<Value>) {
            let agent = headers["user-agent"].to_str().unwrap().to_string();
            seen.lock().unwrap().push((agent, body));
            (
                StatusCode::ACCEPTED,
                Json(json!({"report_id": "20261027T031500Z-1a2b3c4d"})),
            )
        }

        let seen = Seen::default();
        let app = Router::new()
            .route("/report", post(intake))
            .with_state(seen.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let payload = build_payload("it broke", "ui", json!({}), Vec::new()).unwrap();

        let result = send(&payload, Some(&format!("http://{addr}/report"))).await;

        assert_eq!(result["status"], "sent");
        assert_eq!(result["report_id"], "20261027T031500Z-1a2b3c4d");
        assert_eq!(result["issues_url"], ISSUES_URL);
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].0, format!("meshy-dcc/{CLI_VERSION}"));
        assert_eq!(seen[0].1, payload);
        server.abort();
    }

    #[test]
    fn the_environment_overrides_the_built_in_intake_url() {
        assert_eq!(resolve_intake_url("", ""), None);
        assert_eq!(resolve_intake_url("  ", ""), None);
        assert_eq!(
            resolve_intake_url("", "https://b/r").as_deref(),
            Some("https://b/r")
        );
        assert_eq!(
            resolve_intake_url(" http://e/r ", "https://b/r").as_deref(),
            Some("http://e/r")
        );
    }

    #[test]
    fn summaries_are_one_to_four_thousand_characters() {
        assert!(validate_summary("it broke").is_ok());
        assert!(validate_summary(&"x".repeat(MAX_SUMMARY_CHARS)).is_ok());
        assert!(validate_summary("  ").is_err());
        assert!(validate_summary(&"x".repeat(MAX_SUMMARY_CHARS + 1)).is_err());
    }
}
