//! Endpoints behind the user-facing pages of `meshy-dcc ui`: connectors, the
//! agent Skill and the bug report.
//!
//! The work is done by the `meshy-dcc` CLI installed next to the gateway; its
//! JSON output is the contract, so installs and Skill writes keep one
//! implementation. Both act on the computer the gateway runs on, so the
//! endpoints that change anything answer only requests from that computer
//! (a loopback peer); a remote viewer gets `local_only` and runs the CLI there.
//!
//! - `GET  /api/connectors`         — `meshy-dcc dcc-types` (catalog + install receipts)
//! - `POST /api/connectors/install` — `meshy-dcc install --execute --json`
//! - `GET  /api/agents`             — `meshy-dcc agent-skill --agent '*'`
//! - `POST /api/agents/skill`       — `meshy-dcc agent-skill --agent … --execute`
//! - `POST /api/report`             — `meshy-dcc report --source ui [--yes]`
//! - `POST /api/update`             — `meshy-dcc update apply --yes --no-restart`,
//!   then `meshy-dcc gateway restart` in a detached process

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use axum::Json;
use axum::body::Body;
use axum::extract::{ConnectInfo, State};
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::gateway::admin::state::AdminState;

/// Overrides the CLI the gateway runs (tests, unusual layouts).
pub const CLI_BIN_ENV: &str = "MESHY_DCC_CLI_BIN";

const READ_TIMEOUT: Duration = Duration::from_secs(60);
// ponytail: one fixed ceiling; downloads of a large adapter on a slow link may need more.
const INSTALL_TIMEOUT: Duration = Duration::from_secs(600);
// Above the CLI's own 600 s ceiling for the server download.
const UPDATE_TIMEOUT: Duration = Duration::from_secs(900);

fn error(status: StatusCode, code: &str, message: impl Into<String>) -> Response {
    (
        status,
        Json(json!({"ok": false, "error": code, "message": message.into()})),
    )
        .into_response()
}

/// Route layer: only a peer on this computer may change it.
pub async fn local_only(req: Request<Body>, next: Next) -> Response {
    let local = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .is_some_and(|ConnectInfo(addr)| addr.ip().is_loopback());
    if !local {
        return error(
            StatusCode::FORBIDDEN,
            "local_only",
            "this change can only be made on the computer the gateway runs on",
        );
    }
    next.run(req).await
}

/// The CLI next to this executable, or this executable when it is the CLI.
fn cli_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(CLI_BIN_ENV).filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(path));
    }
    let exe = std::env::current_exe().ok()?;
    if exe.file_stem().is_some_and(|stem| stem == "meshy-dcc") {
        return Some(exe);
    }
    let sibling = exe.with_file_name(format!("meshy-dcc{}", std::env::consts::EXE_SUFFIX));
    sibling.is_file().then_some(sibling)
}

async fn run_cli(args: &[String], timeout: Duration) -> Result<Value, Response> {
    let Some(cli) = cli_path() else {
        return Err(error(
            StatusCode::SERVICE_UNAVAILABLE,
            "cli_not_found",
            "meshy-dcc was not found next to the gateway",
        ));
    };
    let mut command = tokio::process::Command::new(&cli);
    command
        .args(["--output", "json", "--non-interactive", "--no-auto-gateway"])
        .args(args)
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    let output = match tokio::time::timeout(timeout, command.output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(err)) => {
            return Err(error(
                StatusCode::BAD_GATEWAY,
                "cli_failed",
                format!("could not run {}: {err}", cli.display()),
            ));
        }
        Err(_) => {
            return Err(error(
                StatusCode::GATEWAY_TIMEOUT,
                "cli_timeout",
                format!("meshy-dcc did not finish within {} s", timeout.as_secs()),
            ));
        }
    };
    serde_json::from_slice(&output.stdout).map_err(|_| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // ponytail: the last 2000 characters; the CLI's own output holds the rest.
        let tail: String = stderr
            .chars()
            .rev()
            .take(2000)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        error(
            StatusCode::BAD_GATEWAY,
            "cli_failed",
            tail.trim().to_string(),
        )
    })
}

