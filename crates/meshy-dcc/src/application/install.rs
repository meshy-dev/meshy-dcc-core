use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::domain::install_catalog::CatalogSource;
use meshy_dcc_catalog::CatalogEntry;
use thiserror::Error;

use crate::domain::install::{
    InstallPlan, InstallPlanError, InstallPlanner, InstallPolicy, InstallRequest, InstallStepAction,
};

const BUNDLED_CATALOG: &str = include_str!("../../../../meshy-dcc-catalog.yml");
/// Catalog of pip adapters that exercises the install mechanics in tests.
#[cfg(test)]
const FIXTURE_CATALOG: &str = include_str!("../../../../tests/fixtures/pip-adapter-catalog.yml");

mod adobe;
mod catalog;
#[cfg(test)]
mod catalog_contract_tests;
mod discovery;
mod pip;
mod policy;
pub mod receipt;
mod report;

pub use discovery::{DccAdapterSummary, DccTypeSummary, DccTypesCatalog};
use pip::{pip_install_args, pip_show_args, pip_uninstall_args};
use policy::{AutoInstallPolicy, ask_consent, render_install_policy_prompt};
pub use report::{
    InstallExecutionError, InstallExecutionReport, InstallReportNextStep, InstallRollbackReport,
    InstallStepReport, InstallStepRollbackReport, InstallVerifyReport,
};
use report::{
    action_failure, execution_report_for_plan, failed_report, safe_report_identifier,
    stable_step_id, success_next_steps,
};

