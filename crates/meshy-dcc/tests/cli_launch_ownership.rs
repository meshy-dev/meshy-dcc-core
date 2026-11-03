//! CLI process restarts recover launch ownership from Core's authoritative store.

mod support;

use meshy_dcc::application::instance_launch::LifecycleStore;
use meshy_dcc::domain::start_instance::LifecycleOperation;
use meshy_dcc_transport::discovery::file_registry::FileRegistry;
use meshy_dcc_transport::discovery::types::ServiceEntry;
use serde_json::json;

#[test]
fn list_recovers_the_same_owned_launch_in_fresh_cli_processes() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let operation_id = uuid::Uuid::new_v4().to_string();
    let mut entry = ServiceEntry::new("godot", "127.0.0.1", 18080);
    entry.pid = Some(std::process::id());
    entry.host_pid = entry.pid;
    entry
        .metadata
        .insert("meshy_dcc_operation_id".into(), operation_id.clone());
    entry
        .metadata
        .insert("meshy_dcc_project".into(), project.display().to_string());
    let registry = FileRegistry::new(root.path()).unwrap();
    registry.register(entry.clone()).unwrap();
    let mut operation = LifecycleOperation::new(
        operation_id.clone(),
        "godot",
        &project,
        &project.join("Godot"),
        None,
    );
    operation.pid = entry.host_pid;
    operation.instance_id = Some(entry.instance_id.to_string());
    operation.owned = true;
    operation.launched = true;
    let store = LifecycleStore::new(root.path());
    store.save(&operation).unwrap();

    let registry = root.path().to_string_lossy();
    let args = ["--no-auto-gateway", "--gateway", "local", "list"];
    for _ in 0..2 {
        let inventory = support::run_json_with_env(&args, &[("MESHY_DCC_REGISTRY_DIR", &registry)]);
        assert_eq!(inventory["total"], 1);
        assert_eq!(
            inventory["instances"][0]["launch"],
            json!({
                "operation_id": operation_id, "owned": true, "launched": true,
            })
        );
    }

    operation.pid = Some(std::process::id() + 1);
    store.save(&operation).unwrap();
    let inventory = support::run_json_with_env(&args, &[("MESHY_DCC_REGISTRY_DIR", &registry)]);
    assert_eq!(inventory["total"], 1);
    assert!(inventory["instances"][0].get("launch").is_none());
}
