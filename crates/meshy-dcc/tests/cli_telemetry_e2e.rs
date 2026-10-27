//! Usage telemetry from the CLI: the first-run notice, the switches, and the
//! separation from bug reports. Every run gets its own HOME and data dirs and
//! never reaches a gateway, so nothing is sent.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

const NOTICE_TEXT: &str = "anonymous, aggregate usage counts";

fn cli(home: &Path, envs: &[(&str, &str)], args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_meshy-dcc"));
    command
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env(
            "MESHY_DCC_SETTINGS_FILE",
            home.join(".meshy-dcc/settings.json"),
        )
        .env("MESHY_DCC_DATA_DIR", home.join("data"))
        .env("MESHY_DCC_REGISTRY_DIR", home.join("registry"))
        .env("MESHY_DCC_GATEWAY_ADMIN_DB", home.join("admin.db"))
        .env("MESHY_DCC_GATEWAY_PORT", "19799")
        .env("MESHY_DCC_INSTALL_OFFLINE", "true")
        .env("MESHY_DCC_TELEMETRY_HOST", "http://127.0.0.1:9")
        // Debug builds stay quiet unless asked; these tests ask.
        .env("MESHY_DCC_TELEMETRY", "1");
    for var in [
        "DO_NOT_TRACK",
        "CI",
        "GITHUB_ACTIONS",
        "GITLAB_CI",
        "BUILDKITE",
        "TF_BUILD",
    ] {
        command.env_remove(var);
    }
    for (key, value) in envs {
        command.env(key, value);
    }
    command
        .args(["--no-auto-gateway", "--output", "json"])
        .args(args)
        .output()
        .unwrap()
}

fn json(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn notice_shows_once_and_not_when_opted_out() {
    let home = tempfile::tempdir().unwrap();
    let first = cli(home.path(), &[], &["dcc-types", "--offline"]);
    assert!(stderr(&first).contains(NOTICE_TEXT), "{}", stderr(&first));
    assert!(stderr(&first).contains("meshy-dcc telemetry off"));
    let second = cli(home.path(), &[], &["dcc-types", "--offline"]);
    assert!(!stderr(&second).contains(NOTICE_TEXT));

    for env in [
        ("DO_NOT_TRACK", "1"),
        ("MESHY_DCC_TELEMETRY", "0"),
        ("CI", "true"),
    ] {
        let quiet = tempfile::tempdir().unwrap();
        let output = cli(quiet.path(), &[env], &["dcc-types", "--offline"]);
        assert!(!stderr(&output).contains(NOTICE_TEXT), "{env:?}");
        assert!(
            !quiet.path().join(".meshy-dcc/telemetry").exists(),
            "{env:?}"
        );
    }
}

#[test]
fn switches_persist_in_settings_and_reset_makes_a_new_id() {
    let home = tempfile::tempdir().unwrap();
    let status = json(&cli(home.path(), &[], &["telemetry", "status"]));
    assert_eq!(status["enabled"], true);
    assert_eq!(status["install_id"], Value::Null);

    let first = json(&cli(home.path(), &[], &["telemetry", "reset"]));
    let id = first["install_id"].as_str().unwrap().to_string();
    assert!(uuid::Uuid::parse_str(&id).is_ok());
    let second = json(&cli(home.path(), &[], &["telemetry", "reset"]));
    assert_ne!(second["install_id"].as_str().unwrap(), id);

    let off = json(&cli(home.path(), &[], &["telemetry", "off"]));
    assert_eq!(off["enabled"], false);
    assert_eq!(off["disabled_reason"], "setting");
    let settings: Value = serde_json::from_slice(
        &std::fs::read(home.path().join(".meshy-dcc/settings.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(settings["telemetry"], false);
    let on = json(&cli(home.path(), &[], &["telemetry", "on"]));
    assert_eq!(on["enabled"], true);

    let dnt = json(&cli(
        home.path(),
        &[("DO_NOT_TRACK", "1")],
        &["telemetry", "status"],
    ));
    assert_eq!(dnt["enabled"], false);
    assert_eq!(dnt["disabled_reason"], "DO_NOT_TRACK");
}

#[test]
fn bug_report_never_carries_the_install_id() {
    let home = tempfile::tempdir().unwrap();
    let id = json(&cli(home.path(), &[], &["telemetry", "reset"]))["install_id"]
        .as_str()
        .unwrap()
        .to_string();
    // Without --yes the report is only previewed; nothing is sent.
    let report = cli(home.path(), &[], &["report", "--summary", "import failed"]);
    let text = String::from_utf8_lossy(&report.stdout);
    assert!(
        text.contains("meshy-dcc.report.v1"),
        "stdout: {text}\nstderr: {}",
        stderr(&report)
    );
    assert!(!text.contains(&id));
}
