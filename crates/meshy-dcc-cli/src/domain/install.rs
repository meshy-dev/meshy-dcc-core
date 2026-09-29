use std::path::PathBuf;

use meshy_dcc_catalog::{CatalogAdobeInstall, CatalogEntry, CatalogInstall};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub(crate) mod policy;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRequest {
    pub dcc_type: String,
    pub version: Option<String>,
    pub catalog_path: Option<PathBuf>,
    pub python: Option<String>,
    /// Optional absolute DCC executable or application path supplied by the user.
    pub dcc_path: Option<PathBuf>,
    /// Source checkout or internal package root containing the host plugin.
    pub plugin_source: Option<PathBuf>,
    /// Adobe UXP/CEP debug root supplied by the operator or studio profile.
    pub adobe_debug_root: Option<PathBuf>,
    /// Project to set up with the adapter's `project_steps`.
    pub project: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
struct InstallOperatorPaths {
    plugin_source: Option<PathBuf>,
    adobe_debug_root: Option<PathBuf>,
    project: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstallPlan {
    pub dcc_type: String,
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dcc_path: Option<PathBuf>,
    pub adapter: CatalogEntry,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog: Option<crate::domain::install_catalog::CatalogProvenance>,
    pub steps: Vec<InstallStep>,
    #[serde(default)]
    pub next_steps: Vec<InstallNextStep>,
    #[serde(default = "InstallPolicy::enabled")]
    pub install_policy: InstallPolicy,
}

/// A single install step with a human-readable description and the action to execute.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstallStep {
    pub name: String,
    pub description: String,
    /// The executable action for this step. `None` for informational/display-only steps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<InstallStepAction>,
}

/// A machine-readable post-install action that gets a live DCC under CLI control.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstallNextStep {
    pub name: String,
    pub description: String,
    /// Optional document URL for agent-facing instructions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Command arguments to run exactly as a process argv vector. `None` means manual host action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<Vec<String>>,
    pub requires_live_instance: bool,
}

/// Environment or studio policy controlling whether the CLI may execute installs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstallPolicy {
    pub auto_install_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}

impl InstallPolicy {
    pub fn enabled() -> Self {
        Self {
            auto_install_enabled: true,
            prompt: None,
        }
    }

    pub fn disabled(prompt: impl Into<String>) -> Self {
        Self {
            auto_install_enabled: false,
            prompt: Some(prompt.into()),
        }
    }
}

