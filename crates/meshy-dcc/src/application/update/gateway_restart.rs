//! Restart the local gateway after `update apply`, so the new server runs at
//! once. Only a standalone gateway on a loopback address of this computer is
//! restarted; one an app started is left alone, because stopping it would stop
//! that app's server. The outcome is read back from the new gateway, and a
//! restart is never retried.

use std::time::Duration;

use serde_json::{Value, json};

use crate::application::gateway_ctrl::{
    self, GatewayDaemonRequest, GatewayDaemonStartRequest, local_auto_gateway_target,
};
use crate::domain::rest::Endpoint;

const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const STOP_TIMEOUT_SECS: u64 = 10;
const START_TIMEOUT_SECS: u64 = 30;

/// What `/admin/api/health` says about the running gateway.
#[derive(Debug, Clone, PartialEq)]
struct Running {
    version: Option<String>,
    started_by: Option<String>,
    persist: bool,
    idle_timeout_secs: u64,
    network_listener: Option<String>,
}

impl Running {
    fn from_health(health: &Value) -> Self {
        let service = &health["service"];
        Self {
            version: health["version"].as_str().map(str::to_owned),
            started_by: service["started_by"].as_str().map(str::to_owned),
            persist: service["persist"].as_bool().unwrap_or(false),
            idle_timeout_secs: service["idle_timeout_secs"].as_u64().unwrap_or(30),
            network_listener: service["network_listener"].as_str().map(str::to_owned),
        }
    }
}

/// Restart the gateway at `base_url` when it runs a version other than
/// `target_version`, and report `status` with the old and new version.
pub async fn restart_local_gateway(base_url: &str, target_version: Option<&str>) -> Value {
    let Some((host, port)) = local_auto_gateway_target(&Endpoint::new(base_url)) else {
        return skipped(
            "not_local",
            "The gateway is not on this computer; restart it where it runs.",
        );
    };
    let before = match admin_health(&host, port).await {
        Some(health) => Running::from_health(&health),
        None if crate::application::gateway_ensure::gateway_health_ok(&host, port).await => {
            return skipped(
                "version_unknown",
                "The gateway did not report its version; run `meshy-dcc gateway restart`.",
            );
        }
        None => {
            return json!({
                "status": "not_running",
                "message": "No gateway is running; the next command starts the new one.",
            });
        }
    };
    if let Some(outcome) = refuse(&before, target_version) {
        return outcome;
    }
    let restart = gateway_ctrl::run_gateway_daemon(GatewayDaemonRequest::Restart {
        start: start_request(&host, port, &before),
        stop_timeout_secs: STOP_TIMEOUT_SECS,
    })
    .await;
    let after = admin_health(&host, port)
        .await
        .map(|health| Running::from_health(&health));
    verify(&before, after.as_ref(), target_version, restart.err())
}

/// Why this gateway must not, or need not, be restarted.
fn refuse(before: &Running, target_version: Option<&str>) -> Option<Value> {
    if before.started_by.as_deref() != Some("gateway") {
        let app = before.started_by.as_deref().unwrap_or("an app");
        return Some(json!({
            "status": "skipped",
            "reason": "started_by_app",
            "started_by": before.started_by,
            "old_version": before.version,
            "message": format!(
                "The gateway was started by {app}; it runs the new version after {app} restarts."
            ),
        }));
    }
    if target_version.is_some() && before.version.as_deref() == target_version {
        return Some(json!({
            "status": "not_needed",
            "old_version": before.version,
            "new_version": before.version,
        }));
    }
    None
}

/// Judge the restart by what the gateway reports afterwards.
fn verify(
    before: &Running,
    after: Option<&Running>,
    target_version: Option<&str>,
    error: Option<anyhow::Error>,
) -> Value {
    let message = error.as_ref().map(|error| format!("{error:#}"));
    let Some(after) = after else {
        // Nothing answers. After a refused restart that means it is stopped;
        // after an accepted one it may still be starting.
        return match message {
            Some(message) => json!({
                "status": "failed",
                "old_version": before.version,
                "new_version": null,
                "message": format!("{message}. No gateway is running; the next command starts one."),
            }),
            None => json!({
                "status": "unknown",
                "old_version": before.version,
                "new_version": null,
                "message": "The gateway did not answer after the restart; check `meshy-dcc gateway status`.",
            }),
        };
    };
    let updated = match target_version {
        Some(target) => after.version.as_deref() == Some(target),
        None => after.version != before.version,
    };
    let status = if updated { "restarted" } else { "failed" };
    let message = match (updated, message) {
        (true, _) => None,
        (false, Some(message)) => Some(message),
        (false, None) => Some(format!(
            "The gateway still runs {}; run `meshy-dcc gateway restart`.",
            after.version.as_deref().unwrap_or("an unknown version")
        )),
    };
    json!({
        "status": status,
        "old_version": before.version,
        "new_version": after.version,
        "message": message,
    })
}

