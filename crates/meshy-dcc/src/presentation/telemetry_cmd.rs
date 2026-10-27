//! `meshy-dcc telemetry status|preview|on|off|reset` (see `docs/telemetry.md`).

use std::time::Duration;

use anyhow::Context;
use clap::Subcommand;
use meshy_dcc_gateway::telemetry;
use serde_json::{Value, json};

#[derive(Debug, Subcommand)]
pub(crate) enum TelemetryAction {
    /// Show whether anonymous usage telemetry is on, and why.
    Status,
    /// Print the exact JSON the next daily send would post. Sends nothing.
    Preview,
    /// Turn telemetry on.
    On,
    /// Turn telemetry off and drop anything not yet sent.
    Off,
    /// Start over with a new random install ID; unsent data is dropped.
    Reset,
}

fn status() -> Value {
    let install_id = std::fs::read_to_string(telemetry::dir().join("install_id"))
        .ok()
        .map(|id| id.trim().to_string());
    json!({
        "enabled": telemetry::enabled(),
        "disabled_reason": telemetry::disabled_reason(),
        "setting": meshy_dcc_updater::settings::get_bool(telemetry::SETTING, true),
        "install_id": install_id,
        "host": telemetry::host(),
        "docs": telemetry::DOCS_URL,
        "turn_off": ["meshy-dcc telemetry off", "DO_NOT_TRACK=1", "MESHY_DCC_TELEMETRY=0"],
    })
}

pub(crate) async fn run(base_url: &str, action: TelemetryAction) -> anyhow::Result<Value> {
    match action {
        TelemetryAction::Status => Ok(status()),
        TelemetryAction::On | TelemetryAction::Off => {
            telemetry::set_enabled(matches!(action, TelemetryAction::On))?;
            Ok(status())
        }
        TelemetryAction::Reset => {
            telemetry::reset();
            Ok(status())
        }
        // The rollup reads the gateway's call history, so the gateway builds it.
        TelemetryAction::Preview => {
            let url = format!(
                "{}/admin/api/telemetry/preview",
                base_url.trim_end_matches('/')
            );
            let response = reqwest::Client::builder()
                .timeout(Duration::from_secs(20))
                .build()?
                .get(&url)
                .send()
                .await
                .with_context(|| format!("could not reach the gateway at {base_url}"))?;
            let status = response.status();
            let body: Value = response.json().await.unwrap_or(Value::Null);
            anyhow::ensure!(
                status.is_success(),
                "gateway answered HTTP {status}: {body}"
            );
            Ok(body)
        }
    }
}
