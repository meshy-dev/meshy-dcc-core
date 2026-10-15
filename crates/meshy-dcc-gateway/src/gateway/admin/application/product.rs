//! Endpoints behind the user-facing pages of `meshy-dcc ui`: connectors and
//! the agent Skill.
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

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use axum::Json;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};

/// Overrides the CLI the gateway runs (tests, unusual layouts).
pub const CLI_BIN_ENV: &str = "MESHY_DCC_CLI_BIN";

const READ_TIMEOUT: Duration = Duration::from_secs(60);
// ponytail: one fixed ceiling; downloads of a large adapter on a slow link may need more.
const INSTALL_TIMEOUT: Duration = Duration::from_secs(600);

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
pub struct SettingsBody {
    pub update_check: bool,
}

/// `GET /admin/api/settings`: the user's preferences shared with the CLI.
pub async fn handle_settings() -> Response {
    Json(json!({ "update_check": meshy_dcc_updater::settings::update_check_enabled() }))
        .into_response()
}

/// `PUT /admin/api/settings`
pub async fn handle_settings_update(Json(body): Json<SettingsBody>) -> Response {
    match meshy_dcc_updater::settings::set_update_check(body.update_check) {
        Ok(()) => handle_settings().await,
        Err(err) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "settings_write_failed",
            &format!("{err:#}"),
        ),
    }
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
