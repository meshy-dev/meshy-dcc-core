//! Machine-wide package preparation is separate from project configuration.

use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use meshy_dcc_catalog::CatalogEntry;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{HostPlaceholders, HostTargets, InstallService};
use crate::domain::install::{
    InstallPlanner, InstallRequest, InstallStepAction, app_bundle_executable, find_dcc_path,
    meshy_dcc_data_dir, normalized_dcc_key,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PreparedAdapter {
    schema_version: u8,
    entry: CatalogEntry,
    dcc_path: Option<PathBuf>,
    package_dir: Option<PathBuf>,
    prepared_at_unix: u64,
}

fn receipt_path(dcc: &str) -> PathBuf {
    meshy_dcc_data_dir()
        .join("installs/prepared")
        .join(format!("{}.json", normalized_dcc_key(dcc)))
}

fn load_prepared(dcc: &str) -> anyhow::Result<PreparedAdapter> {
    let value: PreparedAdapter = serde_json::from_slice(
        &std::fs::read(receipt_path(dcc)).context("adapter has not been prepared")?,
    )?;
    anyhow::ensure!(value.schema_version == 1, "unsupported preparation receipt");
    if let Some(dir) = &value.package_dir {
        let checksum = std::fs::read_to_string(dir.join(".meshy-dcc-package.sha256"))?;
        let expected = value
            .entry
            .install
            .as_ref()
            .and_then(|i| i.sha256.as_deref());
        anyhow::ensure!(
            Some(checksum.trim()) == expected,
            "prepared package checksum changed"
        );
        let host = value.entry.install.as_ref().and_then(|i| i.host.as_ref());
        if let Some(program) = host
            .and_then(|h| h.project_steps.first())
            .and_then(|a| a.first())
            .filter(|program| program.contains("{package_dir}"))
        {
            let program = program.replace("{package_dir}", &dir.to_string_lossy());
            anyhow::ensure!(
                Path::new(&program).is_file(),
                "prepared installer is missing"
            );
        }
    }
    Ok(value)
}

/// Observe explicit metadata or the last authenticated catalog without refreshing it.
pub fn observe_software(catalog: Option<&Path>) -> anyhow::Result<Value> {
    let service = if catalog.is_some() {
        InstallService::bundled()
    } else {
        InstallService::cached()
    };
    service.observe_software(catalog)
}

/// Refresh the official catalog only for an authorized acquisition. Explicit
/// catalogs keep their existing local behavior; planning uses cached observations.
pub async fn prepare_adapter(
    dcc: &str,
    dcc_path: Option<PathBuf>,
    catalog: Option<&Path>,
    execute: bool,
) -> anyhow::Result<Value> {
    let service = if catalog.is_some() {
        InstallService::bundled()
    } else if execute {
        InstallService::refreshed(false).await
    } else {
        InstallService::cached()
    };
    if !execute {
        return service.prepare_adapter(dcc, dcc_path, catalog, false);
    }
    let dcc = dcc.to_owned();
    let catalog = catalog.map(Path::to_path_buf);
    // Acquisition uses blocking HTTP and host installer commands. Keep them off
    // the async caller's runtime thread, as the ordinary install command does.
    tokio::task::spawn_blocking(move || {
        service.prepare_adapter(&dcc, dcc_path, catalog.as_deref(), true)
    })
    .await?
}

impl InstallService {
    /// Catalog-backed observations. A missing path match is not proof of absence.
    /// No host process is started, and no project or instance state is returned.
    pub fn observe_software(&self, catalog: Option<&Path>) -> anyhow::Result<Value> {
        let entries = self.load_entries(catalog)?;
        let mut rows = Vec::new();
        for entry in entries
            .iter()
            .filter(|e| e.tags.iter().any(|t| t.eq_ignore_ascii_case("adapter")))
        {
            let host = entry.install.as_ref().and_then(|i| i.host.as_ref());
            let patterns = host.map(|h| h.dcc_paths.as_slice()).unwrap_or_default();
            let paths = match find_dcc_path(patterns) {
                Ok(path) => vec![path],
                Err(paths) => paths,
            };
            let detection = match paths.len() {
                0 if patterns.is_empty() => "unknown",
                0 => "not_observed",
                1 => "found",
                _ => "multiple",
            };
            for dcc in &entry.dcc {
                let prepared = load_prepared(dcc).ok().filter(|prepared| {
                    prepared.entry.version == entry.version
                        && prepared
                            .entry
                            .install
                            .as_ref()
                            .and_then(|i| i.sha256.as_ref())
                            == entry.install.as_ref().and_then(|i| i.sha256.as_ref())
                });
                let mut row = json!({
                    "dcc_type": dcc, "name": meshy_dcc_models::DccName::parse(dcc).display_name(),
                    "detection": detection,
                    "candidates": paths.iter().map(|p| json!({"path": p})).collect::<Vec<_>>(),
                    "adapter": {
                        "available": entry.install.as_ref().is_some_and(|i| i.install_type == "host-package") && crate::domain::install::policy::validate(entry).is_ok(),
                        "version": entry.version,
                        "prepared": prepared.is_some(),
                        "per_project": host.is_some_and(|h| !h.project_steps.is_empty()),
                    }
                });
                if let Some(homepage) = host.and_then(|h| h.software_homepage.as_ref()) {
                    row["homepage"] = json!(homepage);
                }
                if let Some(prepared) = prepared {
                    row["adapter"]["prepared_at_unix"] = json!(prepared.prepared_at_unix);
                }
                rows.push(row);
            }
        }
        rows.sort_by(|a, b| a["dcc_type"].as_str().cmp(&b["dcc_type"].as_str()));
        Ok(json!({"schema_version": 1, "software": rows}))
    }

    /// Acquire the pinned package and run only machine/user-level enablement.
    /// Project steps are retained in Core's existing installation state for later
    /// configuration; they do not depend on the download server remaining alive.
    pub fn prepare_adapter(
        &self,
        dcc: &str,
        dcc_path: Option<PathBuf>,
        catalog: Option<&Path>,
        execute: bool,
    ) -> anyhow::Result<Value> {
        let entries = self.load_entries(catalog)?;
        let key = normalized_dcc_key(dcc);
        anyhow::ensure!(!key.is_empty(), "dcc_type is required");
        let matches: Vec<_> = entries
            .into_iter()
            .filter(|e| {
                e.tags.iter().any(|t| t.eq_ignore_ascii_case("adapter"))
                    && e.dcc.iter().any(|d| normalized_dcc_key(d) == key)
            })
            .collect();
        anyhow::ensure!(
            matches.len() == 1,
            "expected exactly one catalog adapter for {dcc}"
        );
        let entry = matches.into_iter().next().unwrap();
        crate::domain::install::policy::validate(&entry)?;
        let install = entry
            .install
            .as_ref()
            .context("adapter has no installation contract")?;
        anyhow::ensure!(
            install.install_type == "host-package",
            "adapter preparation requires a host package"
        );
        let mut preparation = entry.clone();
        if let Some(host) = preparation.install.as_mut().and_then(|i| i.host.as_mut()) {
            host.project_steps.clear();
        }
        let request = InstallRequest {
            dcc_type: dcc.into(),
            version: entry.version.clone(),
            catalog_path: None,
            python: None,
            dcc_path: dcc_path.map(app_bundle_executable),
            plugin_source: None,
            adobe_debug_root: None,
            project: None,
        };
        let mut plan = InstallPlanner::plan(&[preparation], request)?;
        // Registration and live verification belong to a project session. Package
        // acquisition already verifies its checksum inside the host-package step.
        plan.steps.retain(|step| {
            step.action.is_none()
                || matches!(
                    step.action,
                    Some(
                        InstallStepAction::HostPackage { .. }
                            | InstallStepAction::WriteLaunchPlan { .. }
                    )
                )
        });
        plan.catalog = Some(self.catalog_provenance(catalog));
        plan = self.apply_auto_install_policy(plan);
        if !execute {
            return Ok(json!({"schema_version":1,"status":"planned","dcc_type":dcc,"plan":plan}));
        }
        let report = self.execute_plan(&plan, true);
        if report.status != "ok" || report.exit_code != 0 {
            return Ok(
                json!({"schema_version":1,"status":"failed","dcc_type":dcc,"report":report}),
            );
        }
        let package_dir = plan.steps.iter().find_map(|step| match &step.action {
            Some(InstallStepAction::HostPackage { extract_to, .. }) => extract_to.clone(),
            _ => None,
        });
        let prepared = PreparedAdapter {
            schema_version: 1,
            entry,
            dcc_path: plan.dcc_path.clone(),
            package_dir: package_dir.clone(),
            prepared_at_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
        };
        let path = receipt_path(dcc);
        std::fs::create_dir_all(path.parent().unwrap())?;
        let mut staging = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
        std::io::Write::write_all(&mut staging, &serde_json::to_vec_pretty(&prepared)?)?;
        staging.persist(&path)?;
        Ok(
            json!({"schema_version":1,"status":"prepared","dcc_type":dcc,
        "adapter_version":prepared.entry.version,"package_dir":package_dir}),
        )
    }
}

/// Configure an existing native project using only the adapter's recorded
/// installer. Native project creation remains ordinary agent file/code work.
pub fn configure_prepared_project(
    dcc: &str,
    project: &Path,
    dcc_path: Option<PathBuf>,
) -> anyhow::Result<Value> {
    configure_prepared_project_before(
        dcc,
        project,
        dcc_path,
        None,
        &super::super::gateway_ensure::default_registry_dir(),
    )
}

pub fn validate_prepared_project_target(
    dcc: &str,
    project: &Path,
    dcc_path: Option<&Path>,
    registry: &Path,
) -> anyhow::Result<()> {
    let prepared = load_prepared(dcc)?;
    configuration_executable(&prepared, dcc, project, dcc_path, registry)?;
    Ok(())
}

fn configuration_executable(
    prepared: &PreparedAdapter,
    dcc: &str,
    project: &Path,
    requested: Option<&Path>,
    registry: &Path,
) -> anyhow::Result<Option<PathBuf>> {
    let host = prepared
        .entry
        .install
        .as_ref()
        .and_then(|i| i.host.as_ref())
        .context("adapter has no host contract")?;
    let consumes_path = host
        .project_steps
        .iter()
        .flatten()
        .any(|arg| arg.contains("{dcc_path}"));
    let requested = requested.map(|p| app_bundle_executable(p.to_path_buf()));
    if let Some(selected) = &requested {
        anyhow::ensure!(
            selected.is_absolute() && selected.is_file(),
            "host executable is missing"
        );
        if !consumes_path {
            anyhow::ensure!(
                host.project_steps.is_empty(),
                "this adapter's project steps cannot select dcc_path; omit it and use the adapter's native project/launch configuration"
            );
            use crate::application::instance_launch::{PlanResolution, plan};
            use crate::domain::start_instance::{Invocation, StartInstanceRequest};
            let launch = StartInstanceRequest {
                dcc_type: dcc.into(),
                project: project.to_path_buf(),
                launch_plan: None,
                version: None,
                wait_ready: false,
                timeout: std::time::Duration::ZERO,
                interval: std::time::Duration::ZERO,
                required: vec![],
                authorized: false,
                dry_run: true,
                registry_dir: registry.to_path_buf(),
                instance_id: None,
                invocation: Invocation::default(),
            };
            let actual = match plan::resolve(&launch, project) {
                PlanResolution::Resolved(plan) => Some(plan.executable),
                PlanResolution::Missing => {
                    host.launch.as_ref().and_then(|_| prepared.dcc_path.clone())
                }
                PlanResolution::Invalid { .. } => bail!(
                    "published launch executable cannot be verified; repair the adapter configuration before selecting dcc_path"
                ),
            };
            anyhow::ensure!(
                actual.as_ref().and_then(|p| p.canonicalize().ok())
                    == Some(selected.canonicalize()?),
                "this adapter's project steps cannot select dcc_path; use the prepared/published executable or prepare the adapter for the chosen executable first"
            );
        }
    }
    let path = requested
        .or_else(|| prepared.dcc_path.clone())
        .or_else(|| find_dcc_path(&host.dcc_paths).ok());
    anyhow::ensure!(
        !consumes_path || path.is_some(),
        "project configuration requires one known host executable; supply dcc_path when discovery is absent or ambiguous"
    );
    if let Some(path) = &path {
        anyhow::ensure!(
            path.is_absolute() && path.is_file(),
            "host executable is missing"
        );
    }
    Ok(path)
}

pub fn configure_prepared_project_before(
    dcc: &str,
    project: &Path,
    dcc_path: Option<PathBuf>,
    deadline: Option<std::time::Instant>,
    registry: &Path,
) -> anyhow::Result<Value> {
    anyhow::ensure!(
        project.is_absolute() && project.is_dir(),
        "project must be an existing absolute directory"
    );
    let project = project.canonicalize()?;
    let prepared = load_prepared(dcc)?;
    let host = prepared
        .entry
        .install
        .as_ref()
        .and_then(|i| i.host.as_ref())
        .context("adapter has no host contract")?;
    let path = configuration_executable(&prepared, dcc, &project, dcc_path.as_deref(), registry)?;
    let values = HostPlaceholders {
        targets: HostTargets {
            dcc_path: path.as_deref(),
            extract_to: prepared.package_dir.as_deref(),
            project: Some(&project),
        },
        // Project steps must use the prepared package directory, never a
        // transient archive path left over from the installer.
        package: Path::new(""),
    };
    for argv in &host.project_steps {
        anyhow::ensure!(
            deadline.is_none_or(|d| std::time::Instant::now() < d),
            "project operation deadline expired before the next installer step"
        );
        anyhow::ensure!(
            !argv.iter().any(|a| a.contains("{package}")),
            "project installer depends on a transient archive"
        );
        let output = values
            .command(argv)
            .map_err(anyhow::Error::msg)?
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()?;
        if !output.status.success() {
            let detail = if output.stderr.is_empty() {
                &output.stdout
            } else {
                &output.stderr
            };
            let text = String::from_utf8_lossy(detail);
            let tail: String = text
                .chars()
                .rev()
                .take(2000)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            bail!("project installer failed: {}", tail.trim());
        }
    }
    Ok(json!({"schema_version":1,"status":"configured","dcc_type":dcc,"project":project}))
}

#[cfg(test)]
#[path = "preparation_tests.rs"]
mod tests;
