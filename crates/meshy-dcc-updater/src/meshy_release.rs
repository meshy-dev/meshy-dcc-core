//! Meshy release channel for the `meshy-dcc` binaries.
//!
//! The gateway self-update and the install catalog resolve their files here,
//! so the CDN layout lives in one place. A release lives in a version
//! directory that is never overwritten (`v0-2-0/` for 0.2.0); the uncached
//! `latest.txt` names the newest release. `cdn.meshy.cn` serves the same
//! bucket for mainland China, and the GitHub Release holds the same files
//! under their release names. Until the Meshy release pipeline publishes
//! these files, requests fail closed.

/// Meshy DCC Core product version; crates, wheels and releases share it.
pub const PRODUCT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Base URL of Meshy DCC Core release files.
pub const BASE_URL: &str = "https://cdn.meshy.ai/meshy-dcc-core";
/// The same bucket on the host that resolves in mainland China.
pub const BASE_URL_CN: &str = "https://cdn.meshy.cn/meshy-dcc-core";
/// GitHub Releases of meshy-dcc-core, the last source tried.
pub const GITHUB_RELEASES: &str = "https://github.com/meshy-dev/meshy-dcc-core/releases";
/// Names the official update channel. The gateway reads the version from
/// `latest.txt` (cdn.meshy.ai, then cdn.meshy.cn, then the GitHub latest
/// release) and fetches that release's signed manifest.
pub const LATEST_URL: &str = "https://cdn.meshy.ai/meshy-dcc-core/latest.txt";

const MANIFEST_PREFIX: &str = "meshy-dcc-update-manifest-";
// Apple Silicon only on macOS; Linux is not published yet.
const PLATFORMS: &[&str] = &["linux-x86_64", "windows-x86_64", "macos-arm64"];

/// Signed install catalog feed; `meshy-dcc` falls back to its bundled copy.
pub fn install_catalog_url() -> String {
    format!("{BASE_URL}/catalog/install-catalog.json")
}

/// Release platform of this build (`macos-arm64`, `windows-x86_64`,
/// `linux-x86_64`), or `None` where Meshy publishes nothing.
pub fn current_platform() -> Option<&'static str> {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("macos-arm64")
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        Some("windows-x86_64")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("linux-x86_64")
    } else {
        None
    }
}

/// `latest.txt` on each CDN host, in the order to try them.
pub fn latest_version_urls() -> [String; 2] {
    [BASE_URL, BASE_URL_CN].map(|base| format!("{base}/latest.txt"))
}

/// A stable release version as `latest.txt` and release tags name it (`0.2.0`).
pub fn is_stable_version(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

/// CDN keys allow lowercase letters, digits and hyphens: `0.2.0` -> `v0-2-0`.
fn cdn_version_dir(version: &str) -> String {
    let mut dir = String::from("v");
    for c in version.to_ascii_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            dir.push(c);
        } else if !dir.ends_with('-') {
            dir.push('-');
        }
    }
    dir
}

/// `windows-x86_64` -> `windows-x64` on the CDN.
fn cdn_platform(platform: &str) -> String {
    platform.replace("x86_64", "x64")
}

/// One release's manifest for `platform`: cdn.meshy.ai, cdn.meshy.cn, then
/// the GitHub Release.
pub fn update_manifest_urls(version: &str, platform: &str) -> [String; 3] {
    let dir = cdn_version_dir(version);
    let label = cdn_platform(platform);
    [
        format!("{BASE_URL}/{dir}/{MANIFEST_PREFIX}{label}.json"),
        format!("{BASE_URL_CN}/{dir}/{MANIFEST_PREFIX}{label}.json"),
        format!("{GITHUB_RELEASES}/download/v{version}/{MANIFEST_PREFIX}{platform}.json"),
    ]
}

