//! Gateway lifecycle commands for `meshy-dcc`.
//!
//! Owns gateway auto-start for a command, the gateway endpoint decision, and
//! the `gateway` subcommand tree plus its request conversions.

use std::path::PathBuf;

use serde_json::Value;

use crate::application::gateway_ctrl;
use crate::application::gateway_ensure;
use crate::application::gateway_profile::{self, GatewayTarget};
use crate::domain::rest::Endpoint;

use super::cli_args::{
    Command, GatewayAction, GatewayDaemonAction, GatewayStartArgs, GatewayStatusArgs,
    GatewayStopArgs,
};

use crate::presentation::marketplace_cmd::MarketplaceAction;

pub(crate) async fn ensure_gateway_for_command(
    base_url: &str,
    command: &Command,
    gateway_target: &GatewayTarget,
    gateway_bin: Option<PathBuf>,
    wait_timeout_secs: u64,
) -> anyhow::Result<()> {
    let Some(endpoint) = gateway_endpoint_for_command(base_url, command, gateway_target) else {
        return Ok(());
    };
    let ensured =
        gateway_ctrl::ensure_local_gateway_for_endpoint(&endpoint, gateway_bin, wait_timeout_secs)
            .await;
    let result = match ensured {
        Ok(Some(result)) => result,
        Ok(None) => return Ok(()),
        // The host can still start and register without a gateway; agent
        // commands bring it up later.
        Err(error) if matches!(command, Command::StartInstance { .. }) => {
            eprintln!("warning: gateway not started before the host: {error:#}");
            return Ok(());
        }
        Err(error) => return Err(error),
    };

    if !result.already_running {
        if let Some(pid) = result.pid {
            eprintln!(
                "info: auto-started gateway at http://{}:{} (pid {pid})",
                result.host, result.port
            );
        } else {
            eprintln!(
                "info: auto-started gateway at http://{}:{}",
                result.host, result.port
            );
        }
    }
    Ok(())
}

pub(crate) fn gateway_endpoint_for_command(
    base_url: &str,
    command: &Command,
    _gateway_target: &GatewayTarget,
) -> Option<Endpoint> {
    match command {
        Command::Smoke { url: None, .. } => Some(Endpoint::new(base_url)),
        Command::Smoke { url: Some(_), .. } => None,
        Command::Health | Command::Ui { .. } | Command::Stats { .. } | Command::Update { .. } => {
            Some(Endpoint::new(base_url))
        }
        Command::Feedback(args) => args.requires_gateway().then_some(Endpoint::new(base_url)),
        // A report reads findings only from a gateway that is already running.
        Command::Doctor { .. } | Command::DccTypes { .. } | Command::Report(_) => None,
        Command::List
        | Command::Search { .. }
        | Command::Describe { .. }
        | Command::LoadSkill { .. }
        | Command::Call { .. }
        | Command::CallBatch { .. }
        | Command::UiControl { .. }
        | Command::RecordReplay { .. }
        | Command::WaitReady { .. }
        | Command::ReloadSkills { .. }
        | Command::StopInstance { .. } => Some(Endpoint::new(base_url)),
        Command::Marketplace {
            action: MarketplaceAction::Install { reload: true, .. },
        }
        | Command::Marketplace {
            action: MarketplaceAction::Uninstall { reload: true, .. },
        } => Some(Endpoint::new(base_url)),
        // Local mode still executes these commands through FileRegistry/direct
        // MCP where that is the richer path, but the CLI owns gateway
        // lifecycle by default so agents can rely on the admin/control plane.
        Command::Install { .. }
        | Command::Marketplace { .. }
        | Command::Lint(_)
        | Command::AgentSkill(_)
        | Command::Components { .. }
        | Command::Gateway { .. }
        | Command::StartInstance { dry_run: true, .. } => None,
        // `start-instance` owns the local lifecycle and never routes through
        // the gateway, but it brings the gateway up first so the host it
        // launches registers behind it instead of hosting one itself.
        Command::StartInstance { .. } => Some(Endpoint::new(base_url)),
    }
}

/// Host and port the gateway subcommands act on when `--host`/`--port` are
/// omitted: the local gateway every other command talks to, so `--base-url`,
/// `MESHY_DCC_BASE_URL` and `MESHY_DCC_GATEWAY_PORT` reach `gateway stop`
/// and `gateway status` too. A remote target falls back to the local default.
pub(crate) struct GatewayAddress {
    host: String,
    port: u16,
}

impl GatewayAddress {
    pub(crate) fn for_base_url(base_url: &str) -> Self {
        let (host, port) = gateway_ctrl::local_auto_gateway_target(&Endpoint::new(base_url))
            .unwrap_or_else(|| ("127.0.0.1".to_string(), 19765));
        Self { host, port }
    }

