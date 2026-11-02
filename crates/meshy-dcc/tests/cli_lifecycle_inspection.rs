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
    let envs = [
        ("MESHY_DCC_REGISTRY_DIR", registry.to_str().unwrap()),
        ("MESHY_DCC_DATA_DIR", data.to_str().unwrap()),
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
    assert!(!data.join("installs/prepared/godot.json").exists());
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