/// The concrete action to perform during install execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum InstallStepAction {
    /// Install a Python package via pip (optionally with a specific interpreter).
    PipInstall {
        /// Pip package name (e.g. "meshy-dcc-maya").
        package: String,
        /// Optional exact package version requested by the user.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        version: Option<String>,
        /// Optional pip extras (e.g. ["maya"]).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        extras: Option<Vec<String>>,
        /// Optional Python/mayapy interpreter path override.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        python: Option<String>,
        /// Immutable wheel artifact URL declared by the catalog.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        artifact_url: Option<String>,
        /// Required SHA-256 for the declared wheel artifact.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sha256: Option<String>,
    },
    /// Check out a git repository at an immutable commit.
    GitClone {
        /// Git remote URL.
        url: String,
        /// Full 40-character commit object ID to check out.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ref_: Option<String>,
        /// Target directory for the clone.
        dest: PathBuf,
    },
    /// Download, verify, and extract a ZIP archive.
    ZipExtract {
        /// Archive download URL.
        url: String,
        /// Required SHA-256 hash for integrity verification.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sha256: Option<String>,
        /// Target extract directory.
        dest: PathBuf,
    },
    /// Copy files from a local path.
    PathCopy {
        /// Source directory or file.
        source: PathBuf,
        /// Target directory.
        dest: PathBuf,
    },
    /// Create an Adobe debug-mode directory link without copying plugin files.
    AdobePluginLink {
        source: PathBuf,
        dest: PathBuf,
        product: String,
        extension_type: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        manifest: Option<PathBuf>,
    },
    /// Download a host-native package and install it by running host commands.
    HostPackage {
        /// Artifact URL declared by the catalog.
        url: String,
        /// Required SHA-256 of the artifact.
        sha256: String,
        /// Host executable the operator supplied (`{dcc_path}`). Commands never
        /// go through a shell.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dcc_path: Option<PathBuf>,
        /// Where the zip package is extracted (`{package_dir}`), when the
        /// catalog asks for extraction.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        extract_to: Option<PathBuf>,
        /// argv templates run first, exit status ignored, so a reinstall starts clean.
        uninstall: Vec<Vec<String>>,
        /// argv templates that install `{package}`, run in order.
        steps: Vec<Vec<String>>,
        /// Project set up by `project_steps` (`{project}`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project: Option<PathBuf>,
        /// argv templates run after `steps` when a project is given.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        project_steps: Vec<Vec<String>>,
    },
    /// Write the start-instance launch plan for this host.
    WriteLaunchPlan {
        dcc_type: String,
        executable: PathBuf,
        argv: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        project_markers: Vec<String>,
    },
    /// Register the DCC adapter with the gateway.
    RegisterDcc {
        /// DCC type name (e.g. "maya").
        dcc_type: String,
        /// Optional Python entry point for the adapter.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        entry_point: Option<String>,
        /// Optional user-provided DCC executable/application path.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dcc_path: Option<PathBuf>,
    },
    /// Verify local install artefacts. Live DCC readiness is a next step.
    Verify,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InstallPlanError {
    #[error("no catalog entry targets dcc type '{0}'")]
    UnsupportedDcc(String),
    #[error("no install metadata found in catalog entry for '{0}'")]
    MissingInstallMetadata(String),
    #[error("catalog entry '{0}' is not available for installation")]
    NotAvailable(String),
    #[error("catalog entry '{name}' has unsupported installation policy '{installation}'")]
    InvalidInstallationPolicy { name: String, installation: String },
    #[error("catalog entry '{name}' has invalid min_core_version '{required}'")]
    InvalidMinCoreVersion { name: String, required: String },
    #[error(
        "catalog entry '{name}' requires meshy-dcc-core >= {required}; current version is {current}"
    )]
    IncompatibleCoreVersion {
        name: String,
        required: String,
        current: String,
    },
    #[error("git install requires a full 40-character commit object ID")]
    UnpinnedGitReference,
    #[error("zip install requires exactly 64 hexadecimal SHA-256 digits")]
    InvalidArchiveChecksum,
    #[error("pip install requires a package, version, universal wheel URL, and SHA-256")]
    InvalidPipArtifact,
    #[error("requested pip version '{requested}' differs from catalog version '{catalog}'")]
    PipVersionMismatch { requested: String, catalog: String },
    #[error("host-package install is invalid: {0}")]
    InvalidHostPackage(&'static str),
}

// ── defaults ─────────────────────────────────────────────────────────────────────

/// Default install paths relative to user home or Meshy DCC data root.
pub fn default_adapter_dir() -> PathBuf {
    dirs_data_dir().join("adapters")
}

fn dirs_data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("MESHY_DCC_DATA_DIR").filter(|dir| !dir.is_empty()) {
        return PathBuf::from(dir);
    }
    dirs::data_dir()
        .map(|p| p.join("meshy-dcc"))
        .unwrap_or_else(|| PathBuf::from("~/.local/share/meshy-dcc"))
}

// ── planner ──────────────────────────────────────────────────────────────────────

pub struct InstallPlanner;