#[derive(Debug, Error)]
pub enum InstallError {
    #[error("{0}")]
    CatalogRefresh(String),
    #[error(transparent)]
    Catalog(#[from] meshy_dcc_catalog::CatalogError),
    #[error(transparent)]
    Plan(#[from] InstallPlanError),
    #[error("consent denied by user")]
    ConsentDenied,
    #[error("step '{step}' failed: {message}")]
    StepFailed { step: String, message: String },
    #[error("rollback of step '{step}' failed: {message}")]
    RollbackFailed { step: String, message: String },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Describes how to undo a completed step.
#[derive(Debug)]
enum StepRollback {
    /// Remove a file or directory that was created.
    RemovePath(PathBuf),
    /// Run a shell command to revert.
    Command { program: String, args: Vec<String> },
}

#[derive(Debug)]
enum StepExecution {
    Completed(Option<StepRollback>),
    Deferred,
}

pub struct InstallService {
    default_catalog_path: Option<PathBuf>,
    auto_install_policy: AutoInstallPolicy,
    catalog_snapshot: Option<catalog::ResolvedCatalog>,
    catalog_error: Option<String>,
    require_fresh_catalog: bool,
}

impl InstallService {
    /// Adapter catalog entries from the bundled or explicitly supplied catalog.
    ///
    /// Read-only view for sibling application modules (e.g. the `doctor`
    /// adapter-import probe) that need adapter package/interpreter metadata.
    pub(crate) fn catalog_entries(
        &self,
        requested_path: Option<&Path>,
    ) -> Result<Vec<CatalogEntry>, InstallError> {
        self.load_entries(requested_path)
    }

    #[must_use]
    pub fn new(default_catalog_path: PathBuf) -> Self {
        Self {
            default_catalog_path: Some(default_catalog_path),
            auto_install_policy: AutoInstallPolicy::from_env(),
            catalog_snapshot: None,
            catalog_error: None,
            require_fresh_catalog: false,
        }
    }

    /// Build a service whose implicit catalog is the release catalog embedded
    /// in this binary. Custom catalogs must be passed explicitly in a request.
    #[must_use]
    pub fn bundled() -> Self {
        Self {
            default_catalog_path: None,
            auto_install_policy: AutoInstallPolicy::from_env(),
            catalog_snapshot: None,
            catalog_error: None,
            require_fresh_catalog: false,
        }
    }

    #[cfg(test)]
    fn with_auto_install_policy(
        default_catalog_path: PathBuf,
        auto_install_policy: AutoInstallPolicy,
    ) -> Self {
        Self {
            default_catalog_path: Some(default_catalog_path),
            auto_install_policy,
            catalog_snapshot: None,
            catalog_error: None,
            require_fresh_catalog: false,
        }
    }

    /// Generate an install plan (display-only, no execution).
    pub fn plan(&self, request: InstallRequest) -> Result<InstallPlan, InstallError> {
        let catalog = self.load_catalog(request.catalog_path.as_deref())?;
        let mut plan = InstallPlanner::plan(&catalog.entries, request)?;
        plan.catalog = Some(catalog.provenance);
        Ok(self.apply_auto_install_policy(plan))
    }

    /// Generate and execute an install plan with user consent.
    ///
    /// Execution always returns one machine-readable Install SOP v1 report.
    /// Internal errors are reduced to stable public codes before serialization.
    pub fn execute(
        &self,
        request: InstallRequest,
        skip_confirmation: bool,
    ) -> InstallExecutionReport {
        let requested_dcc = safe_report_identifier(&request.dcc_type, "unknown");
        match self.plan(request) {
            Ok(plan) => self.execute_plan(&plan, skip_confirmation),
            Err(error) => report::plan_failure_report(requested_dcc, &error),
        }
    }

    fn apply_auto_install_policy(&self, mut plan: InstallPlan) -> InstallPlan {
        if self.auto_install_policy.enabled {
            plan.install_policy = InstallPolicy::enabled();
            return plan;
        }

        let prompt = render_install_policy_prompt(&self.auto_install_policy.prompt_template, &plan);
        plan.install_policy = InstallPolicy::disabled(prompt);
        plan
    }

    fn execute_plan(&self, plan: &InstallPlan, skip_confirmation: bool) -> InstallExecutionReport {
        self.execute_plan_with(
            plan,
            skip_confirmation,
            |action, plan| match action {
                InstallStepAction::Verify => execute_verify(plan).map(StepExecution::Completed),
                _ => execute_action(action),
            },
            execute_rollback,
        )
    }

    fn execute_plan_with<E, R>(
        &self,
        plan: &InstallPlan,
        skip_confirmation: bool,
        mut execute_step: E,
        mut rollback_step: R,
    ) -> InstallExecutionReport
    where
        E: FnMut(&InstallStepAction, &InstallPlan) -> Result<StepExecution, InstallError>,
        R: FnMut(&StepRollback) -> Result<(), InstallError>,
    {
        let mut report = execution_report_for_plan(plan);

        if self.require_fresh_catalog
            && plan.catalog.as_ref().is_some_and(|catalog| {
                !catalog.latest_checked && catalog.source != CatalogSource::Explicit
            })
        {
            eprintln!(
                "Latest installation catalog could not be checked. Reconnect or explicitly use --offline."
            );
            return failed_report(report, "preflight", 10, "INSTALL_CATALOG_UNAVAILABLE", None);
        }

        if !plan.install_policy.auto_install_enabled {
            eprintln!("Automatic installation is disabled by policy.");
            return failed_report(report, "preflight", 10, "AUTO_INSTALL_DISABLED", None);
        }

        let executable_steps = plan
            .steps
            .iter()
            .enumerate()
            .filter_map(|(index, step)| step.action.as_ref().map(|action| (index, action)))
            .collect::<Vec<_>>();

        if executable_steps.is_empty() {
            eprintln!("No executable steps in the install plan.");
            return failed_report(report, "preflight", 10, "NO_EXECUTABLE_STEPS", None);
        }

        eprintln!(
            "Install execution requires {} step(s).",
            executable_steps.len()
        );
        if !skip_confirmation {
            match ask_consent("Proceed with installation? [Y/n]") {
                Ok(true) => {}
                Ok(false) => {
                    return failed_report(report, "preflight", 10, "CONSENT_DENIED", None);
                }
                Err(_) => {
                    return failed_report(report, "preflight", 10, "CONSENT_INPUT_FAILED", None);
                }
            }
        }

        if plan
            .catalog
            .as_ref()
            .and_then(|catalog| catalog.expires_at)
            .is_some_and(|expires| catalog::now_seconds().map_or(true, |now| expires <= now))
        {
            return failed_report(report, "preflight", 10, "INSTALL_CATALOG_EXPIRED", None);
        }

        let mut completed: Vec<(usize, Option<StepRollback>)> = Vec::new();
        let mut has_deferred_steps = false;

        for (ordinal, (step_index, action)) in executable_steps.iter().enumerate() {
            let step_id = stable_step_id(action);
            eprint!(
                "  [{}/{}] {step_id} ... ",
                ordinal + 1,
                executable_steps.len()
            );
            match execute_step(action, plan) {
                Ok(StepExecution::Completed(rollback)) => {
                    eprintln!("OK");
                    report.steps[*step_index].status = "ok".into();
                    report.steps[*step_index].rollback.status = if rollback.is_some() {
                        "available".into()
                    } else {
                        "not_available".into()
                    };
                    completed.push((*step_index, rollback));
                }
                Ok(StepExecution::Deferred) => {
                    eprintln!("DEFERRED");
                    report.steps[*step_index].status = "deferred".into();
                    report.steps[*step_index].rollback.status = "not_available".into();
                    has_deferred_steps = true;
                }
                Err(error) => {
                    // Other steps' errors can carry credentials (pip index URLs, tokens),
                    // so only the Meshy adapter installer's own output is echoed.
                    // ponytail: first 2000 chars; the host's own log holds the rest.
                    if matches!(action, InstallStepAction::HostPackage { .. }) {
                        let message: String = error.to_string().chars().take(2000).collect();
                        eprintln!("FAILED: {message}");
                        report.steps[*step_index].message = Some(message);
                    } else {
                        eprintln!("FAILED");
                    }
                    report.steps[*step_index].status = "failed".into();
                    report.steps[*step_index].rollback.status = "not_available".into();
                    let (failure_stage, failure_exit, failure_code) = action_failure(action);

                    let mut rollback_attempted = false;
                    let mut rollback_failures = 0;
                    for (completed_index, rollback) in completed.iter().rev() {
                        let Some(rollback) = rollback else {
                            continue;
                        };
                        rollback_attempted = true;
                        let rollback_report = &mut report.steps[*completed_index].rollback;
                        rollback_report.attempted = true;
                        if rollback_step(rollback).is_ok() {
                            rollback_report.status = "ok".into();
                        } else {
                            rollback_report.status = "failed".into();
                            rollback_failures += 1;
                        }
                    }
                    report.rollback = InstallRollbackReport {
                        attempted: rollback_attempted,
                        status: if rollback_failures > 0 {
                            "failed".into()
                        } else if rollback_attempted {
                            "ok".into()
                        } else {
                            "not_attempted".into()
                        },
                        failure_count: rollback_failures,
                    };

                    if rollback_failures > 0 {
                        return failed_report(
                            report,
                            "rollback",
                            30,
                            "ROLLBACK_FAILED",
                            Some(failure_code),
                        );
                    }
                    return failed_report(report, failure_stage, failure_exit, failure_code, None);
                }
            }
        }

        if has_deferred_steps {
            eprintln!("Installation execution completed with deferred manual steps.");
            report.status = "partial".into();
        } else {
            eprintln!("Installation execution completed.");
            report.status = "ok".into();
        }
        report.stage = "complete".into();
        report.exit_code = 0;
        report.next_steps = success_next_steps(&report.dcc_type);
        report.verify.failure_stage = Some("host-readiness".into());
        report.verify.failure_reason = Some("LIVE_DCC_VERIFICATION_REQUIRED".into());
        report
    }
}

// ── step executors ───────────────────────────────────────────────────────────────

/// Execute a single install action, distinguishing completed work from manual deferral.
fn execute_action(action: &InstallStepAction) -> Result<StepExecution, InstallError> {
    match action {
        InstallStepAction::PipInstall {
            package,
            version,
            extras,
            python,
            artifact_url,
            sha256,
        } => execute_pip_install(
            package,
            version.as_deref(),
            extras.as_deref(),
            python.as_deref(),
            artifact_url.as_deref(),
            sha256.as_deref(),
        )
        .map(StepExecution::Completed),
        InstallStepAction::GitClone { url, ref_, dest } => {
            execute_git_clone(url, ref_.as_deref(), dest).map(StepExecution::Completed)
        }
        InstallStepAction::ZipExtract { url, sha256, dest } => {
            execute_zip_extract(url, sha256.as_deref(), dest).map(StepExecution::Completed)
        }
        InstallStepAction::PathCopy { source, dest } => {
            execute_path_copy(source, dest).map(StepExecution::Completed)
        }
        InstallStepAction::AdobePluginLink {
            source,
            dest,
            product,
            extension_type,
            ..
        } => adobe::execute_plugin_link(source, dest, product, extension_type)
            .map(StepExecution::Completed),
        InstallStepAction::HostPackage {
            url,
            mirrors,
            sha256,
            dcc_path,
            extract_to,
            uninstall,
            steps,
            project,
            project_steps,
        } => execute_host_package(
            url,
            mirrors,
            sha256,
            HostTargets {
                dcc_path: dcc_path.as_deref(),
                extract_to: extract_to.as_deref(),
                project: project.as_deref(),
            },
            uninstall,
            steps,
            project_steps,
        )
        .map(StepExecution::Completed),
        InstallStepAction::WriteLaunchPlan {
            dcc_type,
            executable,
            argv,
            project_markers,
        } => execute_write_launch_plan(dcc_type, executable, argv, project_markers)
            .map(StepExecution::Completed),
        InstallStepAction::RegisterDcc {
            dcc_type,
            entry_point,
            dcc_path,
        } => Ok(execute_register_dcc(
            dcc_type,
            entry_point.as_deref(),
            dcc_path.as_deref(),
        )),
        InstallStepAction::Verify => Err(InstallError::StepFailed {
            step: "verify".into(),
            message: "verify requires the full install plan".into(),
        }),
    }
}

fn execute_pip_install(
    package: &str,
    version: Option<&str>,
    extras: Option<&[String]>,
    python: Option<&str>,
    artifact_url: Option<&str>,
    sha256: Option<&str>,
) -> Result<Option<StepRollback>, InstallError> {
    let python_cmd = python.unwrap_or("python");
    let package_spec = verified_pip_artifact_spec(package, version, extras, artifact_url, sha256)?;
    let mut cmd = Command::new(python_cmd);
    cmd.args(pip_install_args(&package_spec))
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let status = cmd.status().map_err(|e| InstallError::StepFailed {
        step: format!("pip-install-{package}"),
        message: format!("failed to launch {python_cmd}: {e}"),
    })?;

    if !status.success() {
        return Err(InstallError::StepFailed {
            step: format!("pip-install-{package}"),
            message: format!("{python_cmd} exited with {status}"),
        });
    }

    // Rollback: pip uninstall
    Ok(Some(StepRollback::Command {
        program: python_cmd.to_string(),
        args: pip_uninstall_args(package),
    }))
}

fn verified_pip_artifact_spec(
    package: &str,
    version: Option<&str>,
    extras: Option<&[String]>,
    artifact_url: Option<&str>,
    sha256: Option<&str>,
) -> Result<String, InstallError> {
    let version = version.unwrap_or_default().trim();
    let artifact_url = artifact_url.unwrap_or_default().trim();
    let value = sha256.unwrap_or_default().trim();
    let checksum = value.strip_prefix("sha256:").unwrap_or(value);
    let valid_package = !package.is_empty()
        && package
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && package
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
        && package
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    let valid_extras = extras.is_none_or(|values| {
        values.iter().all(|value| {
            !value.is_empty()
                && value
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_alphanumeric)
                && value
                    .as_bytes()
                    .last()
                    .is_some_and(u8::is_ascii_alphanumeric)
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        })
    });
    let normalized_package = package
        .chars()
        .map(|character| match character {
            '-' | '.' => '_',
            other => other.to_ascii_lowercase(),
        })
        .collect::<String>();
    let filename = artifact_url.rsplit('/').next().unwrap_or_default();
    let expected_prefix = format!("{normalized_package}-{version}-");
    let valid_artifact = artifact_url.starts_with("https://")
        && !artifact_url.contains('#')
        && !artifact_url.contains('?')
        && filename
            .to_ascii_lowercase()
            .starts_with(&expected_prefix.to_ascii_lowercase())
        && filename.ends_with("-py3-none-any.whl");
    if !valid_package
        || !valid_extras
        || version.is_empty()
        || !valid_artifact
        || checksum.len() != 64
        || !checksum.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(InstallError::StepFailed {
            step: "pip-integrity".into(),
            message: "catalog-pinned wheel URL, version, and SHA-256 are required".into(),
        });
    }

    let package_with_extras = if extras.is_some_and(|values| !values.is_empty()) {
        format!("{}[{}]", package, extras.unwrap().join(","))
    } else {
        package.to_string()
    };
    Ok(format!(
        "{package_with_extras} @ {artifact_url}#sha256={}",
        checksum.to_ascii_lowercase()
    ))
}

fn execute_git_clone(
    url: &str,
    ref_: Option<&str>,
    dest: &Path,
) -> Result<Option<StepRollback>, InstallError> {
    let commit = required_git_commit(ref_)?;
    if dest.exists() {
        return Err(InstallError::StepFailed {
            step: "git-clone".into(),
            message: format!("destination already exists: {}", dest.display()),
        });
    }

    // Ensure parent directory exists
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    run_git(
        dest,
        "init",
        &["init", "--quiet", dest.to_string_lossy().as_ref()],
    )?;
    run_git(dest, "remote-add", &["remote", "add", "origin", url])?;
    run_git(dest, "fetch", &["fetch", "--depth", "1", "origin", &commit])?;
    run_git(
        dest,
        "checkout",
        &["checkout", "--detach", "--quiet", "FETCH_HEAD"],
    )?;

    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dest)
        .output()
        .map_err(|error| InstallError::StepFailed {
            step: "git-verify".into(),
            message: format!("failed to launch git: {error}"),
        })?;
    if !output.status.success() {
        return Err(InstallError::StepFailed {
            step: "git-verify".into(),
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    let actual = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_ascii_lowercase();
    if actual != commit {
        return Err(InstallError::StepFailed {
            step: "git-verify".into(),
            message: format!("commit mismatch: expected {commit}, got {actual}"),
        });
    }

    Ok(Some(StepRollback::RemovePath(dest.to_path_buf())))
}

fn required_git_commit(ref_: Option<&str>) -> Result<String, InstallError> {
    let reference = ref_.unwrap_or_default().trim();
    if reference.len() != 40 || !reference.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(InstallError::StepFailed {
            step: "git-integrity".into(),
            message: "a full 40-character commit object ID is required".into(),
        });
    }
    Ok(reference.to_ascii_lowercase())
}

fn run_git(dest: &Path, step: &str, args: &[&str]) -> Result<(), InstallError> {
    let mut command = Command::new("git");
    command.args(args);
    if step != "init" {
        command.current_dir(dest);
    }
    let output = command.output().map_err(|error| InstallError::StepFailed {
        step: format!("git-{step}"),
        message: format!("failed to launch git: {error}"),
    })?;
    if !output.status.success() {
        return Err(InstallError::StepFailed {
            step: format!("git-{step}"),
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(())
}

fn execute_zip_extract(
    url: &str,
    sha256: Option<&str>,
    dest: &Path,
) -> Result<Option<StepRollback>, InstallError> {
    let expected = required_archive_sha256(sha256)?;
    if dest.exists() {
        return Err(InstallError::StepFailed {
            step: "zip-extract".into(),
            message: format!("destination already exists: {}", dest.display()),
        });
    }

    let bytes = download_verified(url, &expected)?;

    // Ensure parent directory exists
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Extract the archive
    let reader = std::io::Cursor::new(&bytes);
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| InstallError::StepFailed {
        step: "zip-extract".into(),
        message: format!("failed to open zip archive: {e}"),
    })?;

    archive
        .extract(dest)
        .map_err(|e| InstallError::StepFailed {
            step: "zip-extract".into(),
            message: format!("failed to extract to {}: {e}", dest.display()),
        })?;

    Ok(Some(StepRollback::RemovePath(dest.to_path_buf())))
}

/// Download `url` and return its bytes only if they match `expected` (lowercase hex).
/// Download from `url`, or from each mirror in turn when a download fails.
/// Bytes that fail the SHA-256 check stop the install: another source is not
/// a way around a failed check.
fn download_first_verified(
    url: &str,
    mirrors: &[String],
    expected: &str,
) -> Result<Vec<u8>, InstallError> {
    let mut failures = Vec::new();
    for source in std::iter::once(url).chain(mirrors.iter().map(String::as_str)) {
        match download_verified(source, expected) {
            Err(InstallError::StepFailed { step, message }) if step == "download" => {
                failures.push(message);
            }
            other => return other,
        }
    }
    Err(InstallError::StepFailed {
        step: "download".into(),
        message: failures.join("; "),
    })
}

fn download_verified(url: &str, expected: &str) -> Result<Vec<u8>, InstallError> {
    let bytes = reqwest::blocking::get(url)
        .and_then(reqwest::blocking::Response::error_for_status)
        .and_then(|response| response.bytes())
        .map_err(|e| InstallError::StepFailed {
            step: "download".into(),
            message: format!("failed to download {url}: {e}"),
        })?;
    use sha2::Digest;
    let actual_hex = sha2::Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if !actual_hex.eq_ignore_ascii_case(expected) {
        return Err(InstallError::StepFailed {
            step: "checksum".into(),
            message: format!("SHA-256 mismatch: expected {expected}, got {actual_hex}"),
        });
    }
    Ok(bytes.to_vec())
}

/// Operator-supplied paths a host-package install may use.
#[derive(Clone, Copy)]
struct HostTargets<'a> {
    dcc_path: Option<&'a Path>,
    extract_to: Option<&'a Path>,
    project: Option<&'a Path>,
}

/// Values for the `{dcc_path}`, `{package}`, `{package_dir}` and `{project}`
/// placeholders of host-package commands.
struct HostPlaceholders<'a> {
    targets: HostTargets<'a>,
    package: &'a Path,
}

impl HostPlaceholders<'_> {
    /// Expand every placeholder in `token`; a placeholder without a value is an error.
    fn expand(&self, token: &str) -> Result<String, String> {
        let values = [
            ("{dcc_path}", self.targets.dcc_path),
            ("{package_dir}", self.targets.extract_to),
            ("{package}", Some(self.package)),
            ("{project}", self.targets.project),
        ];
        let mut expanded = token.to_string();
        for (placeholder, value) in values {
            if expanded.contains(placeholder) {
                let value = value.ok_or_else(|| format!("{placeholder} has no value"))?;
                expanded = expanded.replace(placeholder, &value.to_string_lossy());
            }
        }
        Ok(expanded)
    }

    /// Build argv as a process without a shell; tokens are never re-split.
    fn command(&self, argv: &[String]) -> Result<Command, String> {
        let (program, args) = argv.split_first().ok_or("empty host command")?;
        let mut command = Command::new(self.expand(program)?);
        for token in args {
            command.arg(self.expand(token)?);
        }
        command.stdin(Stdio::null()).stdout(Stdio::null());
        Ok(command)
    }
}

/// Extract a verified zip into `dest` through a sibling staging directory.
/// A matching checksum marker means the same package is already there.
/// Returns whether `dest` was (re)created.
fn extract_host_package(bytes: &[u8], sha256: &str, dest: &Path) -> Result<bool, String> {
    const MARKER: &str = ".meshy-dcc-package.sha256";
    let marker = dest.join(MARKER);
    if fs::read_to_string(&marker).is_ok_and(|value| value.trim().eq_ignore_ascii_case(sha256)) {
        return Ok(false);
    }
    let parent = dest
        .parent()
        .ok_or_else(|| format!("no parent for {}", dest.display()))?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| format!("failed to open zip archive: {e}"))?;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|e| e.to_string())?;
        if entry.is_symlink() {
            return Err(format!("package contains a symlink: {}", entry.name()));
        }
    }
    let staging = tempfile::Builder::new()
        .prefix(".extract-")
        .tempdir_in(parent)
        .map_err(|e| e.to_string())?;
    archive
        .extract(staging.path())
        .map_err(|e| format!("failed to extract package: {e}"))?;
    fs::write(staging.path().join(MARKER), sha256).map_err(|e| e.to_string())?;
    if dest.exists() {
        fs::remove_dir_all(dest).map_err(|e| e.to_string())?;
    }
    fs::rename(staging.keep(), dest).map_err(|e| e.to_string())?;
    Ok(true)
}