    fn resolve(&self, host: Option<String>, port: Option<u16>) -> (String, u16) {
        (
            host.unwrap_or_else(|| self.host.clone()),
            port.unwrap_or(self.port),
        )
    }
}

pub(crate) async fn run_gateway_cmd(
    base_url: &str,
    action: GatewayAction,
    profile_path: &std::path::Path,
) -> anyhow::Result<Value> {
    let address = GatewayAddress::for_base_url(base_url);
    match action {
        GatewayAction::Register { url, name } => {
            gateway_profile::register_profile(profile_path, name, url)
        }
        GatewayAction::List => gateway_profile::list_profiles(profile_path),
        GatewayAction::Set { name } => gateway_profile::set_current_profile(profile_path, name),
        GatewayAction::Daemon { action } => {
            gateway_ctrl::run_gateway_daemon(gateway_daemon_request(action, &address)).await
        }
        GatewayAction::Ensure(args) => {
            let request = start_request(args, &address);
            let reg = request
                .registry_dir
                .clone()
                .unwrap_or_else(gateway_ensure::default_registry_dir);
            let args = gateway_ensure::EnsureGatewayArgs {
                host: request.host,
                port: request.port,
                name: request.name,
                registry_dir: reg,
                remote_host: request.remote_host,
                remote_port: request.remote_port,
                gateway_idle_timeout_secs: request.gateway_idle_timeout_secs,
                gateway_bin: request.gateway_bin,
                wait_timeout_secs: request.wait_timeout_secs,
                pidfile: None,
            };
            let result = gateway_ensure::ensure_gateway_running(&args).await?;
            Ok(serde_json::to_value(result)?)
        }
        GatewayAction::Start(args) => {
            gateway_ctrl::run_gateway_daemon(gateway_ctrl::GatewayDaemonRequest::Start(
                start_request(args, &address),
            ))
            .await
        }
        GatewayAction::Stop(args) => {
            gateway_ctrl::run_gateway_daemon(gateway_ctrl::GatewayDaemonRequest::Stop(
                stop_request(args, &address),
            ))
            .await
        }
        GatewayAction::Status(args) => {
            gateway_ctrl::run_gateway_daemon(gateway_ctrl::GatewayDaemonRequest::Status(
                status_request(args, &address),
            ))
            .await
        }
    }
}

pub(crate) fn gateway_daemon_request(
    action: GatewayDaemonAction,
    address: &GatewayAddress,
) -> gateway_ctrl::GatewayDaemonRequest {
    match action {
        GatewayDaemonAction::Start(args) => {
            gateway_ctrl::GatewayDaemonRequest::Start(start_request(args, address))
        }
        GatewayDaemonAction::Restart(args) => gateway_ctrl::GatewayDaemonRequest::Restart {
            start: start_request(args.start, address),
            stop_timeout_secs: args.stop_timeout_secs,
        },
        GatewayDaemonAction::Stop(args) => {
            gateway_ctrl::GatewayDaemonRequest::Stop(stop_request(args, address))
        }
        GatewayDaemonAction::Status(args) => {
            gateway_ctrl::GatewayDaemonRequest::Status(status_request(args, address))
        }
    }
}

fn start_request(
    args: GatewayStartArgs,
    address: &GatewayAddress,
) -> gateway_ctrl::GatewayDaemonStartRequest {
    let (host, port) = address.resolve(args.host, args.port);
    gateway_ctrl::GatewayDaemonStartRequest {
        host,
        port,
        name: args.name,
        registry_dir: args.registry_dir,
        remote_host: args.remote_host,
        remote_port: args.remote_port,
        gateway_idle_timeout_secs: args.gateway_idle_timeout_secs,
        gateway_bin: args.gateway_bin,
        wait_timeout_secs: args.wait_timeout_secs,
    }
}

fn stop_request(
    args: GatewayStopArgs,
    address: &GatewayAddress,
) -> gateway_ctrl::GatewayDaemonStopRequest {
    let (host, port) = address.resolve(args.host, args.port);
    gateway_ctrl::GatewayDaemonStopRequest {
        host,
        port,
        registry_dir: args.registry_dir,
        wait_timeout_secs: args.wait_timeout_secs,
    }
}

fn status_request(
    args: GatewayStatusArgs,
    address: &GatewayAddress,
) -> gateway_ctrl::GatewayDaemonStatusRequest {
    let (host, port) = address.resolve(args.host, args.port);
    gateway_ctrl::GatewayDaemonStatusRequest {
        host,
        port,
        registry_dir: args.registry_dir,
    }
}
