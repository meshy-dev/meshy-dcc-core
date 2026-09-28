mod support;

use meshy_dcc_transport::discovery::file_registry::FileRegistry;
use meshy_dcc_transport::discovery::types::{ServiceEntry, ServiceStatus};
use serde_json::Value;
use tempfile::{NamedTempFile, TempDir};

use support::*;

const DISCOVERY_DECISION_SCHEMA: &str =
    include_str!("../../../contracts/dcc-discovery-decision-v1.schema.json");

fn validate_decision(value: &Value) {
    let schema: Value = serde_json::from_str(DISCOVERY_DECISION_SCHEMA).unwrap();
    jsonschema::validator_for(&schema)
        .unwrap()
        .validate(value)
        .unwrap();
}

#[test]
fn dcc_types_distinguishes_catalog_support_from_zero_live_instances() {
    let registry = TempDir::new().unwrap();
    let registry_s = registry.path().to_string_lossy().to_string();
    let value = run_json_with_env(
        &["--output", "json", "dcc-types", "--dcc-type", "blender"],
        &[("MESHY_DCC_REGISTRY_DIR", registry_s.as_str())],
    );

    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["dcc_type"], "blender");
    assert_eq!(value["live_instances"], 0);
    assert_eq!(value["public_adapter"], "present");
    assert_eq!(value["released_catalog"], "present");
    assert_eq!(value["package_installation"], "unknown");
    assert_eq!(value["adapter_import"], "unknown");
    assert_eq!(value["project_bootstrap"], "unknown");
    assert_eq!(value["registry_registration"], "absent");
    assert_eq!(value["direct_readiness"], "unknown");
    assert_eq!(value["gateway_capability_index"], "unknown");
    assert_eq!(value["search_hit"], "unknown");
    assert_eq!(value["exact_instance_call"], "not_run");
    assert_eq!(value["real_host_effect"], "not_verified");
    assert_eq!(
        value["uncertainties"],
        serde_json::json!(["version", "custom_fork", "real_host"])
    );
    // The Meshy Blender adapter is cataloged but not published, so there is
    // nothing to install yet.
    assert_eq!(value["failure_stage"], Value::Null);
    assert_eq!(value["next_action"]["id"], "inspect_catalog");
    assert_eq!(value["next_action"]["requires_consent"], false);
    assert_eq!(
        value["next_action"]["command"],
        serde_json::json!([
            "meshy-dcc",
            "--output",
            "json",
            "--non-interactive",
            "dcc-types"
        ])
    );
    assert_eq!(value["next_action"]["instructions_url"], Value::Null);

    validate_decision(&value);
}

#[test]
fn dcc_types_does_not_treat_a_missing_catalog_row_as_unsupported() {
    let registry = TempDir::new().unwrap();
    let registry_s = registry.path().to_string_lossy().to_string();
    let value = run_json_with_env(
        &["--output", "json", "dcc-types", "--dcc-type", "studio tool"],
        &[("MESHY_DCC_REGISTRY_DIR", registry_s.as_str())],
    );

    assert_eq!(value["public_adapter"], "unknown");
    assert_eq!(value["released_catalog"], "absent");
    assert_eq!(value["failure_stage"], "catalog_lookup");
    assert_eq!(value["failure_reason"], "CATALOG_ENTRY_NOT_FOUND");
    assert_eq!(value["next_action"]["id"], "inspect_catalog");
    assert!(
        !value
            .to_string()
            .to_ascii_lowercase()
            .contains("unsupported")
    );

    validate_decision(&value);
}