fn execute_host_package(
    url: &str,
    mirrors: &[String],
    sha256: &str,
    targets: HostTargets<'_>,
    uninstall: &[Vec<String>],
    steps: &[Vec<String>],
    project_steps: &[Vec<String>],
) -> Result<Option<StepRollback>, InstallError> {
    let fail = |message: String| InstallError::StepFailed {
        step: "install-host-package".into(),
        message,
    };
    if let Some(dcc_path) = targets.dcc_path
        && (!dcc_path.is_absolute() || !dcc_path.is_file())
    {
        return Err(fail(format!(
            "--dcc-path must be an absolute path to the host executable: {}",
            dcc_path.display()
        )));
    }
    if let Some(project) = targets.project
        && (!project.is_absolute() || !project.is_dir())
    {
        return Err(fail(format!(
            "--project must be an absolute path to an existing directory: {}",
            project.display()
        )));
    }
    let name = crate::domain::install::host_package_file_name(url)
        .ok_or_else(|| fail(format!("refusing package URL {url}")))?;
    let expected = required_archive_sha256(Some(sha256))?;
    for mirror in mirrors {
        crate::domain::install::host_package_file_name(mirror)
            .ok_or_else(|| fail(format!("refusing package mirror {mirror}")))?;
    }
    let bytes = download_first_verified(url, mirrors, &expected)?;
    let download_dir = tempfile::tempdir()?;
    let package = download_dir.path().join(name);
    fs::write(&package, &bytes)?;
    let extracted = match targets.extract_to {
        Some(dest) => extract_host_package(&bytes, &expected, dest).map_err(fail)?,
        None => false,
    };
    let values = HostPlaceholders {
        targets,
        package: &package,
    };

    // A failed uninstall only means nothing was installed yet.
    for argv in uninstall {
        if let Ok(mut command) = values.command(argv) {
            let _ = command.stderr(Stdio::null()).status();
        }
    }
    for argv in steps.iter().chain(project_steps) {
        let output = values
            .command(argv)
            .map_err(&fail)?
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| fail(format!("failed to launch {:?}: {e}", argv[0])))?;
        if !output.status.success() {
            // Installers explain a failure on stderr, or in their JSON report
            // on stdout when run with `--json`.
            let detail = if output.stderr.iter().all(u8::is_ascii_whitespace) {
                &output.stdout
            } else {
                &output.stderr
            };
            let detail = String::from_utf8_lossy(detail);
            let tail: String = detail
                .chars()
                .rev()
                .take(2000)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            return Err(fail(format!(
                "host command {argv:?} exited with {}: {}",
                output.status,
                tail.trim()
            )));
        }
    }
    if let Some(argv) = uninstall.first()
        && let Ok(program) = values.expand(&argv[0])
    {
        return Ok(Some(StepRollback::Command {
            program,
            args: argv[1..]
                .iter()
                .filter_map(|token| values.expand(token).ok())
                .collect(),
        }));
    }
    Ok(targets
        .extract_to
        .filter(|_| extracted)
        .map(|dest| StepRollback::RemovePath(dest.to_path_buf())))
}