fn validate_install_integrity(
    entry: &CatalogEntry,
    install: &CatalogInstall,
    requested_version: Option<&str>,
) -> Result<(), InstallPlanError> {
    match install.install_type.as_str() {
        "git" => {
            let reference = install.ref_.as_deref().unwrap_or_default().trim();
            if reference.len() != 40 || !reference.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(InstallPlanError::UnpinnedGitReference);
            }
        }
        "zip" => {
            let value = install.sha256.as_deref().unwrap_or_default().trim();
            let checksum = value.strip_prefix("sha256:").unwrap_or(value);
            if checksum.len() != 64 || !checksum.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(InstallPlanError::InvalidArchiveChecksum);
            }
        }
        "host-package" => validate_host_package(install)?,
        "pip" => {
            let package = install.pip_package.as_deref().unwrap_or_default().trim();
            let catalog_version = entry.version.as_deref().unwrap_or_default().trim();
            let artifact_url = install.url.as_deref().unwrap_or_default().trim();
            let value = install.sha256.as_deref().unwrap_or_default().trim();
            let checksum = value.strip_prefix("sha256:").unwrap_or(value);
            let normalized_package = package
                .chars()
                .map(|character| match character {
                    '-' | '.' => '_',
                    other => other.to_ascii_lowercase(),
                })
                .collect::<String>();
            let filename = artifact_url.rsplit('/').next().unwrap_or_default();
            let expected_prefix = format!("{normalized_package}-{catalog_version}-");
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
            let valid_extras = install.pip_extras.as_ref().is_none_or(|values| {
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
                        && value.bytes().all(|byte| {
                            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
                        })
                })
            });
            let valid_artifact = artifact_url.starts_with("https://")
                && !artifact_url.contains('#')
                && !artifact_url.contains('?')
                && filename
                    .to_ascii_lowercase()
                    .starts_with(&expected_prefix.to_ascii_lowercase())
                && filename.ends_with("-py3-none-any.whl");
            if !valid_package
                || !valid_extras
                || catalog_version.is_empty()
                || !valid_artifact
                || checksum.len() != 64
                || !checksum.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(InstallPlanError::InvalidPipArtifact);
            }
            if let Some(requested) = requested_version
                && requested.trim() != catalog_version
            {
                return Err(InstallPlanError::PipVersionMismatch {
                    requested: requested.into(),
                    catalog: catalog_version.into(),
                });
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_host_package(install: &CatalogInstall) -> Result<(), InstallPlanError> {
    let value = install.sha256.as_deref().unwrap_or_default().trim();
    let checksum = value.strip_prefix("sha256:").unwrap_or(value);
    if checksum.len() != 64 || !checksum.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(InstallPlanError::InvalidArchiveChecksum);
    }
    let url = install.url.as_deref().unwrap_or_default();
    if host_package_file_name(url).is_none() {
        return Err(InstallPlanError::InvalidHostPackage(
            "url must be https (or http on loopback) without userinfo, query or fragment, ending in a plain file name",
        ));
    }
    let host = install
        .host
        .as_ref()
        .ok_or(InstallPlanError::InvalidHostPackage(
            "missing host commands",
        ))?;
    if host.steps.is_empty() && host.project_steps.is_empty() && !host.extract {
        return Err(InstallPlanError::InvalidHostPackage(
            "host has no steps, project_steps or extract",
        ));
    }
    let commands = || {
        host.steps
            .iter()
            .chain(&host.uninstall)
            .chain(&host.project_steps)
    };
    if !commands().all(|argv| host_program_is_allowed(argv.first(), host.extract)) {
        return Err(InstallPlanError::InvalidHostPackage(
            "every host command must start with {dcc_path}, or {package_dir}/<relative path> when extract is set",
        ));
    }
    if host
        .steps
        .iter()
        .chain(&host.uninstall)
        .flatten()
        .any(|token| token.contains("{project}"))
    {
        return Err(InstallPlanError::InvalidHostPackage(
            "{project} is only available in project_steps",
        ));
    }
    if host
        .launch
        .as_ref()
        .is_some_and(|argv| argv.first().map(String::as_str) != Some("{executable}"))
    {
        return Err(InstallPlanError::InvalidHostPackage(
            "host.launch must start with {executable}",
        ));
    }
    Ok(())
}

