//! Read-only ownership projection for the CLI's instance inventory.

use std::path::Path;

use meshy_dcc_transport::discovery::types::ServiceEntry;
use serde_json::{Value, json};

use super::{LifecycleStore, entry_metadata, entry_project};
use crate::application::local_instance;
use crate::domain::start_instance::{
    LIFECYCLE_OPERATION_SCHEMA_VERSION, OPERATION_METADATA_KEYS, PROJECT_METADATA_KEYS,
};

/// Expose only original local launches that still match a live instance.
/// Missing or unreadable evidence omits the projection, never fails inventory.
pub fn project_list_launch_ownership(registry_dir: Option<&Path>, inventory: &mut Value) {
    let Some(rows) = inventory.get_mut("instances").and_then(Value::as_array_mut) else {
        return;
    };
    // A remote payload cannot grant ownership of a process on this machine.
    for row in rows.iter_mut().filter_map(Value::as_object_mut) {
        row.remove("launch");
        row.remove("associations");
    }
    let Some(registry_dir) = registry_dir else {
        return;
    };
    let Ok((live, _)) = local_instance::snapshot_dcc_entries(registry_dir) else {
        return;
    };
    let store = LifecycleStore::new(registry_dir);
    for row in rows {
        let Some(entry) = live.iter().find(|entry| row_matches_entry(row, entry)) else {
            continue;
        };
        let associations =
            crate::application::lifecycle::instance_associations(registry_dir, entry);
        if !associations.is_empty() {
            row["associations"] = json!(associations);
        }
        let Some(operation_id) = entry_metadata(entry, OPERATION_METADATA_KEYS) else {
            continue;
        };
        if uuid::Uuid::parse_str(operation_id).is_err() {
            continue;
        }
        let Ok(Some(operation)) = store.load(operation_id) else {
            continue;
        };
        let Some(project) = entry_project(entry) else {
            continue;
        };
        if operation.schema_version == LIFECYCLE_OPERATION_SCHEMA_VERSION
            && operation.operation_id == operation_id
            && operation.launched
            && operation.owned
            && operation.instance_id.as_deref() == Some(entry.instance_id.to_string().as_str())
            && operation.pid.is_some()
            && operation.pid == entry.host_pid.or(entry.pid)
            && operation.dcc_type.eq_ignore_ascii_case(&entry.dcc_type)
            && same_project(&operation.project, &project)
        {
            row["launch"] = json!({
                "operation_id": operation.operation_id,
                "owned": true,
                "launched": true,
            });
            if let Some(managed) = operation.management {
                row["launch"]["agent_context"] = managed.agent_context;
            }
        }
    }
}

fn row_metadata<'a>(row: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| {
        row.get("metadata")?
            .get(*key)?
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    })
}

fn row_matches_entry(row: &Value, entry: &ServiceEntry) -> bool {
    let Some(project) = row_metadata(row, PROJECT_METADATA_KEYS) else {
        return false;
    };
    let Some(bound_project) = entry_project(entry) else {
        return false;
    };
    row.get("instance_id").and_then(Value::as_str) == Some(entry.instance_id.to_string().as_str())
        && row.get("dcc_type").and_then(Value::as_str) == Some(entry.dcc_type.as_str())
        && row.get("stale").and_then(Value::as_bool) != Some(true)
        && row
            .get("host_pid")
            .and_then(Value::as_u64)
            .or_else(|| row.get("pid").and_then(Value::as_u64))
            == entry.host_pid.or(entry.pid).map(u64::from)
        && row_metadata(row, OPERATION_METADATA_KEYS)
            == entry_metadata(entry, OPERATION_METADATA_KEYS)
        && same_project(Path::new(project), &bound_project)
}

