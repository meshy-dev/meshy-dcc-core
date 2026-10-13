//! Shared gateway update-manifest loading.

use std::collections::HashMap;

use futures_util::StreamExt;
use reqwest::header::ACCEPT;
use serde::Deserialize;
use thiserror::Error;

/// A single entry in the update manifest (binary_name -> entry).
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ManifestEntry {
    pub(crate) version: String,
    pub(crate) url: Option<String>,
    pub(crate) sha256: Option<String>,
    /// Other places holding the same bytes, tried when `url` cannot be reached.
    #[serde(default)]
    pub(crate) mirrors: Vec<String>,
    pub(crate) release_notes: Option<String>,
}

#[derive(Debug)]
pub(crate) struct VerifiedManifestAsset<'a> {
    pub(crate) url: &'a str,
    pub(crate) sha256: meshy_dcc_updater::Sha256Digest,
    pub(crate) mirrors: &'a [String],
}

#[derive(Debug, Error)]
pub(crate) enum ManifestAssetError {
    #[error("Update manifest entry for {binary_name} is missing a download URL")]
    MissingUrl { binary_name: String },
    #[error(transparent)]
    Integrity(#[from] meshy_dcc_updater::UpdateError),
}

impl ManifestEntry {
    pub(crate) fn require_asset(
        &self,
        binary_name: &str,
    ) -> Result<VerifiedManifestAsset<'_>, ManifestAssetError> {
        let url = self
            .url
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| ManifestAssetError::MissingUrl {
                binary_name: binary_name.to_owned(),
            })?;
        let sha256 = meshy_dcc_updater::Sha256Digest::require(binary_name, self.sha256.as_deref())?;
        Ok(VerifiedManifestAsset {
            url,
            sha256,
            mirrors: &self.mirrors,
        })
    }
}

/// Top-level update manifest fetched from `update_manifest_url`.
pub(crate) type UpdateManifest = HashMap<String, ManifestEntry>;

const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
const MAX_ATTESTATION_BYTES: usize = 1024 * 1024;

#[derive(Debug, Error)]
pub(crate) enum UpdateManifestError {
    #[error("failed to fetch update metadata from '{url}': {source}")]
    Fetch {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("update metadata from '{url}' exceeds the {limit}-byte safety limit")]
    TooLarge { url: String, limit: usize },
    #[error("update metadata from '{url}' returned unexpected HTTP status {status}")]
    UnexpectedStatus {
        url: String,
        status: reqwest::StatusCode,
    },
    #[error("update manifest from '{url}' is not valid UTF-8: {source}")]
    InvalidUtf8 {
        url: String,
        #[source]
        source: std::string::FromUtf8Error,
    },
    #[error("update manifest JSON is invalid: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("official update manifest attestation verification failed: {0}")]
    Attestation(#[from] meshy_dcc_attestation::AttestationError),
    #[error("Meshy publishes no meshy-dcc release for this platform")]
    NoRelease,
    #[error("could not find the latest meshy-dcc release on the Meshy CDN or GitHub")]
    NoLatestRelease,
}

/// Fetch and parse the configured update manifest.
///
/// The official channel ([`meshy_release::LATEST_URL`]) first finds the latest
/// version, then fetches that release's manifest from cdn.meshy.ai,
/// cdn.meshy.cn or the GitHub Release: a source that cannot be reached moves
/// on to the next, a manifest that fails verification stops.
///
/// [`meshy_release::LATEST_URL`]: meshy_dcc_updater::meshy_release::LATEST_URL
pub(crate) async fn fetch_update_manifest(
    url: &str,
) -> Result<UpdateManifest, UpdateManifestError> {
    use meshy_dcc_updater::meshy_release;

    // Release assets redirect to GitHub's download host. Keep this policy
    // separate from the gateway's redirect-rejecting DCC backend client.
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|source| UpdateManifestError::Fetch {
            url: url.to_owned(),
            source,
        })?;
    if url != meshy_release::LATEST_URL {
        return fetch_manifest_at(&client, url).await;
    }
    let platform = meshy_release::current_platform().ok_or(UpdateManifestError::NoRelease)?;
    let version = latest_version(&client).await?;
    first_reachable_manifest(
        &client,
        meshy_release::update_manifest_urls(&version, platform),
    )
    .await
}

async fn first_reachable_manifest(
    client: &reqwest::Client,
    urls: impl IntoIterator<Item = String>,
) -> Result<UpdateManifest, UpdateManifestError> {
    let mut unreachable = UpdateManifestError::NoLatestRelease;
    for url in urls {
        match fetch_manifest_at(client, &url).await {
            Err(
                error @ (UpdateManifestError::Fetch { .. }
                | UpdateManifestError::UnexpectedStatus { .. }),
            ) => {
                tracing::warn!(%error, "update manifest source unreachable; trying the next");
                unreachable = error;
            }
            result => return result,
        }
    }
    Err(unreachable)
}

/// `latest.txt` on cdn.meshy.ai, then cdn.meshy.cn, then the GitHub latest
/// release, which redirects to `releases/tag/v<version>`.
async fn latest_version(client: &reqwest::Client) -> Result<String, UpdateManifestError> {
    use meshy_dcc_updater::meshy_release;

    for url in meshy_release::latest_version_urls() {
        if let Ok(bytes) = fetch_bounded(client, &url, 64).await
            && let Ok(text) = String::from_utf8(bytes)
            && meshy_release::is_stable_version(text.trim())
        {
            return Ok(text.trim().to_owned());
        }
    }
    let url = format!("{}/latest", meshy_release::GITHUB_RELEASES);
    let response = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|source| UpdateManifestError::Fetch { url, source })?;
    response
        .url()
        .path()
        .rsplit_once("/releases/tag/v")
        .map(|(_, version)| version)
        .filter(|version| meshy_release::is_stable_version(version))
        .map(str::to_owned)
        .ok_or(UpdateManifestError::NoLatestRelease)
}