fn launch_plan_path(dcc_type: &str) -> PathBuf {
    crate::application::gateway_ensure::default_registry_dir()
        .join(crate::domain::start_instance::LAUNCH_PLAN_DIR_NAME)
        .join(format!(
            "{}.json",
            crate::application::instance_launch::plan::normalized_dcc_key(dcc_type)
        ))
}

fn execute_write_launch_plan(
    dcc_type: &str,
    executable: &Path,
    argv: &[String],
    project_markers: &[String],
) -> Result<Option<StepRollback>, InstallError> {
    let path = launch_plan_path(dcc_type);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let document = serde_json::json!({
        "schema_version": crate::domain::start_instance::LAUNCH_PLAN_SCHEMA_VERSION,
        "dcc_type": dcc_type,
        "executable": executable,
        "argv": argv,
        "project_markers": project_markers,
    });
    let text = serde_json::to_string_pretty(&document).map_err(|e| InstallError::StepFailed {
        step: "write-launch-plan".into(),
        message: e.to_string(),
    })?;
    fs::write(&path, text)?;
    Ok(Some(StepRollback::RemovePath(path)))
}

fn required_archive_sha256(sha256: Option<&str>) -> Result<String, InstallError> {
    let value = sha256.unwrap_or_default().trim();
    let checksum = value.strip_prefix("sha256:").unwrap_or(value);
    if checksum.len() != 64 || !checksum.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(InstallError::StepFailed {
            step: "zip-integrity".into(),
            message: "exactly 64 hexadecimal SHA-256 digits are required".into(),
        });
    }
    Ok(checksum.to_ascii_lowercase())
}