fn respond(result: Result<Value, Response>) -> Response {
    match result {
        Ok(value) => Json(value).into_response(),
        Err(response) => response,
    }
}

/// A catalog or agent id; never something the CLI could read as a flag.
fn valid_id(value: &str) -> bool {
    value
        .bytes()
        .next()
        .is_some_and(|b| b.is_ascii_alphanumeric())
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// One install at a time: two would race on the same host files.
fn install_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

/// `GET /admin/api/connectors`
pub async fn handle_connectors() -> Response {
    respond(run_cli(&["dcc-types".into()], READ_TIMEOUT).await)
}

#[derive(Debug, Deserialize)]
pub struct InstallConnectorBody {
    pub dcc_type: String,
    /// Absolute project root, for connectors installed per project.
    #[serde(default)]
    pub project: Option<PathBuf>,
    /// Absolute path of the app, when the CLI could not find it on its own.
    #[serde(default)]
    pub dcc_path: Option<PathBuf>,
}

/// `POST /admin/api/connectors/install`
pub async fn handle_connector_install(Json(body): Json<InstallConnectorBody>) -> Response {
    if !valid_id(&body.dcc_type) {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_dcc_type",
            "unknown connector",
        );
    }
    let mut args = vec![
        "install".to_string(),
        "--dcc-type".to_string(),
        body.dcc_type,
    ];
    if let Some(project) = body.project {
        if !Path::new(&project).is_absolute() {
            return error(
                StatusCode::BAD_REQUEST,
                "invalid_project",
                "the project folder must be a full path",
            );
        }
        args.push("--project".into());
        args.push(project.to_string_lossy().into_owned());
    }
    if let Some(dcc_path) = body.dcc_path {
        if !dcc_path.is_absolute() {
            return error(
                StatusCode::BAD_REQUEST,
                "invalid_dcc_path",
                "the app location must be a full path",
            );
        }
        args.push("--dcc-path".into());
        args.push(dcc_path.to_string_lossy().into_owned());
    }
    args.extend(["--execute".into(), "--json".into()]);
    let Ok(_guard) = install_lock().try_lock() else {
        return error(
            StatusCode::CONFLICT,
            "install_in_progress",
            "another connector is being installed",
        );
    };
    respond(run_cli(&args, INSTALL_TIMEOUT).await)
}

/// `GET /admin/api/agents`
pub async fn handle_agents() -> Response {
    respond(
        run_cli(
            &["agent-skill".into(), "--agent".into(), "*".into()],
            READ_TIMEOUT,
        )
        .await,
    )
}

#[derive(Debug, Deserialize)]
pub struct AgentSkillBody {
    pub agents: Vec<String>,
    #[serde(default)]
    pub remove: bool,
}

/// `POST /admin/api/agents/skill`
pub async fn handle_agent_skill(Json(body): Json<AgentSkillBody>) -> Response {
    if body.agents.is_empty() || !body.agents.iter().all(|id| valid_id(id)) {
        return error(StatusCode::BAD_REQUEST, "invalid_agent", "unknown agent");
    }
    let mut args = vec!["agent-skill".to_string()];
    for agent in body.agents {
        args.push("--agent".into());
        args.push(agent);
    }
    args.push("--execute".into());
    if body.remove {
        args.push("--remove".into());
    }
    respond(run_cli(&args, READ_TIMEOUT).await)
}

#[derive(Debug, Deserialize)]
pub struct ReportBody {
    /// The user's own words, 1 to 4000 characters.
    pub summary: String,
    /// Send the report; without it the CLI only previews the payload.
    #[serde(default)]
    pub send: bool,
}