/// Fetch one manifest, verifying the attestation an official one must carry.
async fn fetch_manifest_at(
    client: &reqwest::Client,
    url: &str,
) -> Result<UpdateManifest, UpdateManifestError> {
    let manifest_bytes = fetch_bounded(client, url, MAX_MANIFEST_BYTES).await?;
    if let Some(attestation_url) = official_attestation_url(url) {
        let bundle_bytes = fetch_bounded(client, &attestation_url, MAX_ATTESTATION_BYTES).await?;
        let bundle =
            String::from_utf8(bundle_bytes).map_err(|source| UpdateManifestError::InvalidUtf8 {
                url: attestation_url,
                source,
            })?;
        meshy_dcc_attestation::verify_attested_bytes(
            &manifest_bytes,
            &bundle,
            &meshy_dcc_attestation::GitHubAttestationPolicy::meshy_release(),
        )?;
    }
    let manifest =
        String::from_utf8(manifest_bytes).map_err(|source| UpdateManifestError::InvalidUtf8 {
            url: url.to_owned(),
            source,
        })?;
    Ok(serde_json::from_str(&manifest)?)
}

async fn fetch_bounded(
    client: &reqwest::Client,
    url: &str,
    limit: usize,
) -> Result<Vec<u8>, UpdateManifestError> {
    let response = client
        .get(url)
        .header(ACCEPT, "application/json")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|source| UpdateManifestError::Fetch {
            url: url.to_owned(),
            source,
        })?
        .error_for_status()
        .map_err(|source| UpdateManifestError::Fetch {
            url: url.to_owned(),
            source,
        })?;
    if !response.status().is_success() {
        return Err(UpdateManifestError::UnexpectedStatus {
            url: url.to_owned(),
            status: response.status(),
        });
    }
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(UpdateManifestError::TooLarge {
            url: url.to_owned(),
            limit,
        });
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|source| UpdateManifestError::Fetch {
            url: url.to_owned(),
            source,
        })?;
        if bytes.len().saturating_add(chunk.len()) > limit {
            return Err(UpdateManifestError::TooLarge {
                url: url.to_owned(),
                limit,
            });
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn official_attestation_url(url: &str) -> Option<String> {
    meshy_dcc_updater::meshy_release::manifest_attestation_url(url)
}

#[cfg(test)]
mod tests {
    use super::official_attestation_url;

    #[tokio::test]
    async fn update_downloads_follow_redirects_and_reject_unresolved_redirects() {
        use axum::{Router, http::StatusCode, response::Redirect, routing::get};
        let app = Router::new()
            .route("/latest", get(|| async { Redirect::temporary("/asset") }))
            .route(
                "/asset",
                get(|| async { r#"{"meshy-dcc":{"version":"0.20.24"}}"# }),
            )
            .route("/loop", get(|| async { Redirect::temporary("/loop") }))
            .route("/missing-location", get(|| async { StatusCode::FOUND }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let manifest = super::fetch_update_manifest(&format!("{base}/latest")).await;
        let loop_result = super::fetch_update_manifest(&format!("{base}/loop")).await;
        let missing_location =
            super::fetch_update_manifest(&format!("{base}/missing-location")).await;
        server.abort();
        assert_eq!(manifest.unwrap()["meshy-dcc"].version, "0.20.24");
        assert!(matches!(
            loop_result,
            Err(super::UpdateManifestError::Fetch { .. })
        ));
        assert!(matches!(
            missing_location,
            Err(super::UpdateManifestError::UnexpectedStatus {
                status: StatusCode::FOUND,
                ..
            })
        ));
    }

    #[test]
    fn official_manifest_requires_meshy_attestation() {
        for url in meshy_dcc_updater::meshy_release::update_manifest_urls("0.2.0", "windows-x86_64")
        {
            assert!(official_attestation_url(&url).is_some(), "{url}");
        }
    }

    #[tokio::test]
    async fn unreachable_sources_move_on_and_invalid_manifests_stop() {
        use axum::{Router, http::StatusCode, routing::get};
        let app = Router::new()
            .route("/missing", get(|| async { StatusCode::FORBIDDEN }))
            .route(
                "/good",
                get(|| async { r#"{"meshy-dcc":{"version":"0.2.0"}}"# }),
            )
            .route("/garbage", get(|| async { "not json" }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = reqwest::Client::new();
        let urls = |paths: &[&str]| {
            paths
                .iter()
                .map(|path| format!("{base}{path}"))
                .collect::<Vec<_>>()
        };

        let found = super::first_reachable_manifest(
            &client,
            [
                vec!["http://127.0.0.1:9/down".to_owned()],
                urls(&["/missing", "/good"]),
            ]
            .concat(),
        )
        .await;
        let stopped = super::first_reachable_manifest(&client, urls(&["/garbage", "/good"])).await;
        let none = super::first_reachable_manifest(&client, urls(&["/missing"])).await;
        server.abort();

        assert_eq!(found.unwrap()["meshy-dcc"].version, "0.2.0");
        assert!(matches!(stopped, Err(super::UpdateManifestError::Parse(_))));
        assert!(matches!(
            none,
            Err(super::UpdateManifestError::Fetch { .. })
        ));
    }
}
