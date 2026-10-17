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
        .env_remove("MESHY_DCC_INSTALL_DISABLED")
        .env("MESHY_DCC_REGISTRY_DIR", temp.path().join("registry"))
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
fn host_package_failure_carries_the_installer_report_from_stdout() {
    let temp = TempDir::new().unwrap();
    let host = temp.path().join("failing-host");
    std::fs::write(
        &host,
        "#!/bin/sh\necho '{\"status\":\"failed\",\"reason\":\"ownership_conflict\"}'\nexit 30\n",
    )
    .unwrap();
    std::fs::set_permissions(&host, std::fs::Permissions::from_mode(0o755)).unwrap();
    let bytes = package(&temp);
    let sha = sha256_hex(&bytes);
    let catalog = catalog(&temp, &serve(bytes), &sha);

    let output = install(&temp, &catalog, Some(&host));

    assert!(!output.status.success());
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(text.contains("ownership_conflict"), "{text}");
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

/// A package whose `bin/setup` logs its argv, optionally with a symlink entry.
fn extract_package(temp: &TempDir, symlink: bool) -> Vec<u8> {
    let dest = temp.path().join("extract-pkg.zip");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&dest).unwrap());
    let options = zip::write::SimpleFileOptions::default().unix_permissions(0o755);
    zip.start_file("bin/setup", options).unwrap();
    zip.write_all(b"#!/bin/sh\necho \"$*\" >> \"$HOST_LOG\"\n")
        .unwrap();
    if symlink {
        zip.add_symlink("bin/escape", "/etc", options).unwrap();
    }
    zip.finish().unwrap();
    std::fs::read(dest).unwrap()
}

fn extract_catalog(temp: &TempDir, url: &str, sha: &str) -> PathBuf {
    let path = temp.path().join("extract-catalog.yml");
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
        extract: true
        project_steps: [["{{package_dir}}/bin/setup", "--project", "{{project}}"]]
"#
        ),
    )
    .unwrap();
    path
}

fn install_project(temp: &TempDir, catalog: &Path, project: Option<&Path>) -> Output {
    let mut command = cli_command();
    command
        .args(["install", "--dcc-type", "testhost", "--catalog"])
        .arg(catalog)
        .args(["--execute", "--json"]);
    if let Some(project) = project {
        command.arg("--project").arg(project);
    }
    command
        .env_remove("MESHY_DCC_INSTALL_DISABLED")
        .env("MESHY_DCC_REGISTRY_DIR", temp.path().join("registry"))
        .env("MESHY_DCC_DATA_DIR", temp.path().join("data"))
        .env("HOST_LOG", temp.path().join("host.log"))
        .output()
        .unwrap()
}

#[test]
fn host_package_extracts_once_and_runs_project_steps_from_the_package() {
    let temp = TempDir::new().unwrap();
    let bytes = extract_package(&temp, false);
    let catalog = extract_catalog(&temp, &serve(bytes.clone()), &sha256_hex(&bytes));
    let project = temp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let package_dir = temp
        .path()
        .join("data/adapters/meshy-dcc-testhost/0.1.0/meshy_dcc_testhost");

    for _ in 0..2 {
        let output = install_project(&temp, &catalog, Some(&project));
        assert_eq!(
            output.status.code(),
            Some(0),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let log = std::fs::read_to_string(temp.path().join("host.log")).unwrap();
    let expected = format!("--project {}", project.display());
    assert_eq!(log.lines().collect::<Vec<_>>(), [&expected, &expected]);
    assert!(package_dir.join("bin/setup").is_file());
    assert_eq!(
        std::fs::read_to_string(package_dir.join(".meshy-dcc-package.sha256")).unwrap(),
        sha256_hex(&bytes)
    );
}

#[test]
fn host_package_project_steps_need_a_project_and_no_symlinks() {
    let temp = TempDir::new().unwrap();
    let bytes = extract_package(&temp, false);
    let catalog = extract_catalog(&temp, &serve(bytes.clone()), &sha256_hex(&bytes));
    let output = install_project(&temp, &catalog, None);
    assert_eq!(
        output.status.code(),
        Some(10),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );

    let bytes = extract_package(&temp, true);
    let catalog = extract_catalog(&temp, &serve(bytes.clone()), &sha256_hex(&bytes));
    let project = temp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let output = install_project(&temp, &catalog, Some(&project));
    assert_eq!(output.status.code(), Some(30));
    assert!(String::from_utf8_lossy(&output.stdout).contains("symlink"));
    assert!(!temp.path().join("host.log").exists());
    assert!(
        !temp
            .path()
            .join("data/adapters/meshy-dcc-testhost/meshy_dcc_testhost")
            .exists()
    );
}

/// A catalog whose build for this platform lists `url` then `mirrors`.
fn mirrored_catalog(temp: &TempDir, url: &str, mirrors: &[String], sha: &str) -> PathBuf {
    let platform = meshy_dcc_updater::meshy_release::current_platform().unwrap();
    let mirrors = mirrors
        .iter()
        .map(|mirror| format!("\"{mirror}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let path = temp.path().join("mirrored.yml");
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
      platforms:
        {platform}:
          url: "{url}"
          sha256: "{sha}"
          mirrors: [{mirrors}]
      host:
        steps: [["{{dcc_path}}", "install", "-e", "{{package}}"]]
        launch: ["{{executable}}"]
"#
        ),
    )
    .unwrap();
    path
}

fn closed_port_url() -> String {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    format!("http://127.0.0.1:{port}/meshy_dcc_testhost.zip")
}

#[test]
fn host_package_falls_back_to_a_mirror_when_the_first_url_is_unreachable() {
    let temp = TempDir::new().unwrap();
    let bytes = package(&temp);
    let sha = sha256_hex(&bytes);
    let catalog = mirrored_catalog(&temp, &closed_port_url(), &[serve(bytes)], &sha);

    let output = install(&temp, &catalog, Some(&fake_host(&temp)));

    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        temp.path().join("host-copy.zip").exists(),
        "the host received the package"
    );
}

#[test]
fn host_package_does_not_try_a_mirror_after_a_failed_checksum() {
    let temp = TempDir::new().unwrap();
    let bytes = package(&temp);
    let sha = sha256_hex(&bytes);
    let tampered = serve(b"not the package".to_vec());
    let catalog = mirrored_catalog(&temp, &tampered, &[serve(bytes)], &sha);

    let output = install(&temp, &catalog, Some(&fake_host(&temp)));

    assert!(!output.status.success());
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(text.contains("SHA-256 mismatch"), "{text}");
    assert!(!temp.path().join("host-copy.zip").exists());
}