/// `POST /admin/api/report`: the CLI's preview or send result, as JSON.
pub async fn handle_report(Json(body): Json<ReportBody>) -> Response {
    // 4000 is `meshy-dcc report`'s own limit; NUL cannot be passed as an argument.
    if body.summary.trim().is_empty()
        || body.summary.chars().count() > 4000
        || body.summary.contains('\0')
    {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_summary",
            "describe the problem in 1 to 4000 characters",
        );
    }
    let mut args = vec![
        "report".to_string(),
        "--summary".into(),
        body.summary,
        "--source".into(),
        "ui".into(),
    ];
    if body.send {
        args.push("--yes".into());
    }
    match run_cli(&args, READ_TIMEOUT).await {
        Ok(report) => {
            let sent = !matches!(report["status"].as_str(), Some("failed" | "unknown"));
            let status = if sent {
                StatusCode::OK
            } else {
                StatusCode::BAD_GATEWAY
            };
            (status, Json(report)).into_response()
        }
        Err(response) => response,
    }
}

/// One update at a time: two would race on the same staged files.
fn update_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

/// `POST /admin/api/update`: the "Update now" button.
///
/// Stages the CLI and the server through the CLI, as `update apply --yes`
/// does, then restarts this gateway from a detached `meshy-dcc gateway
/// restart`, so this reply reaches the page before the gateway stops. The
/// page then waits for `/admin/api/health` to report `target_version`.
/// - `restarting` (202): staged, restart started.
/// - `staged`: staged, but an app started this gateway, so it is not stopped.
/// - `up_to_date`: this gateway already runs the latest release.
/// - `update_failed` (502): the CLI's result says why; nothing was restarted.
pub async fn handle_update(State(s): State<AdminState>) -> Response {
    let Ok(_guard) = update_lock().try_lock() else {
        return error(
            StatusCode::CONFLICT,
            "update_in_progress",
            "an update is already running",
        );
    };
    let base_url = format!("http://127.0.0.1:{}", s.gateway.own_port);
    let args = [
        "--base-url",
        &base_url,
        "update",
        "apply",
        "--yes",
        "--no-restart",
    ]
    .map(String::from);
    let update = match run_cli(&args, UPDATE_TIMEOUT).await {
        Ok(update) => update,
        Err(response) => return response,
    };
    let server = update["server_update"]["status"].as_str();
    if update.get("error").is_some() || matches!(server, Some("failed" | "unknown")) {
        let message = update["server_update"]["message"]
            .as_str()
            .or(update["message"].as_str())
            .unwrap_or("the update did not finish")
            .to_string();
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({"ok": false, "error": "update_failed", "message": message, "update": update})),
        )
            .into_response();
    }
    let target = update["server_update"]["latest_version"]
        .as_str()
        .or(update["latest_version"].as_str())
        .map(str::to_owned);
    if target.as_deref() == Some(s.gateway.server_version.as_str()) {
        return Json(json!({"ok": true, "status": "up_to_date", "update": update})).into_response();
    }
    // Stopping a gateway an app started would stop that app's server.
    if s.gateway.adapter_dcc.as_deref() != Some("gateway") {
        return Json(json!({
            "ok": true,
            "status": "staged",
            "started_by": s.gateway.adapter_dcc,
            "target_version": target,
            "update": update,
        }))
        .into_response();
    }
    if let Err(err) = spawn_gateway_restart(&s, &base_url) {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({
                "ok": false,
                "error": "restart_failed",
                "message": err,
                "target_version": target,
                "update": update,
            })),
        )
            .into_response();
    }
    (
        StatusCode::ACCEPTED,
        Json(json!({"ok": true, "status": "restarting", "target_version": target, "update": update})),
    )
        .into_response()
}

