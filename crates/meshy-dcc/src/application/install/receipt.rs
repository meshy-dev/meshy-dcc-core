//! Install receipts: one small JSON file per successful `install --execute`,
//! so `dcc-types` (and the admin UI behind it) can say which connectors are
//! installed, at which version and in which project.
//!
//! Receipts live under the Meshy DCC data directory, not the registry
//! directory, because the registry sits in the temp directory and is cleared.
//! A receipt records what this CLI installed; it is not a probe of the host.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::InstallExecutionReport;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallReceipt {
    pub dcc_type: String,
    pub adapter_version: String,
    pub core_version: String,
    /// Seconds since the Unix epoch.
    pub installed_at: u64,
    /// The project the adapter went into, for per-project adapters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<PathBuf>,
    /// Where the connector's package was extracted. Usage telemetry sends a
    /// skill's name only when its folder is inside one of these. Absent for
    /// connectors the host installs itself (Blender's extension) and for
    /// receipts written before this field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_dir: Option<PathBuf>,
}

fn receipts_dir() -> PathBuf {
    crate::domain::install::meshy_dcc_data_dir().join("installs")
}

/// One file per DCC, or per DCC and project, so a reinstall replaces it.
fn receipt_path(dir: &Path, dcc_type: &str, project: Option<&Path>) -> PathBuf {
    let name = match project {
        None => format!("{dcc_type}.json"),
        Some(project) => {
            let hash = Sha256::digest(project.to_string_lossy().as_bytes());
            let short: String = hash[..6].iter().map(|b| format!("{b:02x}")).collect();
            format!("{dcc_type}-{short}.json")
        }
    };
    dir.join(name)
}

/// Write the receipt for a successful report; returns where it went.
pub fn record(report: &InstallExecutionReport, project: Option<&Path>) -> Option<PathBuf> {
    record_in(&receipts_dir(), report, project)
}

fn record_in(
    dir: &Path,
    report: &InstallExecutionReport,
    project: Option<&Path>,
) -> Option<PathBuf> {
    // `partial` is the normal outcome for host adapters: registering with the
    // gateway is deferred until the host starts.
    if !matches!(report.status.as_str(), "ok" | "partial") {
        return None;
    }
    let project = project.map(|p| fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf()));
    let receipt = InstallReceipt {
        dcc_type: report.dcc_type.clone(),
        adapter_version: report.adapter_version.clone(),
        core_version: report.core_version.clone(),
        installed_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
        project,
        package_dir: report
            .package_dir
            .as_ref()
            .map(|p| fs::canonicalize(p).unwrap_or_else(|_| p.clone())),
    };
    let path = receipt_path(dir, &receipt.dcc_type, receipt.project.as_deref());
    let write = || -> std::io::Result<()> {
        fs::create_dir_all(dir)?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(&receipt)?)?;
        fs::rename(&tmp, &path)
    };
    match write() {
        Ok(()) => Some(path),
        Err(error) => {
            // The install itself succeeded; a missing receipt only hides it from the UI.
            eprintln!(
                "warning: could not write install receipt {}: {error}",
                path.display()
            );
            None
        }
    }
}

/// Every receipt, oldest first. Unreadable files are skipped.
pub fn list() -> Vec<InstallReceipt> {
    list_in(&receipts_dir())
}

fn list_in(dir: &Path) -> Vec<InstallReceipt> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut receipts: Vec<InstallReceipt> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
        .filter_map(|entry| serde_json::from_slice(&fs::read(entry.path()).ok()?).ok())
        .collect();
    receipts.sort_by_key(|receipt| receipt.installed_at);
    receipts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(status: &str) -> InstallExecutionReport {
        let mut report: InstallExecutionReport = serde_json::from_value(serde_json::json!({
            "schema_version": 1, "status": status, "dcc_type": "unity",
            "adapter_version": "0.1.0", "core_version": "0.2.0", "stage": "complete",
            "exit_code": 0, "steps": [],
            "rollback": {"attempted": false, "status": "not_attempted", "failure_count": 0},
            "next_steps": [], "receipt_path": null,
            "verify": {"directly_usable": false, "failure_stage": null, "failure_reason": null}
        }))
        .unwrap();
        report.status = status.into();
        report
    }

    #[test]
    fn records_successful_installs_once_per_dcc_and_project() {
        let dir = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        assert!(record_in(dir.path(), &report("failed"), None).is_none());

        let first = record_in(dir.path(), &report("ok"), Some(project.path())).unwrap();
        let again = record_in(dir.path(), &report("ok"), Some(project.path())).unwrap();
        assert_eq!(first, again, "a reinstall replaces the receipt");
        record_in(dir.path(), &report("partial"), None).unwrap();

        let receipts = list_in(dir.path());
        assert_eq!(receipts.len(), 2);
        assert!(receipts.iter().any(|r| r.project.is_none()));
        assert!(receipts.iter().all(|r| r.adapter_version == "0.1.0"));
    }

    #[test]
    fn records_the_extracted_package_dir_when_there_is_one() {
        let dir = tempfile::tempdir().unwrap();
        let package = tempfile::tempdir().unwrap();
        let mut extracted = report("ok");
        extracted.package_dir = Some(package.path().to_path_buf());
        record_in(dir.path(), &extracted, None).unwrap();
        assert_eq!(
            list_in(dir.path())[0].package_dir.as_deref(),
            Some(fs::canonicalize(package.path()).unwrap().as_path())
        );

        // A host-installed connector has no package dir; neither has an old receipt.
        let other = tempfile::tempdir().unwrap();
        record_in(other.path(), &report("ok"), None).unwrap();
        assert_eq!(list_in(other.path())[0].package_dir, None);
        let old: InstallReceipt = serde_json::from_str(
            r#"{"dcc_type":"maya","adapter_version":"0.1.0","core_version":"0.2.2","installed_at":1}"#,
        )
        .unwrap();
        assert_eq!(old.package_dir, None);
    }
}
