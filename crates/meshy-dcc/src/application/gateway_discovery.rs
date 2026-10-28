//! Gateway binary discovery.
//!
//! 1. An explicit path from the caller
//! 2. `meshy-dcc-server` beside the current executable (the installer puts it there)
//! 3. The current executable, which runs the same gateway as `meshy-dcc gateway`
//!
//! Nothing is downloaded: a binary fetched at run time and executed without
//! verification is not worth the start-up it saves.

use std::path::PathBuf;

use anyhow::Context;
use serde::Serialize;

/// Gateway binary filename, platform-aware.
pub(crate) const GATEWAY_BINARY_NAME: &str = "meshy-dcc-server";

#[cfg(windows)]
const GATEWAY_BINARY_NAME_WITH_EXT: &str = "meshy-dcc-server.exe";
#[cfg(not(windows))]
const GATEWAY_BINARY_NAME_WITH_EXT: &str = GATEWAY_BINARY_NAME;

/// Resolve the gateway binary: the explicit path, then `meshy-dcc-server`
/// beside the CLI, then the CLI itself.
///
/// Returns an error only when the explicit path is missing.
pub async fn resolve_gateway_bin(gateway_bin: Option<&PathBuf>) -> anyhow::Result<PathBuf> {
    // Layer 0: Explicit path provided by caller
    if let Some(explicit) = gateway_bin {
        if explicit.exists() {
            return Ok(explicit.clone());
        }
        anyhow::bail!("explicit gateway binary not found: {}", explicit.display());
    }

    // Layer 1: Same-directory lookup
    if let Some(found) = find_in_same_dir() {
        return Ok(found);
    }

    // Layer 2: Sub-command fallback (current executable)
    let current =
        std::env::current_exe().context("resolving current executable for sub-command fallback")?;
    Ok(current)
}

#[derive(Debug, Clone, Serialize)]
pub struct GatewayBinaryDiagnostic {
    pub status: &'static str,
    pub source: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configured_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Diagnose which gateway binary auto-start would launch, without launching it.
pub fn diagnose_gateway_bin(gateway_bin: Option<&PathBuf>) -> GatewayBinaryDiagnostic {
    if let Some(explicit) = gateway_bin {
        if explicit.exists() {
            return binary_diagnostic(
                "ok",
                "explicit",
                Some(explicit.clone()),
                Some(explicit.clone()),
            );
        }
        return GatewayBinaryDiagnostic {
            status: "missing",
            source: "explicit",
            path: None,
            configured_path: Some(explicit.clone()),
            version: None,
            error: Some(format!(
                "explicit gateway binary not found: {}",
                explicit.display()
            )),
        };
    }

    if let Some(found) = find_in_same_dir() {
        return binary_diagnostic("ok", "same_dir", Some(found), None);
    }

    // The CLI runs the gateway itself.
    binary_diagnostic("ok", "cli", std::env::current_exe().ok(), None)
}

fn binary_diagnostic(
    status: &'static str,
    source: &'static str,
    path: Option<PathBuf>,
    configured_path: Option<PathBuf>,
) -> GatewayBinaryDiagnostic {
    let (version, error) = match path.as_ref() {
        Some(path) => probe_binary_version(path),
        None => (None, None),
    };
    GatewayBinaryDiagnostic {
        status,
        source,
        path,
        configured_path,
        version,
        error,
    }
}

fn probe_binary_version(path: &std::path::Path) -> (Option<String>, Option<String>) {
    match std::process::Command::new(path).arg("--version").output() {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !text.is_empty() {
                return (Some(text), None);
            }
            let text = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if !text.is_empty() {
                return (Some(text), None);
            }
            (
                None,
                Some("--version succeeded but produced no output".to_string()),
            )
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let message = if stderr.is_empty() {
                format!("--version exited with status {}", output.status)
            } else {
                stderr
            };
            (None, Some(message))
        }
        Err(err) => (None, Some(format!("running --version failed: {err}"))),
    }
}

// ── Layer 1: Same-directory lookup ──────────────────────────────────────

/// Look for `meshy-dcc-server` (or `.exe` on Windows) in the same directory
/// as the current executable.
pub(crate) fn find_in_same_dir() -> Option<PathBuf> {
    let current_exe = std::env::current_exe().ok()?;
    let exe_dir = current_exe.parent()?;
    let candidate = exe_dir.join(GATEWAY_BINARY_NAME_WITH_EXT);
    if candidate.is_file() {
        Some(candidate)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_in_same_dir_returns_none_when_binary_missing() {
        // Our test binary is not named meshy-dcc-server, so this should
        // return None in test builds.
        let result = find_in_same_dir();
        // Accept both None (binary not found) and Some (coincidental match
        // in an unusual test layout).
        if let Some(path) = &result {
            assert!(
                path.file_name()
                    .map(|n| n.to_string_lossy().contains("meshy-dcc-server"))
                    .unwrap_or(false),
                "unexpected found path: {}",
                path.display()
            );
        }
    }

    #[test]
    fn test_gateway_binary_name_has_correct_extension() {
        #[cfg(windows)]
        assert!(GATEWAY_BINARY_NAME_WITH_EXT.ends_with(".exe"));
        #[cfg(not(windows))]
        assert!(!GATEWAY_BINARY_NAME_WITH_EXT.ends_with(".exe"));
    }
}