/// A host command runs the operator's `{dcc_path}` or, for an extracted
/// package, a file named by a plain relative path inside `{package_dir}`.
fn host_program_is_allowed(program: Option<&String>, extract: bool) -> bool {
    match program.map(String::as_str) {
        Some("{dcc_path}") => true,
        Some(program) if extract => program.strip_prefix("{package_dir}/").is_some_and(|rest| {
            !rest.contains(['\\', '{', '}'])
                && rest
                    .split('/')
                    .all(|part| !part.is_empty() && part != "." && part != "..")
        }),
        _ => false,
    }
}

/// File name of an allowed host-package URL: https anywhere, plain http only
/// on loopback, no userinfo, query or fragment, and a name that cannot escape
/// the download directory.
pub(crate) fn host_package_file_name(url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url).ok()?;
    let loopback = matches!(parsed.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
    if !(parsed.scheme() == "https" || (parsed.scheme() == "http" && loopback))
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return None;
    }
    let name = parsed.path_segments()?.next_back()?;
    let valid = name
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphanumeric)
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
    valid.then(|| name.to_string())
}

impl InstallPlanner {
    /// Generate an install plan from catalog entries and user request.
    ///
    /// If the matching catalog entry has an `install` field, the generated steps
    /// will include executable actions.  Otherwise only informational steps are
    /// produced (for display-only plan output).
    pub fn plan(
        entries: &[CatalogEntry],
        request: InstallRequest,
    ) -> Result<InstallPlan, InstallPlanError> {
        let dcc_key = normalized_dcc_key(&request.dcc_type);
        let adapter = select_adapter(entries, &dcc_key)
            .cloned()
            .ok_or_else(|| InstallPlanError::UnsupportedDcc(request.dcc_type.clone()))?;

        policy::validate(&adapter)?;

        let dcc_type = request.dcc_type.clone();
        let version = request.version.clone().or_else(|| adapter.version.clone());
        let dcc_path = request.dcc_path.clone();

        if let Some(install) = &adapter.install {
            validate_install_integrity(&adapter, install, request.version.as_deref())?;
        }

        let steps = match &adapter.install {
            Some(install) => Self::build_executable_steps(
                &adapter,
                install,
                &dcc_type,
                version.clone(),
                request.python.clone(),
                dcc_path.clone(),
                InstallOperatorPaths {
                    plugin_source: request.plugin_source.clone(),
                    adobe_debug_root: request.adobe_debug_root.clone(),
                    project: request.project.clone(),
                },
            ),
            None => Self::build_info_steps(),
        };

        let next_steps = Self::build_next_steps(&adapter, &dcc_type, dcc_path.as_deref());

        Ok(InstallPlan {
            next_steps,
            dcc_type,
            version,
            dcc_path,
            adapter,
            catalog: None,
            steps,
            install_policy: InstallPolicy::enabled(),
        })
    }