fn same_project(left: &Path, right: &Path) -> bool {
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::start_instance::LifecycleOperation;
    use meshy_dcc_transport::discovery::file_registry::FileRegistry;

    struct Fixture {
        root: tempfile::TempDir,
        _registry: FileRegistry,
        entry: ServiceEntry,
        operation: LifecycleOperation,
    }

    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir().unwrap();
            let mut entry = ServiceEntry::new("godot", "127.0.0.1", 18080);
            entry.pid = Some(std::process::id());
            entry.host_pid = entry.pid;
            let operation_id = uuid::Uuid::new_v4().to_string();
            entry
                .metadata
                .insert("meshy_dcc_operation_id".into(), operation_id.clone());
            entry.metadata.insert(
                "meshy_dcc_project".into(),
                root.path().display().to_string(),
            );
            let registry = FileRegistry::new(root.path()).unwrap();
            registry.register(entry.clone()).unwrap();
            let mut operation = LifecycleOperation::new(
                operation_id,
                "godot",
                root.path(),
                Path::new("/fixture/Godot"),
                None,
            );
            operation.instance_id = Some(entry.instance_id.to_string());
            operation.pid = entry.host_pid;
            operation.launched = true;
            operation.owned = true;
            LifecycleStore::new(root.path()).save(&operation).unwrap();
            Self {
                root,
                _registry: registry,
                entry,
                operation,
            }
        }

        fn inventory(&self) -> Value {
            json!({"instances": [local_instance::instance_to_value(self.entry.clone()).unwrap()]})
        }

        fn receipt(&self) -> std::path::PathBuf {
            LifecycleStore::new(self.root.path())
                .dir()
                .join(format!("{}.json", self.operation.operation_id))
        }
    }

    #[test]
    fn inventory_recovers_the_original_launch_without_writing_receipts() {
        let fixture = Fixture::new();
        let original = std::fs::read(fixture.receipt()).unwrap();
        // Every projection starts from disk, including after a client restart.
        for _ in 0..2 {
            let mut inventory = fixture.inventory();
            project_list_launch_ownership(Some(fixture.root.path()), &mut inventory);
            assert_eq!(
                inventory["instances"][0]["launch"],
                json!({
                    "operation_id": fixture.operation.operation_id, "owned": true, "launched": true,
                })
            );
        }
        assert_eq!(std::fs::read(fixture.receipt()).unwrap(), original);
    }

    #[test]
    fn inventory_omits_launch_when_any_receipt_identity_is_not_proven() {
        let fixture = Fixture::new();
        for field in [
            "owned",
            "launched",
            "pid",
            "instance_id",
            "dcc_type",
            "project",
            "schema_version",
            "operation_id",
        ] {
            let mut receipt = serde_json::to_value(&fixture.operation).unwrap();
            receipt[field] = match field {
                "owned" | "launched" => json!(false),
                "pid" => json!(std::process::id() + 1),
                "instance_id" | "operation_id" => json!(uuid::Uuid::new_v4().to_string()),
                "dcc_type" => json!("blender"),
                "project" => json!(fixture.root.path().join("another-project")),
                "schema_version" => json!(LIFECYCLE_OPERATION_SCHEMA_VERSION + 1),
                _ => unreachable!(),
            };
            std::fs::write(fixture.receipt(), serde_json::to_vec(&receipt).unwrap()).unwrap();
            let mut inventory = fixture.inventory();
            project_list_launch_ownership(Some(fixture.root.path()), &mut inventory);
            assert!(inventory["instances"][0].get("launch").is_none(), "{field}");
        }
        std::fs::write(fixture.receipt(), b"invalid receipt").unwrap();
        let mut inventory = fixture.inventory();
        project_list_launch_ownership(Some(fixture.root.path()), &mut inventory);
        assert!(inventory["instances"][0].get("launch").is_none());
    }

    #[test]
    fn remote_or_stale_inventory_cannot_claim_a_local_launch() {
        let fixture = Fixture::new();
        let mut remote = fixture.inventory();
        remote["instances"][0]["launch"] = json!({"owned": true});
        project_list_launch_ownership(None, &mut remote);
        assert!(remote["instances"][0].get("launch").is_none());
        for field in ["instance_id", "host_pid", "dcc_type", "metadata", "stale"] {
            let mut inventory = fixture.inventory();
            inventory["instances"][0][field] = match field {
                "instance_id" => json!(uuid::Uuid::new_v4().to_string()),
                "host_pid" => json!(std::process::id() + 1),
                "dcc_type" => json!("blender"),
                "metadata" => json!({}),
                "stale" => json!(true),
                _ => unreachable!(),
            };
            project_list_launch_ownership(Some(fixture.root.path()), &mut inventory);
            assert!(inventory["instances"][0].get("launch").is_none(), "{field}");
        }
    }

    #[test]
    fn inventory_requires_an_existing_canonical_project() {
        let mut fixture = Fixture::new();
        let missing = fixture.root.path().join("removed-project");
        fixture
            .entry
            .metadata
            .insert("meshy_dcc_project".into(), missing.display().to_string());
        fixture._registry.register(fixture.entry.clone()).unwrap();
        fixture.operation.project = missing;
        LifecycleStore::new(fixture.root.path())
            .save(&fixture.operation)
            .unwrap();
        let mut inventory = fixture.inventory();
        project_list_launch_ownership(Some(fixture.root.path()), &mut inventory);
        assert!(inventory["instances"][0].get("launch").is_none());
    }
}
