//! Meshy release channel for the `meshy-dcc` binaries.
//!
//! The gateway self-update and the CLI's gateway download both resolve their
//! files here, so the CDN layout lives in one place. Until the Meshy release
//! pipeline publishes these files, requests fail closed.

/// Meshy DCC Core product version: `--version`, release tags, the update
/// channel and download paths.
///
/// Crate versions stay at the fork base on purpose. Gateway election and
/// adapter minimum-core checks compare that wire version with hosts that embed
/// the same runtime, so it must not move to the product numbering.
pub const PRODUCT_VERSION: &str = "0.1.0";

/// Base URL of Meshy DCC Core release files.
pub const BASE_URL: &str = "https://cdn.meshy.ai/meshy-dcc-core";

const MANIFEST_PREFIX: &str = "meshy-dcc-update-manifest-";
const PLATFORMS: &[&str] = &["linux-x86_64", "windows-x86_64", "macos-universal2"];

/// Latest update manifest for a release platform such as `macos-universal2`.
/// Signed install catalog feed; `meshy-dcc` falls back to its bundled copy.
pub fn install_catalog_url() -> String {
    format!("{BASE_URL}/catalog/install-catalog.json")
}

pub fn update_manifest_url(platform: &str) -> String {
    format!("{BASE_URL}/latest/{MANIFEST_PREFIX}{platform}.json")
}

/// Detached Sigstore bundle that must accompany an official Meshy manifest.
///
/// Returns `None` for any other URL; an operator-configured manifest stays
/// trusted as configured, matching upstream behavior.
pub fn manifest_attestation_url(url: &str) -> Option<String> {
    let rest = url.strip_prefix(BASE_URL)?.strip_prefix('/')?;
    let (dir, file) = rest.split_once('/')?;
    let versioned = dir.strip_prefix('v').is_some_and(|v| {
        !v.is_empty()
            && v.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    });
    if dir != "latest" && !versioned {
        return None;
    }
    let platform = file.strip_prefix(MANIFEST_PREFIX)?.strip_suffix(".json")?;
    if !PLATFORMS.contains(&platform) {
        return None;
    }
    Some(format!(
        "{BASE_URL}/{dir}/{MANIFEST_PREFIX}{platform}.sigstore.json"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_manifests_require_a_sibling_attestation() {
        let latest = update_manifest_url("macos-universal2");
        assert_eq!(
            manifest_attestation_url(&latest).as_deref(),
            Some(
                "https://cdn.meshy.ai/meshy-dcc-core/latest/meshy-dcc-update-manifest-macos-universal2.sigstore.json"
            )
        );
        assert!(
            manifest_attestation_url(
                "https://cdn.meshy.ai/meshy-dcc-core/v0.1.0/meshy-dcc-update-manifest-linux-x86_64.json"
            )
            .is_some()
        );
    }

    #[test]
    fn other_urls_are_not_treated_as_official() {
        for url in [
            "https://studio.example/update.json",
            "http://cdn.meshy.ai/meshy-dcc-core/latest/meshy-dcc-update-manifest-macos-universal2.json",
            "https://cdn.meshy.ai.attacker.example/meshy-dcc-core/latest/meshy-dcc-update-manifest-macos-universal2.json",
            "https://cdn.meshy.ai/meshy-dcc-core/latest/unexpected.json",
            "https://cdn.meshy.ai/meshy-dcc-core/latest/nested/meshy-dcc-update-manifest-macos-universal2.json",
            "https://cdn.meshy.ai/meshy-dcc-core/v/meshy-dcc-update-manifest-macos-universal2.json",
            "https://cdn.meshy.ai/meshy-dcc-core/latest/meshy-dcc-update-manifest-macos-universal2.json?x=1",
        ] {
            assert_eq!(manifest_attestation_url(url), None, "{url}");
        }
    }
}