    /// Build executable steps from a catalog entry's `install` metadata.
    fn build_executable_steps(
        entry: &CatalogEntry,
        install: &CatalogInstall,
        dcc_type: &str,
        version: Option<String>,
        python_override: Option<String>,
        dcc_path: Option<PathBuf>,
        operator_paths: InstallOperatorPaths,
    ) -> Vec<InstallStep> {
        let adapter_dir = default_adapter_dir().join(&entry.name);

        let mut steps = Vec::new();
        let install_action = match install.install_type.as_str() {
            "pip" => {
                let python = python_override.or_else(|| install.python_path.clone());
                InstallStepAction::PipInstall {
                    package: install
                        .pip_package
                        .clone()
                        .unwrap_or_else(|| entry.name.clone()),
                    version,
                    extras: install.pip_extras.clone(),
                    python,
                    artifact_url: install.url.clone(),
                    sha256: install.sha256.clone(),
                }
            }
            "git" => InstallStepAction::GitClone {
                url: install
                    .url
                    .clone()
                    .unwrap_or_else(|| format!("https://github.com/meshy-dev/{}", entry.name)),
                ref_: install.ref_.clone(),
                dest: adapter_dir.clone(),
            },
            "zip" => InstallStepAction::ZipExtract {
                url: install.url.clone().unwrap_or_default(),
                sha256: install.sha256.clone(),
                dest: adapter_dir.clone(),
            },
            "path" => {
                let source = install
                    .url
                    .clone()
                    .map(|u| {
                        u.strip_prefix("file://")
                            .map(PathBuf::from)
                            .unwrap_or(PathBuf::from(&u))
                    })
                    .unwrap_or_else(|| PathBuf::from("."));
                InstallStepAction::PathCopy {
                    source,
                    dest: adapter_dir.clone(),
                }
            }
            "host-package" => {
                return Self::build_host_package_steps(
                    entry,
                    install,
                    dcc_type,
                    dcc_path,
                    operator_paths.project,
                );
            }
            other => {
                // Unknown install type — produce an info step instead.
                return vec![InstallStep {
                    name: format!("install-{}", other),
                    description: format!(
                        "Unsupported install type '{other}': manual installation required."
                    ),
                    action: None,
                }];
            }
        };

        steps.push(InstallStep {
            name: format!("install-{}", install.install_type),
            description: format!("Install {} adapter via {}", dcc_type, install.install_type),
            action: Some(install_action),
        });

        if let Some(adobe) = &install.adobe {
            steps.push(Self::build_adobe_link_step(
                entry,
                adobe,
                operator_paths.plugin_source.as_deref(),
                operator_paths.adobe_debug_root.as_deref(),
            ));
        }

        // Register step
        steps.push(InstallStep {
            name: "register-dcc".into(),
            description: format!(
                "Start or enable the {} host plugin so its sidecar self-registers",
                dcc_type
            ),
            action: Some(InstallStepAction::RegisterDcc {
                dcc_type: dcc_type.to_string(),
                entry_point: install.entry_point.clone(),
                dcc_path,
            }),
        });

        // Verify step
        steps.push(InstallStep {
            name: "verify".into(),
            description: "Verify installed package or file artefacts before the DCC plugin starts."
                .into(),
            action: Some(InstallStepAction::Verify),
        });

        steps
    }

    fn build_host_package_steps(
        entry: &CatalogEntry,
        install: &CatalogInstall,
        dcc_type: &str,
        dcc_path: Option<PathBuf>,
        project: Option<PathBuf>,
    ) -> Vec<InstallStep> {
        let host = install.host.clone().unwrap_or_default();
        let needs_dcc_path = host.launch.is_some()
            || host
                .steps
                .iter()
                .chain(&host.uninstall)
                .chain(&host.project_steps)
                .flatten()
                .any(|token| token.contains("{dcc_path}"));
        if needs_dcc_path && dcc_path.is_none() {
            return vec![InstallStep {
                name: "install-host-package".into(),
                description: format!(
                    "Provide --dcc-path with the {dcc_type} executable to install this adapter."
                ),
                action: None,
            }];
        }
        if !host.project_steps.is_empty() && project.is_none() {
            return vec![InstallStep {
                name: "install-host-package".into(),
                description: format!(
                    "Provide --project with the {dcc_type} project to set up; this adapter installs into each project."
                ),
                action: None,
            }];
        }
        // Each package version gets its own directory, so a host still running
        // an older version keeps its files.
        let extract_to = host.extract.then(|| {
            let url = install.url.as_deref().unwrap_or_default();
            let name = host_package_file_name(url).unwrap_or_default();
            let stem = name.strip_suffix(".zip").unwrap_or(&name).to_string();
            default_adapter_dir().join(&entry.name).join(stem)
        });
        // The catalog's own spelling names the launch plan, so start-instance
        // finds it under the same DCC type.
        let plan_dcc = entry
            .dcc
            .first()
            .cloned()
            .unwrap_or_else(|| dcc_type.into());
        let mut steps = vec![InstallStep {
            name: "install-host-package".into(),
            description: format!("Install the {dcc_type} adapter package with the host"),
            action: Some(InstallStepAction::HostPackage {
                url: install.url.clone().unwrap_or_default(),
                sha256: install.sha256.clone().unwrap_or_default(),
                dcc_path: dcc_path.clone(),
                extract_to,
                uninstall: host.uninstall,
                steps: host.steps,
                project: project.clone(),
                project_steps: host.project_steps,
            }),
        }];
        if let (Some(argv), Some(dcc_path)) = (host.launch, dcc_path.clone()) {
            steps.push(InstallStep {
                name: "write-launch-plan".into(),
                description: format!("Record how start-instance launches {dcc_type}"),
                action: Some(InstallStepAction::WriteLaunchPlan {
                    dcc_type: plan_dcc,
                    executable: dcc_path.clone(),
                    argv,
                    project_markers: host.project_markers,
                }),
            });
        }
        steps.push(InstallStep {
            name: "register-dcc".into(),
            description: format!("Start {dcc_type} so the adapter self-registers"),
            action: Some(InstallStepAction::RegisterDcc {
                dcc_type: dcc_type.to_string(),
                entry_point: None,
                dcc_path,
            }),
        });
        steps.push(InstallStep {
            name: "verify".into(),
            description: "Verify the host executable and launch plan exist.".into(),
            action: Some(InstallStepAction::Verify),
        });
        steps
    }