fn execute_path_copy(source: &Path, dest: &Path) -> Result<Option<StepRollback>, InstallError> {
    if dest.exists() {
        return Err(InstallError::StepFailed {
            step: "path-copy".into(),
            message: format!("destination already exists: {}", dest.display()),
        });
    }

    if !source.exists() {
        return Err(InstallError::StepFailed {
            step: "path-copy".into(),
            message: format!("source does not exist: {}", source.display()),
        });
    }

    // Ensure parent directory exists
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if source.is_dir() {
        copy_dir_recursive(source, dest)?;
    } else {
        std::fs::copy(source, dest)?;
    }

    Ok(Some(StepRollback::RemovePath(dest.to_path_buf())))
}

fn execute_register_dcc(
    _dcc_type: &str,
    _entry_point: Option<&str>,
    _dcc_path: Option<&Path>,
) -> StepExecution {
    // Registration is owned by the DCC plugin's sidecar. The CLI can install
    // packages, but it must not pretend a host process is registered until the
    // plugin starts, stays alive, and advertises itself in the registry.
    StepExecution::Deferred
}

fn execute_verify(plan: &InstallPlan) -> Result<Option<StepRollback>, InstallError> {
    if let Some(path) = &plan.dcc_path
        && !path.exists()
    {
        return Err(InstallError::StepFailed {
            step: "verify".into(),
            message: format!("DCC path does not exist: {}", path.display()),
        });
    }
    for step in &plan.steps {
        let Some(action) = &step.action else {
            continue;
        };
        match action {
            InstallStepAction::GitClone { dest, .. }
            | InstallStepAction::ZipExtract { dest, .. }
            | InstallStepAction::PathCopy { dest, .. } => verify_installed_path(dest)?,
            InstallStepAction::PipInstall {
                package,
                version,
                python,
                ..
            } => verify_pip_package(package, version.as_deref(), python.as_deref())?,
            InstallStepAction::AdobePluginLink { dest, manifest, .. } => {
                adobe::verify_linked_plugin(dest, manifest.as_deref())?
            }
            InstallStepAction::WriteLaunchPlan { dcc_type, .. } => {
                verify_installed_path(&launch_plan_path(dcc_type))?
            }
            InstallStepAction::HostPackage {
                extract_to: Some(dest),
                ..
            } => verify_installed_path(dest)?,
            InstallStepAction::HostPackage { .. }
            | InstallStepAction::RegisterDcc { .. }
            | InstallStepAction::Verify => {}
        }
    }
    Ok(None)
}

fn verify_installed_path(path: &Path) -> Result<(), InstallError> {
    if !path.exists() {
        return Err(InstallError::StepFailed {
            step: "verify".into(),
            message: format!("installed path does not exist: {}", path.display()),
        });
    }
    if path.is_dir() && fs::read_dir(path)?.next().is_none() {
        return Err(InstallError::StepFailed {
            step: "verify".into(),
            message: format!("installed directory is empty: {}", path.display()),
        });
    }
    Ok(())
}

fn verify_pip_package(
    package: &str,
    expected_version: Option<&str>,
    python: Option<&str>,
) -> Result<(), InstallError> {
    let python_cmd = python.unwrap_or("python");
    let output = Command::new(python_cmd)
        .args(pip_show_args(package))
        .output()
        .map_err(|e| InstallError::StepFailed {
            step: "verify".into(),
            message: format!("failed to launch {python_cmd}: {e}"),
        })?;
    if !output.status.success() {
        return Err(InstallError::StepFailed {
            step: "verify".into(),
            message: format!("{python_cmd} could not verify installed pip package '{package}'"),
        });
    }
    if let Some(expected) = expected_version {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let actual = stdout
            .lines()
            .find_map(|line| line.strip_prefix("Version:"))
            .map(str::trim)
            .unwrap_or_default();
        if actual != expected {
            return Err(InstallError::StepFailed {
                step: "verify".into(),
                message: format!(
                    "installed {package} version mismatch: expected {expected}, got {actual}"
                ),
            });
        }
    }
    Ok(())
}

// ── rollback ─────────────────────────────────────────────────────────────────────

fn execute_rollback(rollback: &StepRollback) -> Result<(), InstallError> {
    match rollback {
        StepRollback::RemovePath(path) => {
            if path.exists() {
                if path.is_dir() {
                    std::fs::remove_dir_all(path)?;
                } else {
                    std::fs::remove_file(path)?;
                }
            }
            Ok(())
        }
        StepRollback::Command { program, args } => {
            let status = Command::new(program)
                .args(args)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map_err(|e| InstallError::RollbackFailed {
                    step: program.clone(),
                    message: format!("failed to launch {program}: {e}"),
                })?;
            if !status.success() {
                return Err(InstallError::RollbackFailed {
                    step: program.clone(),
                    message: format!("{program} exited with {status}"),
                });
            }
            Ok(())
        }
    }
}

/// Recursively copy a directory and its contents.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), InstallError> {
    if !dst.exists() {
        std::fs::create_dir_all(dst)?;
    }
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let file_name = entry.file_name();
        let dst_path = dst.join(&file_name);

        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