fn start_request(host: &str, port: u16, before: &Running) -> GatewayDaemonStartRequest {
    let (remote_host, remote_port) = before
        .network_listener
        .as_deref()
        .and_then(|addr| addr.parse::<std::net::SocketAddr>().ok())
        .map_or(("127.0.0.1".to_owned(), 0), |addr| {
            (addr.ip().to_string(), addr.port())
        });
    GatewayDaemonStartRequest {
        host: host.to_owned(),
        port,
        name: std::env::var("MESHY_DCC_GATEWAY_NAME")
            .ok()
            .filter(|name| !name.trim().is_empty())
            .or_else(|| Some("meshy-dcc-gateway".to_owned())),
        registry_dir: None,
        remote_host,
        remote_port,
        // 0 keeps a persistent gateway running, as it was.
        gateway_idle_timeout_secs: if before.persist {
            0
        } else {
            before.idle_timeout_secs
        },
        gateway_bin: None,
        wait_timeout_secs: START_TIMEOUT_SECS,
    }
}

async fn admin_health(host: &str, port: u16) -> Option<Value> {
    let client = reqwest::Client::builder()
        .timeout(PROBE_TIMEOUT)
        .build()
        .ok()?;
    let response = client
        .get(format!("http://{host}:{port}/admin/api/health"))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.json().await.ok()
}

fn skipped(reason: &str, message: &str) -> Value {
    json!({ "status": "skipped", "reason": reason, "message": message })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn running(version: &str, started_by: &str) -> Running {
        Running::from_health(&json!({
            "version": version,
            "service": {"started_by": started_by, "persist": false, "idle_timeout_secs": 30},
        }))
    }

    #[test]
    fn an_app_hosted_gateway_is_never_restarted() {
        let outcome = refuse(&running("0.2.1", "blender"), Some("0.2.2")).unwrap();
        assert_eq!(outcome["status"], "skipped");
        assert_eq!(outcome["reason"], "started_by_app");
    }

    #[test]
    fn a_gateway_on_the_target_version_is_left_running() {
        let outcome = refuse(&running("0.2.2", "gateway"), Some("0.2.2")).unwrap();
        assert_eq!(outcome["status"], "not_needed");
        assert!(refuse(&running("0.2.1", "gateway"), Some("0.2.2")).is_none());
    }

    #[test]
    fn the_new_version_must_answer_after_the_restart() {
        let before = running("0.2.1", "gateway");
        let restarted = verify(&before, Some(&running("0.2.2", "gateway")), Some("0.2.2"), None);
        assert_eq!(restarted["status"], "restarted");
        assert_eq!(restarted["old_version"], "0.2.1");
        assert_eq!(restarted["new_version"], "0.2.2");

        let stale = verify(&before, Some(&running("0.2.1", "gateway")), Some("0.2.2"), None);
        assert_eq!(stale["status"], "failed");
    }

    #[test]
    fn silence_after_an_accepted_restart_is_unknown() {
        let before = running("0.2.1", "gateway");
        assert_eq!(verify(&before, None, Some("0.2.2"), None)["status"], "unknown");
        let refused = verify(&before, None, Some("0.2.2"), Some(anyhow::anyhow!("refusing")));
        assert_eq!(refused["status"], "failed");
    }

    #[test]
    fn a_persistent_gateway_restarts_persistent_with_its_listener() {
        let mut before = running("0.2.1", "gateway");
        before.persist = true;
        before.network_listener = Some("0.0.0.0:19766".into());
        let request = start_request("127.0.0.1", 19865, &before);
        assert_eq!(request.gateway_idle_timeout_secs, 0);
        assert_eq!((request.remote_host.as_str(), request.remote_port), ("0.0.0.0", 19766));
    }
}
