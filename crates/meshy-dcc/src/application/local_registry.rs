//! Local FileRegistry inventory for `meshy-dcc list`.
//!
//! This is the local-mode inventory path: it reads the same core default
//! registry directory used by sidecars and gateway runners, prunes dead rows
//! via `FileRegistry::read_alive`, and formats the result like the gateway
//! `/v1/instances` payload.

use std::path::{Path, PathBuf};

use meshy_dcc_transport::discovery::types::ServiceEntry;
use serde_json::{Value, json};

use crate::application::local_instance;

/// Inventory for `meshy-dcc list`: live rows, and it evicts dead ones.
pub fn list_local_instances(registry_dir: PathBuf) -> anyhow::Result<Value> {
    let (entries, evicted) = local_instance::live_dcc_entries(&registry_dir)?;
    let mut payload = inventory(entries, &registry_dir)?;
    payload["evicted"] = json!(evicted);
    Ok(payload)
}

/// Strictly read-only inventory for `meshy-dcc doctor`: the live rows in the
/// same shape as [`list_local_instances`], plus the dead rows under `dead`
/// (summaries). Nothing is evicted, created or rewritten.
pub fn inspect_local_instances(registry_dir: PathBuf) -> anyhow::Result<Value> {
    let (live, dead) = local_instance::snapshot_dcc_entries(&registry_dir)?;
    let mut payload = inventory(live, &registry_dir)?;
    payload["read_only"] = json!(true);
    payload["dead"] = Value::Array(dead.iter().map(local_instance::instance_summary).collect());
    Ok(payload)
}

fn inventory(entries: Vec<ServiceEntry>, registry_dir: &Path) -> anyhow::Result<Value> {
    let mut instances: Vec<_> = entries
        .into_iter()
        .map(local_instance::instance_to_value)
        .collect::<anyhow::Result<Vec<_>>>()?;
    instances.sort_by(|left, right| {
        let left_key = list_sort_key(left);
        let right_key = list_sort_key(right);
        left_key.cmp(&right_key)
    });

    Ok(json!({
        "total": instances.len(),
        "instances": instances,
        "source": "local_registry",
        "registry_dir": registry_dir,
        "gateway": {
            "current": {
                "name": "local",
                "role": "local",
                "registry_dir": registry_dir,
            },
            "candidates": [],
            "source": "local_registry"
        }
    }))
}

fn list_sort_key(value: &Value) -> (String, String) {
    let dcc = value
        .get("dcc_type")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let id = value
        .get("instance_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    (dcc, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use meshy_dcc_transport::discovery::file_registry::FileRegistry;
    use meshy_dcc_transport::discovery::types::{ServiceEntry, ServiceStatus};
    use std::time::{Duration, SystemTime};

    #[test]
    fn local_list_formats_registry_rows() {
        let dir = tempfile::tempdir().unwrap();
        let registry = FileRegistry::new(dir.path()).unwrap();
        let mut entry = ServiceEntry::new("maya", "127.0.0.1", 18080);
        entry.display_name = Some("Maya-Rig".to_string());
        entry.acquire_lease(
            "expired-workflow",
            Some("expired-job".to_string()),
            Some(SystemTime::now() - Duration::from_secs(1)),
        );
        registry.register(entry).unwrap();

        let payload = list_local_instances(dir.path().to_path_buf()).unwrap();

        assert_eq!(payload["source"], "local_registry");
        assert_eq!(payload["total"], 1);
        assert_eq!(payload["instances"][0]["dcc_type"], "maya");
        assert_eq!(
            payload["instances"][0]["mcp_url"],
            "http://127.0.0.1:18080/mcp"
        );
        assert_eq!(payload["instances"][0]["status"], "available");
        assert_eq!(payload["instances"][0]["lease_owner"], Value::Null);
        assert_eq!(payload["instances"][0]["current_job_id"], Value::Null);
        assert_eq!(payload["instances"][0]["lease_expires_at"], Value::Null);
        assert_eq!(
            payload["instances"][0]["direct_control"]["service_status"],
            ServiceStatus::Available.to_string()
        );
    }
}