    fn build_adobe_link_step(
        entry: &CatalogEntry,
        adobe: &CatalogAdobeInstall,
        plugin_source: Option<&std::path::Path>,
        adobe_debug_root: Option<&std::path::Path>,
    ) -> InstallStep {
        let source = plugin_source.map(|root| {
            adobe
                .source_subpath
                .as_deref()
                .map(|relative| root.join(relative))
                .unwrap_or_else(|| root.to_path_buf())
        });
        let dest = adobe_debug_root.map(|root| {
            adobe
                .target_subpath
                .as_deref()
                .or(adobe.plugin_id.as_deref())
                .map(|relative| root.join(relative))
                .unwrap_or_else(|| root.to_path_buf())
        });
        let manifest = match (&source, adobe.manifest_path.as_deref()) {
            (Some(source), Some(relative)) => Some(source.join(relative)),
            _ => None,
        };
        let action = match (source, dest) {
            (Some(source), Some(dest)) => Some(InstallStepAction::AdobePluginLink {
                source,
                dest,
                product: adobe.product.clone(),
                extension_type: adobe.extension_type.clone(),
                manifest,
            }),
            _ => None,
        };
        let description = if action.is_some() {
            format!(
                "Link {} {} plugin into the Adobe debug root",
                adobe.product, adobe.extension_type
            )
        } else {
            format!(
                "Provide --plugin-source and --adobe-debug-root to link the {} {} plugin",
                entry.name, adobe.product
            )
        };
        InstallStep {
            name: "install-adobe-debug-link".into(),
            description,
            action,
        }
    }

    /// Build informational-only steps when no install metadata exists.
    fn build_info_steps() -> Vec<InstallStep> {
        vec![
            InstallStep {
                name: "resolve-adapter".into(),
                description: "Resolve the official adapter package from the Meshy DCC catalog."
                    .into(),
                action: None,
            },
            InstallStep {
                name: "install-runtime".into(),
                description:
                    "Install the cross-platform meshy-dcc and companion runtime binaries.".into(),
                action: None,
            },
            InstallStep {
                name: "install-dcc-adapter".into(),
                description:
                    "Install the DCC-specific adapter into the user's local DCC plugin location."
                        .into(),
                action: None,
            },
            InstallStep {
                name: "verify".into(),
                description:
                    "Follow the emitted next_steps for live instance discovery, readiness, and CLI smoke checks."
                        .into(),
                action: None,
            },
        ]
    }