/// Start `meshy-dcc gateway restart` for this gateway, with its own idle
/// and network settings, in a process that outlives it.
fn spawn_gateway_restart(s: &AdminState, base_url: &str) -> Result<(), String> {
    let cli = cli_path().ok_or("meshy-dcc was not found next to the gateway")?;
    let idle = if s.gateway.gateway_persist {
        0
    } else {
        s.gateway.gateway_idle_timeout_secs
    };
    let (remote_host, remote_port) = s
        .remote_listener
        .map_or(("127.0.0.1".to_string(), 0), |addr| {
            (addr.ip().to_string(), addr.port())
        });
    let mut command = std::process::Command::new(&cli);
    command
        .args(["--output", "json", "--non-interactive", "--base-url", base_url])
        .args(["gateway", "restart", "--port"])
        .arg(s.gateway.own_port.to_string())
        .arg("--gateway-idle-timeout-secs")
        .arg(idle.to_string())
        .args(["--remote-host", &remote_host, "--remote-port"])
        .arg(remote_port.to_string())
        // The successor may wait out TCP TIME_WAIT on the port, as auto-start does.
        .args(["--wait-timeout-secs", "90"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        // Its own process group, so stopping this gateway does not stop it.
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        command.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    let mut child = command
        .spawn()
        .map_err(|err| format!("could not run {}: {err}", cli.display()))?;
    // Reap it if this gateway outlives it.
    std::thread::spawn(move || child.wait());
    Ok(())
}

/// Each field is optional; only the ones present change.
#[derive(Debug, Deserialize)]
pub struct SettingsBody {
    pub update_check: Option<bool>,
    pub telemetry: Option<bool>,
    /// The window's telemetry banner was dismissed.
    pub telemetry_notice_seen: Option<bool>,
}

/// `GET /admin/api/settings`: the user's preferences shared with the CLI.
/// `telemetry` is the saved choice; `telemetry_off_reason` names what turns it
/// off regardless of it (`DO_NOT_TRACK`, `MESHY_DCC_TELEMETRY`, `ci`, ...).
pub async fn handle_settings() -> Response {
    use crate::gateway::telemetry;
    let off_reason = telemetry::disabled_reason().filter(|reason| *reason != "setting");
    Json(json!({
        "update_check": meshy_dcc_updater::settings::update_check_enabled(),
        "telemetry": meshy_dcc_updater::settings::get_bool(telemetry::SETTING, true),
        "telemetry_off_reason": off_reason,
        "telemetry_notice": telemetry::enabled() && !telemetry::notice_seen("window"),
        "telemetry_docs": telemetry::DOCS_URL,
    }))
    .into_response()
}

/// `PUT /admin/api/settings`
pub async fn handle_settings_update(Json(body): Json<SettingsBody>) -> Response {
    use crate::gateway::telemetry;
    let mut result = Ok(());
    if let Some(on) = body.update_check {
        result = result.and(meshy_dcc_updater::settings::set_update_check(on));
    }
    if let Some(on) = body.telemetry {
        result = result.and(telemetry::set_enabled(on));
    }
    // Choosing either way also answers the banner.
    if body.telemetry_notice_seen == Some(true) || body.telemetry.is_some() {
        telemetry::mark_notice("window");
    }
    match result {
        Ok(()) => handle_settings().await,
        Err(err) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "settings_write_failed",
            format!("{err:#}"),
        ),
    }
}

/// `GET /admin/api/telemetry/preview`: the exact body the next daily send
/// would post. Sends nothing.
pub async fn handle_telemetry_preview(
    axum::extract::State(s): axum::extract::State<super::super::state::AdminState>,
) -> Response {
    use crate::gateway::telemetry;
    Json(json!({
        "enabled": telemetry::enabled(),
        "disabled_reason": telemetry::disabled_reason(),
        "host": telemetry::host(),
        "body": telemetry::preview(&s, telemetry::now_ms()).await,
    }))
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_plain_names() {
        assert!(valid_id("blender"));
        assert!(valid_id("substance3d_designer"));
        assert!(valid_id("claude-code"));
        assert!(!valid_id("*"));
        assert!(!valid_id("--execute"));
        assert!(!valid_id("../x"));
        assert!(!valid_id(""));
    }
}