/// Detached Sigstore bundle that must accompany an official Meshy manifest:
/// `-sigstore.json` beside a CDN manifest (one extension), `.sigstore.json`
/// beside a GitHub Release manifest.
///
/// Returns `None` for any other URL; an operator-configured manifest stays
/// trusted as configured, matching upstream behavior.
pub fn manifest_attestation_url(url: &str) -> Option<String> {
    let release_file = |rest: &str, dir_ok: fn(&str) -> bool| -> Option<(String, String)> {
        let (dir, file) = rest.split_once('/')?;
        let platform = file.strip_prefix(MANIFEST_PREFIX)?.strip_suffix(".json")?;
        dir_ok(dir.strip_prefix('v')?).then(|| (dir.to_owned(), platform.to_owned()))
    };
    for base in [BASE_URL, BASE_URL_CN] {
        if let Some(rest) = url
            .strip_prefix(base)
            .and_then(|rest| rest.strip_prefix('/'))
        {
            let (dir, label) = release_file(rest, |v| {
                !v.is_empty()
                    && v.bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            })?;
            PLATFORMS
                .iter()
                .any(|p| cdn_platform(p) == label)
                .then_some(())?;
            return Some(format!(
                "{base}/{dir}/{MANIFEST_PREFIX}{label}-sigstore.json"
            ));
        }
    }
    let rest = url
        .strip_prefix(GITHUB_RELEASES)?
        .strip_prefix("/download/")?;
    let (tag, platform) = release_file(rest, |v| {
        !v.is_empty()
            && v.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b".+-".contains(&b))
    })?;
    PLATFORMS.contains(&platform.as_str()).then_some(())?;
    Some(format!(
        "{GITHUB_RELEASES}/download/{tag}/{MANIFEST_PREFIX}{platform}.sigstore.json"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_release_manifest_is_tried_on_both_cdn_hosts_then_github() {
        assert_eq!(
            update_manifest_urls("0.2.0", "windows-x86_64"),
            [
                "https://cdn.meshy.ai/meshy-dcc-core/v0-2-0/meshy-dcc-update-manifest-windows-x64.json",
                "https://cdn.meshy.cn/meshy-dcc-core/v0-2-0/meshy-dcc-update-manifest-windows-x64.json",
                "https://github.com/meshy-dev/meshy-dcc-core/releases/download/v0.2.0/meshy-dcc-update-manifest-windows-x86_64.json",
            ]
        );
        assert_eq!(latest_version_urls()[0], LATEST_URL);
    }

    #[test]
    fn official_manifests_require_a_sibling_attestation() {
        let [ai, cn, github] = update_manifest_urls("0.2.0", "macos-arm64");
        assert_eq!(
            manifest_attestation_url(&ai).as_deref(),
            Some(
                "https://cdn.meshy.ai/meshy-dcc-core/v0-2-0/meshy-dcc-update-manifest-macos-arm64-sigstore.json"
            )
        );
        assert_eq!(
            manifest_attestation_url(&cn).as_deref(),
            Some(
                "https://cdn.meshy.cn/meshy-dcc-core/v0-2-0/meshy-dcc-update-manifest-macos-arm64-sigstore.json"
            )
        );
        assert_eq!(
            manifest_attestation_url(&github).as_deref(),
            Some(
                "https://github.com/meshy-dev/meshy-dcc-core/releases/download/v0.2.0/meshy-dcc-update-manifest-macos-arm64.sigstore.json"
            )
        );
    }

    #[test]
    fn other_urls_are_not_treated_as_official() {
        for url in [
            "https://studio.example/update.json",
            LATEST_URL,
            "http://cdn.meshy.ai/meshy-dcc-core/v0-2-0/meshy-dcc-update-manifest-macos-arm64.json",
            "https://cdn.meshy.ai.attacker.example/meshy-dcc-core/v0-2-0/meshy-dcc-update-manifest-macos-arm64.json",
            "https://cdn.meshy.ai/meshy-dcc-core/v0-2-0/unexpected.json",
            "https://cdn.meshy.ai/meshy-dcc-core/v0-2-0/nested/meshy-dcc-update-manifest-macos-arm64.json",
            "https://cdn.meshy.ai/meshy-dcc-core/v/meshy-dcc-update-manifest-macos-arm64.json",
            "https://cdn.meshy.ai/meshy-dcc-core/v0.2.0/meshy-dcc-update-manifest-macos-arm64.json",
            "https://cdn.meshy.ai/meshy-dcc-core/v0-2-0/meshy-dcc-update-manifest-windows-x86_64.json",
            "https://cdn.meshy.ai/meshy-dcc-core/v0-2-0/meshy-dcc-update-manifest-macos-arm64.json?x=1",
            "https://github.com/meshy-dev/meshy-dcc-core/releases/latest/download/meshy-dcc-update-manifest-macos-arm64.json",
            "https://github.com/meshy-dev/meshy-dcc-core/releases/download/v0.2.0/meshy-dcc-update-manifest-windows-x64.json",
            "https://github.com/other/meshy-dcc-core/releases/download/v0.2.0/meshy-dcc-update-manifest-macos-arm64.json",
        ] {
            assert_eq!(manifest_attestation_url(url), None, "{url}");
        }
    }

    #[test]
    fn stable_versions_only() {
        assert!(is_stable_version("0.2.0"));
        assert!(is_stable_version("10.20.30"));
        for v in [
            "",
            "0.2",
            "0.2.0-beta.1",
            "v0.2.0",
            "0..0",
            "0.2.0\n",
            "<html>",
        ] {
            assert!(!is_stable_version(v), "{v:?}");
        }
    }
}