    fn build_next_steps(
        entry: &CatalogEntry,
        dcc_type: &str,
        dcc_path: Option<&std::path::Path>,
    ) -> Vec<InstallNextStep> {
        let mut steps = Vec::new();

        if let Some(url) = install_instructions_url(entry) {
            steps.push(InstallNextStep {
                name: "read-install-instructions".into(),
                description: format!(
                    "Read the adapter-maintained install.md for {dcc_type}; treat it as the authoritative host-specific setup runbook before executing local install steps."
                ),
                url: Some(url),
                command: None,
                requires_live_instance: false,
            });
        }

        steps.extend([
            InstallNextStep {
                name: "start-dcc-plugin".into(),
                description: format!(
                    "Start or enable the {dcc_type} host plugin. Package install alone does not create a live registry row; the plugin sidecar must start, stay alive, and self-register."
                ),
                url: None,
                command: None,
                requires_live_instance: false,
            },
            InstallNextStep {
                name: "inspect-runtime".into(),
                description:
                    "Inspect CLI, server binary, server version, gateway profile, and default registry diagnostics."
                        .into(),
                url: None,
                command: Some(command(["meshy-dcc", "doctor"])),
                requires_live_instance: false,
            },
            InstallNextStep {
                name: "confirm-local-instance".into(),
                description: format!(
                    "Confirm the {dcc_type} plugin published a direct local MCP/server instance in the shared registry."
                ),
                url: None,
                command: Some(command(["meshy-dcc", "list"])),
                requires_live_instance: false,
            },
            InstallNextStep {
                name: "wait-ready".into(),
                description: format!(
                    "Wait until the {dcc_type} adapter reports readiness before issuing tool calls."
                ),
                url: None,
                command: Some(command_with_dcc(
                    ["meshy-dcc", "wait-ready", "--dcc-type"],
                    dcc_type,
                    [],
                )),
                requires_live_instance: true,
            },
            InstallNextStep {
                name: "discover-tools".into(),
                description: format!(
                    "Search available {dcc_type} tools through the CLI direct-control route."
                ),
                url: None,
                command: Some(command_with_dcc(
                    ["meshy-dcc", "search", "--dcc-type"],
                    dcc_type,
                    ["--query", "diagnostics"],
                )),
                requires_live_instance: true,
            },
            InstallNextStep {
                name: "search-community-skills".into(),
                description: format!(
                    "Find optional community skill packages that target {dcc_type}."
                ),
                url: None,
                command: Some(command_with_dcc(
                    ["meshy-dcc", "marketplace", "search", "--dcc"],
                    dcc_type,
                    ["--query", "skills"],
                )),
                requires_live_instance: false,
            },
            InstallNextStep {
                name: "inspect-community-skill".into(),
                description:
                    "Inspect the selected marketplace skill package before installing it, replacing <package-name> with the chosen package."
                        .into(),
                url: None,
                command: Some(command(["meshy-dcc", "marketplace", "inspect", "<package-name>"])),
                requires_live_instance: false,
            },
            InstallNextStep {
                name: "install-community-skill".into(),
                description:
                    "Install a selected marketplace skill package for this DCC, replacing <package-name> with the chosen package."
                        .into(),
                url: None,
                command: Some(command_with_dcc(
                    ["meshy-dcc", "marketplace", "install", "<package-name>", "--dcc"],
                    dcc_type,
                    [],
                )),
                requires_live_instance: false,
            },
            InstallNextStep {
                name: "reload-skills".into(),
                description: format!(
                    "Reload skills in the live {dcc_type} adapter after installing community skill packages."
                ),
                url: None,
                command: Some(command_with_dcc(
                    ["meshy-dcc", "reload-skills", "--dcc-type"],
                    dcc_type,
                    [],
                )),
                requires_live_instance: true,
            },
        ]);

        steps.push(InstallNextStep {
            name: "resolve-dcc-path".into(),
            description: match dcc_path {
                Some(path) => format!(
                    "Use the supplied DCC path '{}' and verify the host plugin starts there.",
                    path.display()
                ),
                None => format!(
                    "If no {dcc_type} host is found, ask the user for its absolute executable or application path and rerun with --dcc-path."
                ),
            },
            url: None,
            command: None,
            requires_live_instance: false,
        });

        steps
    }
}

