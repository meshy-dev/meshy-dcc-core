//! Read-only public observations run without gateway auto-start or project setup.

mod support;

use serde_json::json;

#[test]
fn software_and_preparation_plan_are_project_free_and_do_not_create_runtime_state() {
    let root = tempfile::tempdir().unwrap();
    let catalog = root.path().join("catalog.json");
    let executable = root.path().join("Godot");
    std::fs::write(&executable, "fixture").unwrap();
    std::fs::write(&catalog, json!({"entries":[{
        "name":"meshy-dcc-godot","description":"fixture","dcc":["godot"],"tags":["adapter"],"version":"1.0.0",
        "policy":{"installation":"available"},"install":{"type":"host-package","url":"https://example.invalid/fixture.zip","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "host":{"extract":true,"dcc_paths":[executable],"software_homepage":"https://godotengine.org/","project_steps":[["python3","{package_dir}/install.py","--project","{project}"]]}}
    }]}).to_string()).unwrap();
    let registry = root.path().join("registry");
    let data = root.path().join("data");
    let cache = root.path().join("invalid-default-cache.json");
    std::fs::write(&cache, "untrusted cache must not affect explicit catalogs").unwrap();
    let envs = [
        ("MESHY_DCC_REGISTRY_DIR", registry.to_str().unwrap()),
        ("MESHY_DCC_DATA_DIR", data.to_str().unwrap()),
        ("MESHY_DCC_INSTALL_CACHE", cache.to_str().unwrap()),
    ];
    let software = support::run_json_with_env(
        &["software", "--catalog", catalog.to_str().unwrap(), "--json"],
        &envs,
    );
    assert_eq!(software["software"][0]["name"], "Godot");
    assert_eq!(software["software"][0]["detection"], "found");
    assert_eq!(software["software"][0]["adapter"]["prepared"], false);
    assert!(software["software"][0].get("project").is_none());
    let plan = support::run_json_with_env(
        &[
            "prepare-adapter",
            "--dcc-type",
            "godot",
            "--catalog",
            catalog.to_str().unwrap(),
            "--json",
        ],
        &envs,
    );
    assert_eq!(plan["status"], "planned");
    assert!(!registry.exists());
    assert!(!data.exists());
    assert_eq!(
        std::fs::read_to_string(cache).unwrap(),
        "untrusted cache must not affect explicit catalogs"
    );
}

#[test]
fn software_rejects_an_invalid_default_catalog_without_repairing_or_changing_state() {
    let root = tempfile::tempdir().unwrap();
    let cache = root.path().join("catalog.json");
    let data = root.path().join("data");
    let registry = root.path().join("registry");
    std::fs::write(&cache, "invalid cached catalog").unwrap();
    std::fs::create_dir(&data).unwrap();
    std::fs::create_dir(&registry).unwrap();
    let receipt = data.join("existing-install-receipt.json");
    let launch = registry.join("existing-launch.json");
    std::fs::write(&receipt, "existing installation").unwrap();
    std::fs::write(&launch, "existing launch state").unwrap();
    let output = support::cli_command()
        .args(["software", "--json"])
        .env_remove("MESHY_DCC_CATALOG_PATH")
        .env("MESHY_DCC_INSTALL_CACHE", &cache)
        .env("MESHY_DCC_DATA_DIR", &data)
        .env("MESHY_DCC_REGISTRY_DIR", &registry)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("installation catalog rejected"));
    assert_eq!(
        std::fs::read_to_string(&cache).unwrap(),
        "invalid cached catalog"
    );
    assert!(!cache.with_extension("lock").exists());
    assert_eq!(
        std::fs::read_to_string(receipt).unwrap(),
        "existing installation"
    );
    assert_eq!(
        std::fs::read_to_string(launch).unwrap(),
        "existing launch state"
    );
    assert_eq!(std::fs::read_dir(data).unwrap().count(), 1);
    assert_eq!(std::fs::read_dir(registry).unwrap().count(), 1);
}

#[test]
fn unavailable_gateway_activity_is_unknown_without_starting_services() {
    let root = tempfile::tempdir().unwrap();
    let registry = root.path().join("registry");
    let output = support::run_json_with_env(
        &[
            "--base-url",
            "http://127.0.0.1:1",
            "active-work",
            "--agent-name",
            "fixture-client",
            "--json",
        ],
        &[("MESHY_DCC_REGISTRY_DIR", registry.to_str().unwrap())],
    );
    assert_eq!(output["state"], "unknown");
    assert_eq!(output["scope"]["agent_name"], "fixture-client");
    assert!(
        output["unknown_instances"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "gateway_calls")
    );
    assert!(!registry.exists());
}
