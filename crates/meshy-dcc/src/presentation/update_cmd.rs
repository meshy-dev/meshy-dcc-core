use clap::Subcommand;
use serde_json::Value;

use super::output::{ExitCode, OutputWriter};
use crate::application::update::{
    CLI_BINARY_NAME, CLI_VERSION, UpdateService, cached_cli_update, is_background_refresh,
    just_applied_status, record_cli_update, spawn_cli_update_refresh,
};

#[derive(Debug, Subcommand)]
pub(crate) enum UpdateAction {
    /// Check whether a newer version is available.
    Check {
        #[arg(long)]
        binary: Option<String>,
        #[arg(long)]
        current_version: Option<String>,
    },
    /// Download the latest CLI version and stage it for the next launch.
    Apply {
        /// Confirm that the user approved downloading and staging the update.
        #[arg(long)]
        yes: bool,
    },
}

pub(crate) struct UpdateCommandResult {
    pub value: Value,
    pub failed: bool,
    pub exit_code: ExitCode,
}

pub(crate) async fn run(
    base_url: &str,
    action: UpdateAction,
) -> anyhow::Result<UpdateCommandResult> {
    let (value, exit_code) = match action {
        UpdateAction::Check {
            binary,
            current_version,
        } => {
            let binary_name = binary.unwrap_or_else(|| CLI_BINARY_NAME.to_string());
            let current_version = current_version.unwrap_or_else(|| CLI_VERSION.to_string());
            let cache_default_cli =
                binary_name == CLI_BINARY_NAME && current_version == CLI_VERSION;
            let service = UpdateService::new(base_url, &binary_name, &current_version);
            let value = service.check_update().await?;
            if cache_default_cli {
                record_cli_update(&value);
            }
            let exit_code = if value.get("error").is_some() {
                ExitCode::Unavailable
            } else {
                ExitCode::Success
            };
            (value, exit_code)
        }
        UpdateAction::Apply { yes } => {
            let service = UpdateService::new(base_url, CLI_BINARY_NAME, CLI_VERSION);
            let mut value = service.apply_update(yes).await?;
            let mut exit_code = match value["error"].as_str() {
                None => ExitCode::Success,
                Some("confirmation_required") => ExitCode::InvalidInput,
                Some(_) => ExitCode::Unavailable,
            };
            // The gateway binary beside the CLI moves to the same release,
            // whether or not the CLI itself had an update.
            if exit_code == ExitCode::Success
                && let Some(server) = stage_sibling_server(base_url)
            {
                if server["status"] == "failed" {
                    exit_code = ExitCode::Unavailable;
                }
                value["server_update"] = server;
            }
            (value, exit_code)
        }
    };
    Ok(UpdateCommandResult {
        failed: exit_code != ExitCode::Success,
        value,
        exit_code,
    })
}

/// Stage the `meshy-dcc-server` installed beside this CLI through its own
/// `update apply`, which checks the same gateway and verifies the download.
/// `None` when there is no such binary or the gateway is not local.
fn stage_sibling_server(base_url: &str) -> Option<Value> {
    let server = crate::application::gateway_discovery::find_in_same_dir()?;
    stage_server_at(&server, base_url)
}

fn stage_server_at(server: &std::path::Path, base_url: &str) -> Option<Value> {
    let url = reqwest::Url::parse(base_url).ok()?;
    let host = url
        .host_str()?
        .trim_start_matches('[')
        .trim_end_matches(']');
    let local = host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback());
    if !local {
        return None;
    }
    let port = url.port_or_known_default()?;
    let output = std::process::Command::new(server)
        .args(["update", "apply"])
        .env("MESHY_DCC_GATEWAY_PORT", port.to_string())
        .stdin(std::process::Stdio::null())
        .output();
    Some(match output {
        Ok(output) if output.status.success() => serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|_| serde_json::json!({ "status": "unknown" })),
        Ok(output) => serde_json::json!({
            "status": "failed",
            "message": String::from_utf8_lossy(&output.stderr).trim(),
        }),
        Err(error) => serde_json::json!({ "status": "failed", "message": error.to_string() }),
    })
}

pub(crate) fn surface_cached_cli_update(
    value: &mut Value,
    writer: &OutputWriter,
    base_url: &str,
    enabled_for_command: bool,
) -> anyhow::Result<()> {
    if !enabled_for_command || is_background_refresh() {
        return Ok(());
    }

    let cached = cached_cli_update();
    let status = just_applied_status().or(cached.notification);
    if let Some(status) = status {
        if let Some(object) = value.as_object_mut() {
            object.insert("cli_update".into(), status.clone());
        }
        match status["version_status"].as_str() {
            Some("update_available") => {
                let current = status["current_version"].as_str().unwrap_or("unknown");
                let latest = status["latest_version"].as_str().unwrap_or("unknown");
                // `update apply` refuses a package-managed binary; name the
                // command that does upgrade it.
                let message = match crate::application::package_manager::detect() {
                    Some(marker) => format!(
                        "info: meshy-dcc {current} -> {latest} is available. Ask the user before updating; {}.",
                        marker.upgrade_hint()
                    ),
                    None => format!(
                        "info: meshy-dcc {current} -> {latest} is available. Ask the user before updating; after confirmation run `meshy-dcc update apply --yes`. The update activates on the next CLI launch and does not restart running servers."
                    ),
                };
                writer.diagnostic(&message)?
            }
            Some("applied") => writer.diagnostic(
                "info: the staged meshy-dcc update was applied on this launch; running servers were not restarted",
            )?,
            _ => {}
        }
    }
    if cached.refresh_due {
        let _ = spawn_cli_update_refresh(base_url);
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt as _;

    use super::stage_server_at;

    fn fake_server(dir: &std::path::Path, script: &str) -> std::path::PathBuf {
        let path = dir.join("meshy-dcc-server");
        std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[test]
    fn server_update_runs_against_the_local_gateway_port() {
        let dir = tempfile::tempdir().unwrap();
        let server = fake_server(
            dir.path(),
            r#"[ "$1 $2" = "update apply" ] || exit 9
printf '{"status":"staged","port":"%s"}' "$MESHY_DCC_GATEWAY_PORT""#,
        );
        let value = stage_server_at(&server, "http://127.0.0.1:19865").unwrap();
        assert_eq!(value["status"], "staged");
        assert_eq!(value["port"], "19865");
    }

    #[test]
    fn a_failed_server_update_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let server = fake_server(dir.path(), "echo 'no gateway' >&2; exit 1");
        let value = stage_server_at(&server, "http://localhost:19765").unwrap();
        assert_eq!(value["status"], "failed");
        assert_eq!(value["message"], "no gateway");
    }

    #[test]
    fn a_remote_gateway_never_updates_the_local_server() {
        let dir = tempfile::tempdir().unwrap();
        let server = fake_server(dir.path(), "exit 0");
        assert!(stage_server_at(&server, "http://10.0.0.5:19765").is_none());
    }
}
