//! `install` for `host-package` catalog entries: download, verify, run host commands.
#![cfg(unix)]

mod support;

use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Output;

use serde_json::Value;
use support::*;
use tempfile::TempDir;

/// Serve `bytes` for every request on a loopback port.
fn serve(bytes: Vec<u8>) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut request = Vec::new();
            let mut buf = [0_u8; 1024];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                match stream.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => request.extend_from_slice(&buf[..n]),
                }
            }
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                bytes.len()
            );
            let _ = stream.write_all(&bytes);
        }
    });
    format!("http://{addr}/meshy_dcc_testhost.zip")
}

/// A stand-in host that logs its argv and copies the `-e` package.
fn fake_host(temp: &TempDir) -> PathBuf {
    let path = temp.path().join("fake-host");
    std::fs::write(
        &path,
        "#!/bin/sh\necho \"$*\" >> \"$HOST_LOG\"\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"-e\" ]; then cp \"$2\" \"$HOST_COPY\"; fi\n  shift\ndone\n",
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn catalog(temp: &TempDir, url: &str, sha: &str) -> PathBuf {
    let path = temp.path().join("catalog.yml");
    std::fs::write(
        &path,
        format!(
            r#"version: "1"
entries:
  - name: "meshy-dcc-testhost"
    description: "test host adapter"
    dcc: ["testhost"]
    tags: ["adapter"]
    version: "0.1.0"
    install:
      type: host-package
      url: "{url}"
      sha256: "{sha}"
      host:
        uninstall: [["{{dcc_path}}", "remove", "testhost"]]
        steps: [["{{dcc_path}}", "install", "-e", "{{package}}"]]
        launch: ["{{executable}}"]
"#
        ),
    )
    .unwrap();
    path
}

fn install(temp: &TempDir, catalog: &Path, dcc_path: Option<&Path>) -> Output {
    let mut command = cli_command();
    command
        .args(["install", "--dcc-type", "testhost", "--catalog"])
        .arg(catalog)
        .args(["--execute", "--json"]);
    if let Some(path) = dcc_path {
        command.arg("--dcc-path").arg(path);
    }
    command
        .env_remove("DCC_MCP_INSTALL_DISABLED")
        .env("DCC_MCP_REGISTRY_DIR", temp.path().join("registry"))
        .env("HOST_LOG", temp.path().join("host.log"))
        .env("HOST_COPY", temp.path().join("host-copy.zip"))
        .output()
        .unwrap()
}

fn package(temp: &TempDir) -> Vec<u8> {
    write_zip(
        &[("blender_manifest.toml", "id = \"testhost\"\n")],
        &temp.path().join("pkg.zip"),
    )
}

#[test]
fn host_package_runs_uninstall_then_install_and_writes_launch_plan() {
    let temp = TempDir::new().unwrap();
    let bytes = package(&temp);
    let url = serve(bytes.clone());
    let catalog = catalog(&temp, &url, &sha256_hex(&bytes));
    let host = fake_host(&temp);

    let output = install(&temp, &catalog, Some(&host));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={stdout} stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let steps: Vec<_> = report["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| {
            (
                step["id"].as_str().unwrap(),
                step["status"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        steps,
        [
            ("install-host-package", "ok"),
            ("write-launch-plan", "ok"),
            ("register-dcc", "deferred"),
            ("verify", "ok"),
        ]
    );

    let log = std::fs::read_to_string(temp.path().join("host.log")).unwrap();
    let lines: Vec<_> = log.lines().collect();
    assert_eq!(lines[0], "remove testhost");
    assert!(
        lines[1].starts_with("install -e /") && lines[1].ends_with("/meshy_dcc_testhost.zip"),
        "{log}"
    );
    assert_eq!(
        std::fs::read(temp.path().join("host-copy.zip")).unwrap(),
        bytes
    );

    let plan: Value = serde_json::from_str(
        &std::fs::read_to_string(temp.path().join("registry/launch-plans/testhost.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(plan["executable"], host.to_string_lossy().as_ref());
    assert_eq!(plan["argv"], serde_json::json!(["{executable}"]));
}

#[test]
fn host_package_with_wrong_checksum_runs_no_host_command() {
    let temp = TempDir::new().unwrap();
    let url = serve(package(&temp));
    let catalog = catalog(&temp, &url, &"0".repeat(64));

    let output = install(&temp, &catalog, Some(&fake_host(&temp)));
    assert_eq!(
        output.status.code(),
        Some(30),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(!temp.path().join("host.log").exists());
    assert!(
        !temp
            .path()
            .join("registry/launch-plans/testhost.json")
            .exists()
    );
}

#[test]
fn host_package_refuses_remote_plain_http_and_missing_dcc_path() {
    let temp = TempDir::new().unwrap();
    let sha = "0".repeat(64);
    let remote = catalog(&temp, "http://example.com/meshy_dcc_testhost.zip", &sha);
    let output = install(&temp, &remote, Some(&fake_host(&temp)));
    assert_eq!(output.status.code(), Some(10));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["error"]["code"], "INSTALL_PLAN_FAILED");

    let local = catalog(&temp, "http://127.0.0.1:9/meshy_dcc_testhost.zip", &sha);
    let output = install(&temp, &local, None);
    assert_eq!(
        output.status.code(),
        Some(10),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(!temp.path().join("host.log").exists());
}
