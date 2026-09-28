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
    pub(crate) release_notes: Option<String>,
}

#[derive(Debug)]
pub(crate) struct VerifiedManifestAsset<'a> {
    pub(crate) url: &'a str,
    pub(crate) sha256: meshy_dcc_updater::Sha256Digest,
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
        Ok(VerifiedManifestAsset { url, sha256 })
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
}

/// Fetch and parse the configured update manifest.
pub(crate) async fn fetch_update_manifest(
    url: &str,
) -> Result<UpdateManifest, UpdateManifestError> {
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
    let manifest_bytes = fetch_bounded(&client, url, MAX_MANIFEST_BYTES).await?;
    if let Some(attestation_url) = official_attestation_url(url) {
        let bundle_bytes = fetch_bounded(&client, &attestation_url, MAX_ATTESTATION_BYTES).await?;
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
        let url = meshy_dcc_updater::meshy_release::update_manifest_url("windows-x86_64");
        assert!(official_attestation_url(&url).is_some());
        assert_eq!(
            official_attestation_url(
                "https://github.com/meshy-dev/meshy-dcc-core/releases/latest/download/meshy-dcc-update-manifest-windows-x86_64.json"
            ),
            None
        );
    }
}