#[test]
fn targeted_decision_uses_the_bundled_release_catalog_not_cwd() {
    let cwd = TempDir::new().unwrap();
    std::fs::write(
        cwd.path().join("meshy-dcc-catalog.yml"),
        r#"entries:
  - name: private-blender
    description: Private Blender adapter
    dcc: [blender]
    tags: [adapter]
    install:
      type: pip
      pip_package: private-blender
      instructions_url: https://private.invalid/secret/install
"#,
    )
    .unwrap();
    let registry = TempDir::new().unwrap();
    let output = cli_command()
        .current_dir(cwd.path())
        .env("MESHY_DCC_REGISTRY_DIR", registry.path())
        .args(["--output", "json", "dcc-types", "--dcc-type", "blender"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(value["public_adapter"], "present");
    assert_eq!(value["released_catalog"], "present");
    // The cwd entry is installable; the bundled Meshy one is not published yet.
    assert_eq!(value["next_action"]["id"], "inspect_catalog");
    assert_eq!(value["next_action"]["instructions_url"], Value::Null);
    assert!(!value.to_string().contains("private.invalid"));
    validate_decision(&value);
}

#[test]
fn unfiltered_catalog_uses_the_bundled_release_catalog_not_cwd() {
    let cwd = TempDir::new().unwrap();
    std::fs::write(
        cwd.path().join("meshy-dcc-catalog.yml"),
        r#"entries:
  - name: private-only-adapter
    description: Private adapter
    dcc: [private-host]
    tags: [adapter]
    url: https://private.invalid/secret/repository
"#,
    )
    .unwrap();
    let output = cli_command()
        .current_dir(cwd.path())
        .args(["--output", "json", "dcc-types"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let serialized = value.to_string();

    for name in bundled_adapter_names() {
        assert!(serialized.contains(&name), "{name}");
    }
    assert!(!serialized.contains("private-only-adapter"));
    assert!(!serialized.contains("private.invalid"));
}

#[test]
fn plan_only_install_uses_the_bundled_release_catalog_not_cwd() {
    let cwd = TempDir::new().unwrap();
    std::fs::write(
        cwd.path().join("meshy-dcc-catalog.yml"),
        r#"entries:
  - name: private-blender
    description: Private Blender adapter
    dcc: [blender]
    tags: [adapter]
    url: https://private.invalid/secret/repository
    install:
      type: pip
      pip_package: private-blender
"#,
    )
    .unwrap();
    let output = cli_command()
        .current_dir(cwd.path())
        .args([
            "--output",
            "json",
            "--non-interactive",
            "install",
            "--dcc-type",
            "blender",
        ])
        .output()
        .unwrap();
    // The cwd entry would plan; the bundled Meshy Blender entry is not published yet.
    assert!(!output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(combined.contains("'meshy-dcc-blender' is not available for installation"));
    assert!(!combined.contains("private-blender"));
    assert!(!combined.contains("private.invalid"));
}

#[test]
fn dcc_types_does_not_present_a_custom_catalog_as_the_release_catalog() {
    let registry = TempDir::new().unwrap();
    let registry_s = registry.path().to_string_lossy().to_string();
    let catalog = NamedTempFile::new().unwrap();
    std::fs::write(
        catalog.path(),
        r#"entries:
  - name: studio-unreal
    description: Studio Unreal adapter
    dcc: [unreal]
    tags: [adapter]
    install:
      type: pip
      pip_package: studio-unreal
      instructions_url: https://internal.invalid/unreal/install
"#,
    )
    .unwrap();
    let catalog_s = catalog.path().to_string_lossy().to_string();
    let value = run_json_with_env(
        &[
            "--output",
            "json",
            "dcc-types",
            "--catalog",
            &catalog_s,
            "--dcc-type",
            "unreal",
        ],
        &[("MESHY_DCC_REGISTRY_DIR", registry_s.as_str())],
    );

    assert_eq!(value["public_adapter"], "unknown");
    assert_eq!(value["released_catalog"], "unknown");
    assert_eq!(value["next_action"]["id"], "inspect_catalog");
    assert_eq!(value["next_action"]["instructions_url"], Value::Null);
    assert!(!value["next_action"].to_string().contains(&catalog_s));
    assert!(!value["next_action"].to_string().contains("install"));
    assert!(!value.to_string().contains("internal.invalid"));
}

#[test]
fn dcc_types_keeps_live_registration_and_readiness_separate_from_catalog_support() {
    let registry_dir = TempDir::new().unwrap();
    let registry = FileRegistry::new(registry_dir.path()).unwrap();
    registry
        .register(ServiceEntry::new("godot", "127.0.0.1", 18080))
        .unwrap();
    let registry_s = registry_dir.path().to_string_lossy().to_string();
    let value = run_json_with_env(
        &["--output", "json", "dcc-types", "--dcc-type", "godot"],
        &[("MESHY_DCC_REGISTRY_DIR", registry_s.as_str())],
    );

    assert_eq!(value["public_adapter"], "present");
    assert_eq!(value["released_catalog"], "present");
    assert_eq!(value["live_instances"], 1);
    assert_eq!(value["registry_registration"], "present");
    assert_eq!(value["direct_readiness"], "ready");
    assert_eq!(value["gateway_capability_index"], "unknown");
    assert_eq!(value["search_hit"], "unknown");
    assert_eq!(value["exact_instance_call"], "not_run");
    assert_eq!(value["next_action"]["id"], "search_capabilities");
}

#[test]
fn dcc_types_waits_for_a_registered_but_unready_instance_before_searching() {
    let registry_dir = TempDir::new().unwrap();
    let registry = FileRegistry::new(registry_dir.path()).unwrap();
    let mut entry = ServiceEntry::new("unreal", "127.0.0.1", 18080);
    entry.status = ServiceStatus::Booting;
    registry.register(entry).unwrap();
    let registry_s = registry_dir.path().to_string_lossy().to_string();
    let value = run_json_with_env(
        &["--output", "json", "dcc-types", "--dcc-type", "unreal"],
        &[("MESHY_DCC_REGISTRY_DIR", registry_s.as_str())],
    );

    assert_eq!(value["registry_registration"], "present");
    assert_eq!(value["direct_readiness"], "not_ready");
    assert_eq!(value["failure_stage"], "direct_readiness");
    assert_eq!(value["failure_reason"], "INSTANCE_NOT_READY");
    assert_eq!(value["next_action"]["id"], "wait_ready");
    assert_eq!(
        value["next_action"]["command"],
        serde_json::json!([
            "meshy-dcc",
            "--output",
            "json",
            "wait-ready",
            "--dcc-type",
            "unreal"
        ])
    );
}

#[test]
fn invalid_dcc_identifiers_return_a_safe_schema_valid_failure() {
    let registry = TempDir::new().unwrap();
    let invalid_values = [
        "".to_string(),
        "x".repeat(65),
        "maya\r\nPRIVATE".to_string(),
        r"C:\private\adapter".to_string(),
        "/private/adapter".to_string(),
    ];

    for invalid in invalid_values {
        let output = cli_command()
            .env("MESHY_DCC_REGISTRY_DIR", registry.path())
            .args(["--output", "json", "dcc-types", "--dcc-type", &invalid])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        assert_eq!(value["dcc_type"], "unknown");
        assert_eq!(value["live_instances"], Value::Null);
        assert_eq!(value["registry_registration"], "unknown");
        assert_eq!(value["failure_stage"], "input_validation");
        assert_eq!(value["failure_reason"], "INVALID_DCC_TYPE");
        assert_eq!(value["next_action"]["id"], "inspect_catalog");
        if !invalid.is_empty() {
            assert!(!combined.contains(&invalid));
        }
        validate_decision(&value);
    }
}

#[test]
fn invalid_catalog_returns_a_safe_schema_valid_failure() {
    let registry_dir = TempDir::new().unwrap();
    let registry = FileRegistry::new(registry_dir.path()).unwrap();
    registry
        .register(ServiceEntry::new("unreal", "127.0.0.1", 18080))
        .unwrap();
    let catalog = NamedTempFile::new().unwrap();
    std::fs::write(catalog.path(), "entries: [private-secret").unwrap();
    let catalog_s = catalog.path().to_string_lossy().to_string();
    let registry_s = registry_dir.path().to_string_lossy().to_string();
    let output = cli_command()
        .env("MESHY_DCC_REGISTRY_DIR", &registry_s)
        .args([
            "--output",
            "json",
            "dcc-types",
            "--catalog",
            &catalog_s,
            "--dcc-type",
            "unreal",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert_eq!(value["failure_stage"], "catalog_load");
    assert_eq!(value["failure_reason"], "CATALOG_LOAD_FAILED");
    assert_eq!(value["live_instances"], 1);
    assert_eq!(value["registry_registration"], "present");
    assert_eq!(value["direct_readiness"], "ready");
    assert_eq!(value["next_action"]["id"], "inspect_catalog");
    assert!(!combined.contains(&catalog_s));
    validate_decision(&value);
}

#[test]
fn unreadable_registry_returns_a_safe_schema_valid_failure() {
    let registry_file = NamedTempFile::new().unwrap();
    let registry_s = registry_file.path().to_string_lossy().to_string();
    let output = cli_command()
        .env("MESHY_DCC_REGISTRY_DIR", &registry_s)
        .args(["--output", "json", "dcc-types", "--dcc-type", "unreal"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert_eq!(value["public_adapter"], "present");
    assert_eq!(value["released_catalog"], "present");
    assert_eq!(value["live_instances"], Value::Null);
    assert_eq!(value["registry_registration"], "unknown");
    assert_eq!(value["failure_stage"], "registry_read");
    assert_eq!(value["failure_reason"], "REGISTRY_READ_FAILED");
    assert_eq!(value["next_action"]["id"], "inspect_registry");
    assert!(!combined.contains(&registry_s));
    validate_decision(&value);
}
