use std::process::{Command, Output};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{Value, json};
use tempfile::TempDir;

fn cli_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_meshy-dcc"))
}

/// `Command::output()` as an agent sees it: done only when stdout and stderr
/// reach EOF, which a detached child holding the pipes would postpone.
fn output_within(mut command: Command, limit: Duration) -> Output {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || tx.send(command.output()));
    rx.recv_timeout(limit)
        .expect("CLI output stayed open: a child it started holds the pipes")
        .unwrap()
}

fn unused_loopback_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

struct AutoGatewayCleanup<'a> {
    host: &'a str,
    port: u16,
    envs: &'a [(&'a str, &'a str)],
}

impl Drop for AutoGatewayCleanup<'_> {
    fn drop(&mut self) {
        let mut command = cli_command();
        let port_s = self.port.to_string();
        command.args([
            "--no-auto-gateway",
            "gateway",
            "stop",
            "--host",
            self.host,
            "--port",
            port_s.as_str(),
        ]);
        for (key, value) in self.envs {
            command.env(key, value);
        }
        let _ = command.output();
    }
}

#[test]
fn list_auto_starts_builtin_local_gateway() {
    let port = unused_loopback_port();
    let base_url = format!("http://127.0.0.1:{port}");
    let registry = TempDir::new().unwrap();
    let registry_s = registry.path().to_string_lossy().to_string();
    let cli_bin = env!("CARGO_BIN_EXE_meshy-dcc");
    let envs = [
        ("MESHY_DCC_REGISTRY_DIR", registry_s.as_str()),
        ("MESHY_DCC_GATEWAY_IDLE_TIMEOUT_SECS", "1"),
    ];
    let _cleanup = AutoGatewayCleanup {
        host: "127.0.0.1",
        port,
        envs: &envs,
    };

    let output = {
        let mut command = cli_command();
        command.args([
            "--base-url",
            &base_url,
            "--auto-gateway-bin",
            cli_bin,
            "--auto-gateway-timeout-secs",
            "15",
            "list",
        ]);
        for (key, value) in &envs {
            command.env(key, value);
        }
        output_within(command, Duration::from_secs(60))
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stderr.contains("auto-started gateway"),
        "list should auto-start the local gateway before inventory: {stderr}"
    );
    let inventory: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(inventory["gateway"]["current"]["host"], "127.0.0.1");
    assert_eq!(inventory["gateway"]["current"]["port"], json!(port));
    assert!(inventory["instances"].as_array().is_some());
}