// ── tests ────────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "install/recovery_tests.rs"]
mod recovery_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::install::InstallStep;

    /// Serve the upstream catalog from the bundled slot, for tests of install
    /// mechanics that need its pip adapters.
    pub(super) fn with_upstream_catalog(mut service: InstallService) -> InstallService {
        use crate::domain::install_catalog::CatalogProvenance;
        use sha2::{Digest, Sha256};
        service.catalog_snapshot = Some(catalog::ResolvedCatalog {
            entries: meshy_dcc_catalog::load_from_str(FIXTURE_CATALOG).unwrap(),
            provenance: CatalogProvenance {
                sha256: Some(hex::encode(Sha256::digest(FIXTURE_CATALOG.as_bytes()))),
                ..CatalogProvenance::local(CatalogSource::Bundled)
            },
        });
        service
    }

    /// The signed remote catalog is checked against the entry schema; the
    /// bundled copy has to pass the same check.
    #[test]
    fn bundled_catalog_passes_the_entry_schema() {
        let entries = meshy_dcc_catalog::load_from_str(BUNDLED_CATALOG).unwrap();
        meshy_dcc_catalog::validate_catalog_entries(&entries).unwrap();
    }

    #[test]
    fn service_uses_bundled_catalog_when_default_path_is_missing() {
        let service = InstallService::new(PathBuf::from("__missing_meshy_dcc_catalog__.yml"));
        assert_eq!(
            service.catalog_provenance(None).source,
            CatalogSource::Bundled
        );
        let entries = service.catalog_entries(None).unwrap();
        let raw = meshy_dcc_catalog::load_from_str(BUNDLED_CATALOG).unwrap();
        let names = |list: &[meshy_dcc_catalog::CatalogEntry]| {
            list.iter()
                .map(|entry| entry.name.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&entries), names(&raw));
        // Every entry is narrowed to this platform's build.
        let platform = meshy_dcc_updater::meshy_release::current_platform().unwrap();
        for entry in &entries {
            if let Some(install) = &entry.install
                && let Some(build) = install.platforms.get(platform)
            {
                assert_eq!(
                    install.url.as_deref(),
                    Some(build.url.as_str()),
                    "{}",
                    entry.name
                );
            }
        }
        let listing = service.dcc_types(None).unwrap();
        let blender = listing
            .dcc_types
            .iter()
            .find(|entry| entry.dcc_type == "blender")
            .unwrap();
        assert_eq!(blender.adapters[0].name, "meshy-dcc-blender");
        assert!(!blender.adapters[0].catalog_install_available);
        let entry = entries
            .iter()
            .find(|entry| entry.name == "meshy-dcc-blender")
            .unwrap();
        assert!(entry.policy.as_ref().unwrap().reason.is_some());

        let error = service
            .plan(InstallRequest {
                dcc_type: "blender".into(),
                version: None,
                catalog_path: None,
                python: None,
                dcc_path: None,
                plugin_source: None,
                adobe_debug_root: None,
                project: None,
            })
            .unwrap_err();
        assert!(matches!(
            error,
            InstallError::Plan(InstallPlanError::NotAvailable { name, reason: Some(_) })
                if name == "meshy-dcc-blender"
        ));
    }

    #[test]
    fn bundled_entries_plan_or_report_not_available() {
        let entries = meshy_dcc_catalog::load_from_str(BUNDLED_CATALOG).unwrap();
        for entry in &entries {
            let result = InstallPlanner::plan(
                &entries,
                InstallRequest {
                    dcc_type: entry.dcc[0].clone(),
                    version: None,
                    catalog_path: None,
                    python: None,
                    dcc_path: None,
                    plugin_source: None,
                    adobe_debug_root: None,
                    project: None,
                },
            );
            let published = entry
                .policy
                .as_ref()
                .is_none_or(|policy| policy.installation == "available");
            match result {
                Ok(_) => assert!(published, "{}", entry.name),
                Err(InstallPlanError::NotAvailable { name, .. }) => {
                    assert!(!published && name == entry.name, "{}", entry.name)
                }
                Err(error) => panic!("{}: {error}", entry.name),
            }
        }
    }

    #[test]
    fn upstream_catalog_prefers_an_adapter_over_its_skill_pack() {
        let service = with_upstream_catalog(InstallService::new(PathBuf::from(
            "__missing_meshy_dcc_catalog__.yml",
        )));
        let plan = service
            .plan(InstallRequest {
                dcc_type: "unreal".into(),
                version: None,
                catalog_path: None,
                python: None,
                dcc_path: None,
                plugin_source: None,
                adobe_debug_root: None,
                project: None,
            })
            .unwrap();

        assert_eq!(plan.adapter.name, "meshy-dcc-unreal");
        assert!(matches!(
            plan.steps[0].action,
            Some(InstallStepAction::PipInstall { .. })
        ));
    }

    #[test]
    fn upstream_catalog_lists_adapter_dcc_types_and_collapses_aliases() {
        let service = with_upstream_catalog(InstallService::new(PathBuf::from(
            "__missing_meshy_dcc_catalog__.yml",
        )));
        let catalog = service.dcc_types(None).unwrap();

        assert!(catalog.custom_types_supported);
        assert_eq!(catalog.dcc_types.len(), 38);
        for alias in ["after effects", "after-effects", "comfy-ui"] {
            assert!(
                catalog
                    .dcc_types
                    .iter()
                    .all(|entry| entry.dcc_type != alias),
                "alias {alias} must collapse into its first canonical catalog spelling"
            );
        }
        assert_eq!(
            catalog
                .dcc_types
                .iter()
                .filter(|entry| entry.dcc_type == "3dsmax")
                .count(),
            1
        );
        for (expected_type, expected_adapter) in [
            ("c4d", "meshy-dcc-cinema4d"),
            ("comfyui", "meshy-dcc-comfyui"),
            ("freecad", "meshy-dcc-freecad"),
            ("gimp", "meshy-dcc-gimp"),
            ("godot", "meshy-dcc-godot"),
            ("krita", "meshy-dcc-krita"),
            ("kdenlive", "meshy-dcc-kdenlive"),
            ("liquigen", "meshy-dcc-liquigen"),
            ("mari", "meshy-dcc-mari"),
            ("material-maker", "meshy-dcc-material-maker"),
            ("obs", "meshy-dcc-obs"),
            ("openscad", "meshy-dcc-openscad"),
            ("powerpoint", "meshy-dcc-PowerPoint"),
            ("renderdoc", "meshy-dcc-renderdoc"),
            ("sketchup", "meshy-dcc-sketchup"),
            ("shogun", "meshy-dcc-shogun"),
            ("tiled", "meshy-dcc-tiled"),
            ("touchdesigner", "meshy-dcc-touchdesigner"),
            ("unity", "meshy-dcc-unity"),
            ("wwise", "meshy-dcc-wwise"),
        ] {
            let entry = catalog
                .dcc_types
                .iter()
                .find(|entry| entry.dcc_type == expected_type)
                .unwrap_or_else(|| panic!("missing cataloged DCC type {expected_type}"));
            assert!(
                entry
                    .adapters
                    .iter()
                    .any(|adapter| adapter.name == expected_adapter)
            );
        }
    }

    #[test]
    fn install_plan_reports_disabled_auto_install_policy_with_custom_prompt() {
        let service = with_upstream_catalog(InstallService::with_auto_install_policy(
            PathBuf::from("__missing_meshy_dcc_catalog__.yml"),
            AutoInstallPolicy::disabled(
                "Auto install unavailable; contact PipelineTD to deploy {adapter} for {dcc_type}.",
            ),
        ));
        let plan = service
            .plan(InstallRequest {
                dcc_type: "maya".into(),
                version: None,
                catalog_path: None,
                python: None,
                dcc_path: None,
                plugin_source: None,
                adobe_debug_root: None,
                project: None,
            })
            .unwrap();

        assert!(!plan.install_policy.auto_install_enabled);
        assert_eq!(
            plan.install_policy.prompt.as_deref(),
            Some("Auto install unavailable; contact PipelineTD to deploy meshy-dcc-maya for maya.")
        );
    }

    #[test]
    fn execute_reports_preflight_failure_when_auto_install_is_disabled() {
        let service = with_upstream_catalog(InstallService::with_auto_install_policy(
            PathBuf::from("__missing_meshy_dcc_catalog__.yml"),
            AutoInstallPolicy::disabled(
                "Automatic install disabled; ask PipelineTD for {adapter}.",
            ),
        ));

        let report = service.execute(
            InstallRequest {
                dcc_type: "maya".into(),
                version: None,
                catalog_path: None,
                python: Some("/__nonexistent__/python".into()),
                dcc_path: None,
                plugin_source: None,
                adobe_debug_root: None,
                project: None,
            },
            true,
        );

        assert_eq!(report.status, "failed");
        assert_eq!(report.stage, "preflight");
        assert_eq!(report.exit_code, 10);
        assert_eq!(report.error.as_ref().unwrap().code, "AUTO_INSTALL_DISABLED");
        assert!(report.steps.iter().all(|step| step.status == "not_run"));
        assert!(!report.verify.directly_usable);
        assert_eq!(report.verify.failure_stage.as_deref(), Some("preflight"));
        assert_eq!(
            report.verify.failure_reason.as_deref(),
            Some("AUTO_INSTALL_DISABLED")
        );
    }

    #[test]
    fn pip_install_missing_python_reports_error() {
        let result = execute_pip_install(
            "nonexistent-package",
            Some("1.0.0"),
            None,
            Some("/__nonexistent__/python"),
            Some(
                "https://files.pythonhosted.org/packages/example/nonexistent_package-1.0.0-py3-none-any.whl",
            ),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(&err, InstallError::StepFailed { step, .. } if step.contains("pip-install")),
            "expected StepFailed, got {err}"
        );
    }

    #[test]
    fn git_clone_nonexistent_url_fails() {
        let dest = PathBuf::from("/__nonexistent__/test-repo");
        let result = execute_git_clone("https://__nonexistent__.invalid/repo.git", None, &dest);
        assert!(result.is_err());
    }

    #[test]
    fn git_and_zip_integrity_fail_before_network_or_filesystem_io() {
        let root = tempfile::tempdir().unwrap();
        let git_dest = root.path().join("git-install");
        let git_error = execute_git_clone(
            "https://__nonexistent__.invalid/repo.git",
            Some("main"),
            &git_dest,
        )
        .unwrap_err();
        assert!(git_error.to_string().contains("40-character commit"));
        assert!(!git_dest.exists());

        let zip_dest = root.path().join("zip-install");
        let zip_error = execute_zip_extract(
            "https://__nonexistent__.invalid/archive.zip",
            None,
            &zip_dest,
        )
        .unwrap_err();
        assert!(zip_error.to_string().contains("64 hexadecimal"));
        assert!(!zip_dest.exists());

        assert_eq!(
            required_archive_sha256(Some(&format!("sha256:{}", "A".repeat(64)))).unwrap(),
            "a".repeat(64)
        );
    }

    #[test]
    fn path_copy_missing_source_fails() {
        let result = execute_path_copy(
            &PathBuf::from("/__nonexistent__/source"),
            &PathBuf::from("/__nonexistent__/dest"),
        );
        assert!(result.is_err());
    }

    #[test]
    fn rollback_remove_path_does_not_error_on_nonexistent() {
        let rb = StepRollback::RemovePath(PathBuf::from("/__nonexistent__/path"));
        assert!(execute_rollback(&rb).is_ok());
    }

    #[test]
    fn register_dcc_is_explicitly_deferred() {
        let result = execute_register_dcc("maya", Some("meshy_dcc_maya.cli:main"), None);
        assert!(matches!(result, StepExecution::Deferred));
    }

    #[test]
    fn pip_install_uses_verified_direct_artifact_reference() {
        let spec = verified_pip_artifact_spec(
            "meshy-dcc-maya",
            Some("0.9.22"),
            Some(&["maya".to_string()]),
            Some(
                "https://files.pythonhosted.org/packages/example/meshy_dcc_maya-0.9.22-py3-none-any.whl",
            ),
            Some("A".repeat(64).as_str()),
        )
        .unwrap();
        assert_eq!(
            pip_install_args(&spec),
            vec![
                "-m".to_string(),
                "pip".to_string(),
                "install".to_string(),
                "--upgrade".to_string(),
                format!(
                    "meshy-dcc-maya[maya] @ https://files.pythonhosted.org/packages/example/meshy_dcc_maya-0.9.22-py3-none-any.whl#sha256={}",
                    "a".repeat(64)
                ),
            ]
        );

        let error = verified_pip_artifact_spec(
            "meshy-dcc-unity",
            Some("0.11.2"),
            None,
            Some("https://pypi.org/project/meshy-dcc-unity"),
            None,
        );
        assert!(error.unwrap_err().to_string().contains("pip-integrity"));
        assert_eq!(
            pip_uninstall_args("meshy-dcc-maya"),
            vec![
                "-m".to_string(),
                "pip".to_string(),
                "uninstall".to_string(),
                "-y".to_string(),
                "meshy-dcc-maya".to_string(),
            ]
        );
        assert_eq!(
            pip_show_args("meshy-dcc-maya"),
            vec![
                "-m".to_string(),
                "pip".to_string(),
                "show".to_string(),
                "meshy-dcc-maya".to_string(),
            ]
        );
    }

    fn verify_plan(action: InstallStepAction) -> InstallPlan {
        InstallPlan {
            catalog: None,
            dcc_type: "maya".into(),
            version: None,
            dcc_path: None,
            adapter: meshy_dcc_catalog::CatalogEntry {
                name: "meshy-dcc-maya".into(),
                description: "Maya adapter".into(),
                dcc: vec!["maya".into()],
                targets: vec![],
                url: None,
                issues_url: None,
                tags: vec![],
                version: None,
                min_core_version: None,
                install: None,
                package: None,
                maintainer: None,
                category: None,
                policy: None,
                requires: None,
                icon: None,
                showcase: None,
            },
            steps: vec![
                InstallStep {
                    name: "install".into(),
                    description: "Install adapter".into(),
                    action: Some(action),
                },
                InstallStep {
                    name: "verify".into(),
                    description: "Verify adapter".into(),
                    action: Some(InstallStepAction::Verify),
                },
            ],
            next_steps: vec![],
            install_policy: InstallPolicy::enabled(),
        }
    }

    #[test]
    fn verify_accepts_non_empty_installed_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let source = tmp.path().join("source");
        let dest = tmp.path().join("dest");
        fs::create_dir_all(&dest).unwrap();
        fs::write(dest.join("adapter.txt"), "installed").unwrap();

        let result = execute_verify(&verify_plan(InstallStepAction::PathCopy { source, dest }));
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn verify_rejects_missing_installed_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let source = tmp.path().join("source");
        let dest = tmp.path().join("missing");

        let err = execute_verify(&verify_plan(InstallStepAction::PathCopy {
            source,
            dest: dest.clone(),
        }))
        .unwrap_err();
        assert!(
            matches!(&err, InstallError::StepFailed { step, message }
                if step == "verify" && message.contains(&dest.display().to_string())),
            "expected verify StepFailed, got {err}"
        );
    }

    #[test]
    fn verify_rejects_empty_installed_directory() {
        let tmp = tempfile::TempDir::new().unwrap();
        let source = tmp.path().join("source");
        let dest = tmp.path().join("empty");
        fs::create_dir_all(&dest).unwrap();

        let err = execute_verify(&verify_plan(InstallStepAction::PathCopy {
            source,
            dest: dest.clone(),
        }))
        .unwrap_err();
        assert!(
            matches!(&err, InstallError::StepFailed { step, message }
                if step == "verify" && message.contains("installed directory is empty")),
            "expected verify StepFailed, got {err}"
        );
    }

    #[test]
    fn execution_report_success_keeps_live_dcc_readiness_unproven() {
        let service = InstallService::new(PathBuf::from("__unused__.yml"));
        let plan = verify_plan(InstallStepAction::PathCopy {
            source: PathBuf::from("source"),
            dest: PathBuf::from("dest"),
        });

        let report = service.execute_plan_with(
            &plan,
            true,
            |_action, _plan| Ok(StepExecution::Completed(None)),
            |_rollback| Ok(()),
        );

        assert_eq!(report.status, "ok");
        assert_eq!(report.stage, "complete");
        assert_eq!(report.exit_code, 0);
        assert!(report.error.is_none());
        assert_eq!(report.steps[0].id, "install-path");
        assert_eq!(report.steps[0].status, "ok");
        assert_eq!(report.steps[0].rollback.status, "not_available");
        assert_eq!(report.steps[1].id, "verify");
        assert_eq!(report.steps[1].status, "ok");
        assert!(!report.verify.directly_usable);
        assert_eq!(
            report.verify.failure_stage.as_deref(),
            Some("host-readiness")
        );
        assert_eq!(
            report.verify.failure_reason.as_deref(),
            Some("LIVE_DCC_VERIFICATION_REQUIRED")
        );
        assert_eq!(report.receipt_path, None);

        let mut actual = serde_json::to_value(report).unwrap();
        actual["core_version"] = serde_json::json!("0.0.0-test");
        let expected: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/install-execution-report-v1-success.json"
        ))
        .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn execution_report_mid_failure_rolls_back_completed_steps_in_reverse() {
        let service = InstallService::new(PathBuf::from("__unused__.yml"));
        let mut plan = verify_plan(InstallStepAction::PathCopy {
            source: PathBuf::from("source"),
            dest: PathBuf::from("dest"),
        });
        plan.steps.insert(
            1,
            InstallStep {
                name: "register-dcc".into(),
                description: "Register adapter".into(),
                action: Some(InstallStepAction::RegisterDcc {
                    dcc_type: "maya".into(),
                    entry_point: None,
                    dcc_path: None,
                }),
            },
        );
        let mut calls = 0;
        let mut rollback_order = Vec::new();

        let report = service.execute_plan_with(
            &plan,
            true,
            |_action, _plan| {
                calls += 1;
                if calls == 1 {
                    Ok(StepExecution::Completed(Some(StepRollback::RemovePath(
                        PathBuf::from("secret-a"),
                    ))))
                } else {
                    Err(InstallError::StepFailed {
                        step: "secret-stage".into(),
                        message: "token=super-secret".into(),
                    })
                }
            },
            |rollback| {
                if let StepRollback::RemovePath(path) = rollback {
                    rollback_order.push(path.clone());
                }
                Ok(())
            },
        );

        assert_eq!(rollback_order, vec![PathBuf::from("secret-a")]);
        assert_eq!(report.status, "failed");
        assert_eq!(report.stage, "install");
        assert_eq!(report.exit_code, 30);
        assert_eq!(report.error.as_ref().unwrap().code, "INSTALL_STEP_FAILED");
        assert_eq!(report.steps[0].status, "ok");
        assert!(report.steps[0].rollback.attempted);
        assert_eq!(report.steps[0].rollback.status, "ok");
        assert_eq!(report.steps[1].status, "failed");
        assert_eq!(report.steps[2].status, "not_run");
        assert!(report.rollback.attempted);
        assert_eq!(report.rollback.status, "ok");
        assert_eq!(report.rollback.failure_count, 0);
        assert_eq!(
            report.verify.failure_reason.as_deref(),
            Some("INSTALL_STEP_FAILED")
        );

        let public = serde_json::to_string(&report).unwrap();
        assert!(!public.contains("secret-a"));
        assert!(!public.contains("secret-stage"));
        assert!(!public.contains("super-secret"));
    }

    #[test]
    fn execution_report_rollback_failure_is_partial_and_stable() {
        let service = InstallService::new(PathBuf::from("__unused__.yml"));
        let mut plan = verify_plan(InstallStepAction::PathCopy {
            source: PathBuf::from("source"),
            dest: PathBuf::from("dest"),
        });
        plan.steps.insert(
            1,
            InstallStep {
                name: "register-dcc".into(),
                description: "Register adapter".into(),
                action: Some(InstallStepAction::RegisterDcc {
                    dcc_type: "maya".into(),
                    entry_point: None,
                    dcc_path: None,
                }),
            },
        );
        let mut calls = 0;

        let report = service.execute_plan_with(
            &plan,
            true,
            |_action, _plan| {
                calls += 1;
                if calls == 1 {
                    Ok(StepExecution::Completed(Some(StepRollback::RemovePath(
                        PathBuf::from("secret-path"),
                    ))))
                } else {
                    Err(InstallError::StepFailed {
                        step: "install".into(),
                        message: "primary secret".into(),
                    })
                }
            },
            |_rollback| {
                Err(InstallError::RollbackFailed {
                    step: "secret-program".into(),
                    message: "rollback secret".into(),
                })
            },
        );

        assert_eq!(report.status, "partial");
        assert_eq!(report.stage, "rollback");
        assert_eq!(report.exit_code, 30);
        let error = report.error.as_ref().unwrap();
        assert_eq!(error.code, "ROLLBACK_FAILED");
        assert_eq!(error.primary_code.as_deref(), Some("INSTALL_STEP_FAILED"));
        assert_eq!(report.steps[0].rollback.status, "failed");
        assert_eq!(report.rollback.status, "failed");
        assert_eq!(report.rollback.failure_count, 1);
        assert_eq!(
            report.verify.failure_reason.as_deref(),
            Some("ROLLBACK_FAILED")
        );

        let public = serde_json::to_string(&report).unwrap();
        for secret in [
            "secret-path",
            "secret-program",
            "primary secret",
            "rollback secret",
        ] {
            assert!(!public.contains(secret));
        }

        let mut actual = serde_json::to_value(report).unwrap();
        actual["core_version"] = serde_json::json!("0.0.0-test");
        let expected: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/install-execution-report-v1-rollback-failed.json"
        ))
        .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn failed_execution_report_matches_shared_cross_language_fixture() {
        let service = InstallService::new(PathBuf::from("__unused__.yml"));
        let mut plan = verify_plan(InstallStepAction::PathCopy {
            source: PathBuf::from("source"),
            dest: PathBuf::from("dest"),
        });
        plan.steps.insert(
            1,
            InstallStep {
                name: "register-dcc".into(),
                description: "Register adapter".into(),
                action: Some(InstallStepAction::RegisterDcc {
                    dcc_type: "maya".into(),
                    entry_point: None,
                    dcc_path: None,
                }),
            },
        );
        let mut calls = 0;
        let report = service.execute_plan_with(
            &plan,
            true,
            |_action, _plan| {
                calls += 1;
                if calls == 1 {
                    Ok(StepExecution::Completed(Some(StepRollback::RemovePath(
                        PathBuf::from("private"),
                    ))))
                } else {
                    Err(InstallError::StepFailed {
                        step: "private".into(),
                        message: "private".into(),
                    })
                }
            },
            |_rollback| Ok(()),
        );
        let mut actual = serde_json::to_value(report).unwrap();
        actual["core_version"] = serde_json::json!("0.0.0-test");
        let expected: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/install-execution-report-v1-failed.json"
        ))
        .unwrap();

        assert_eq!(actual, expected);
    }
}