fn command<const N: usize>(args: [&str; N]) -> Vec<String> {
    args.into_iter().map(str::to_string).collect()
}

fn command_with_dcc<const P: usize, const S: usize>(
    prefix: [&str; P],
    dcc_type: &str,
    suffix: [&str; S],
) -> Vec<String> {
    prefix
        .into_iter()
        .chain(std::iter::once(dcc_type))
        .chain(suffix)
        .map(str::to_string)
        .collect()
}

fn install_instructions_url(entry: &CatalogEntry) -> Option<String> {
    entry
        .install
        .as_ref()
        .and_then(|install| non_empty(install.instructions_url.as_deref()))
        .map(str::to_string)
        .or_else(|| entry.url.as_deref().and_then(github_install_md_raw_url))
}

fn github_install_md_raw_url(url: &str) -> Option<String> {
    let repo = url
        .trim_end_matches('/')
        .strip_suffix(".git")
        .unwrap_or_else(|| url.trim_end_matches('/'));
    let path = repo.strip_prefix("https://github.com/")?;
    let mut parts = path.split('/');
    let owner = parts.next()?;
    let name = parts.next()?;
    if owner.is_empty() || name.is_empty() || parts.next().is_some() {
        return None;
    }
    Some(format!(
        "https://raw.githubusercontent.com/{owner}/{name}/main/install.md"
    ))
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn select_adapter<'a>(entries: &'a [CatalogEntry], dcc_key: &str) -> Option<&'a CatalogEntry> {
    entries
        .iter()
        .filter(|entry| {
            entry
                .dcc
                .iter()
                .any(|dcc| normalized_dcc_key(dcc) == dcc_key)
        })
        .max_by_key(|entry| adapter_rank(entry, dcc_key))
}

fn adapter_rank(entry: &CatalogEntry, dcc_key: &str) -> (bool, bool, bool, bool) {
    let has_adapter_tag = entry
        .tags
        .iter()
        .any(|tag| tag.eq_ignore_ascii_case("adapter"));
    let has_skill_tag = entry
        .tags
        .iter()
        .any(|tag| tag.eq_ignore_ascii_case("skills"));
    let official_adapter_name = entry
        .name
        .eq_ignore_ascii_case(&format!("meshy-dcc-{dcc_key}"));

    (
        has_adapter_tag,
        entry.install.is_some(),
        official_adapter_name,
        !has_skill_tag,
    )
}

pub(crate) fn normalized_dcc_key(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod host_package_tests {
    use super::host_package_file_name;

    #[test]
    fn host_package_url_allows_https_and_loopback_http_only() {
        let name = |url: &str| host_package_file_name(url);
        assert_eq!(
            name("https://cdn.meshy.ai/a/pkg-1.0.zip").as_deref(),
            Some("pkg-1.0.zip")
        );
        assert_eq!(
            name("http://127.0.0.1:8000/pkg.zip").as_deref(),
            Some("pkg.zip")
        );
        assert_eq!(
            name("http://[::1]:8000/pkg.zip").as_deref(),
            Some("pkg.zip")
        );
        for rejected in [
            "http://example.com/pkg.zip",
            "http://127.0.0.1:80@evil.example/pkg.zip",
            "https://user@cdn.meshy.ai/pkg.zip",
            "https://cdn.meshy.ai/pkg.zip?x=1",
            "https://cdn.meshy.ai/pkg.zip#x",
            "https://cdn.meshy.ai/..%2Fpkg.zip",
            "https://cdn.meshy.ai/",
            "file:///tmp/pkg.zip",
        ] {
            assert_eq!(name(rejected), None, "{rejected}");
        }
    }
}
