use super::*;

fn payload() -> Payload {
    serde_json::from_value(serde_json::json!({
        "schema_version": 1,
        "source_revision": "a".repeat(40),
        "issued_at": 1000,
        "expires_at": 2000,
        "entries": [{"name":"meshy-dcc-godot", "description":"Godot adapter", "dcc":["godot"]}]
    }))
    .unwrap()
}

#[test]
fn payload_rejects_expired_future_and_unbounded_lifetimes() {
    assert!(validate_payload(&payload(), 1500, true).is_ok());
    assert!(validate_payload(&payload(), 2000, true).is_err());
    assert!(validate_payload(&payload(), 2000, false).is_ok());
    assert!(validate_payload(&payload(), 699, false).is_err());
    for expires in [999, 1000, 1000 + MAX_LIFETIME + 1] {
        let mut candidate = payload();
        candidate.expires_at = expires;
        assert!(validate_payload(&candidate, 1500, true).is_err());
    }
}

#[test]
fn payload_rejects_unknown_schema_duplicate_entries_and_mutable_revision() {
    let mut candidate = payload();
    candidate.schema_version = 2;
    assert!(validate_payload(&candidate, 1500, true).is_err());
    candidate = payload();
    candidate.entries.push(candidate.entries[0].clone());
    assert!(validate_payload(&candidate, 1500, true).is_err());
    candidate = payload();
    candidate.source_revision = "main".into();
    assert!(validate_payload(&candidate, 1500, true).is_err());
    candidate = payload();
    candidate.entries.clear();
    assert!(validate_payload(&candidate, 1500, true).is_err());
}

#[test]
fn payload_rejects_unbound_artifact_metadata() {
    let mut candidate = payload();
    candidate.entries = meshy_dcc_catalog::load_from_str(super::super::FIXTURE_CATALOG).unwrap();
    assert!(validate_payload(&candidate, 1500, true).is_ok());
    let entry = candidate
        .entries
        .iter_mut()
        .find(|entry| entry.name == "meshy-dcc-godot")
        .unwrap();
    entry.version = Some("9.9.9".into());
    assert!(validate_payload(&candidate, 1500, true).is_err());
}

fn snapshot(issued_at: u64, digest: &str) -> ResolvedCatalog {
    ResolvedCatalog {
        entries: payload().entries,
        provenance: CatalogProvenance {
            issued_at: Some(issued_at),
            sha256: Some(digest.into()),
            ..CatalogProvenance::local(CatalogSource::Remote)
        },
    }
}

#[test]
fn rejects_replay_and_same_time_equivocation() {
    let accepted = snapshot(1500, "a");
    assert!(reject_rollback(&accepted, &snapshot(1499, "a")).is_err());
    assert!(reject_rollback(&accepted, &snapshot(1500, "b")).is_err());
    assert!(reject_rollback(&accepted, &snapshot(1500, "a")).is_ok());
    assert!(reject_rollback(&accepted, &snapshot(1501, "b")).is_ok());
}

#[test]
fn rejects_unsigned_truncated_and_oversized_envelopes() {
    let unsigned = serde_json::json!({"catalog":"{}", "attestation":{}});
    assert!(verify(&serde_json::to_vec(&unsigned).unwrap(), 1500, true).is_err());
    assert!(verify(b"truncated", 1500, true).is_err());
    assert!(verify(&vec![b' '; MAX_BYTES + 1], 1500, true).is_err());
}

#[test]
fn cache_replacement_is_atomic_and_invalid_cache_cannot_be_promoted() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("catalog.json");
    assert!(read_cache(&path).unwrap().is_none());
    persist(&path, b"old").unwrap();
    persist(&path, b"new").unwrap();
    assert_eq!(read_cache(&path).unwrap().unwrap(), b"new");
    let now = now_seconds().unwrap();
    let mut replacement = snapshot(now, "x");
    replacement.provenance.expires_at = Some(now + 60);
    let expected = verify(b"new", now, false).unwrap_err().to_string();
    let error = persist_verified(&path, b"replacement", &replacement, now).unwrap_err();
    assert_eq!(error.to_string(), expected);
    assert_eq!(std::fs::read(&path).unwrap(), b"new");
}

#[test]
fn cached_resolution_is_read_only_and_rejects_untrusted_cache_instead_of_falling_back() {
    let root = tempfile::tempdir().unwrap();
    let cache = root.path().join("missing-parent/catalog.json");
    let _env = meshy_dcc_test_utils::EnvVarGuard::set("MESHY_DCC_INSTALL_CACHE", cache.to_str());
    assert_eq!(
        resolve_cached().unwrap().provenance.source,
        CatalogSource::Bundled
    );
    assert!(!cache.parent().unwrap().exists());

    std::fs::create_dir(cache.parent().unwrap()).unwrap();
    let unsigned = br#"{"catalog":"{}","attestation":{}}"#;
    std::fs::write(&cache, unsigned).unwrap();
    assert!(resolve_cached().is_err());
    assert_eq!(std::fs::read(&cache).unwrap(), unsigned);
    assert!(!cache.with_extension("lock").exists());
}

#[test]
fn authenticated_cached_metadata_preserves_revocations_and_refuses_expiry() {
    // This boundary takes authenticated snapshots; signature rejection is covered above.
    let mut accepted = snapshot(1000, "trusted-digest");
    accepted.provenance.expires_at = Some(2000);
    accepted.entries[0].policy = Some(meshy_dcc_catalog::CatalogPolicy {
        installation: "not_available".into(),
        reason: Some("withdrawn release".into()),
    });
    let observed = cached_or_bundled(Some(accepted.clone()), 1500).unwrap();
    assert_eq!(observed.provenance.source, CatalogSource::Cache);
    assert!(!observed.provenance.latest_checked);
    assert_eq!(observed.entries[0].policy, accepted.entries[0].policy);
    assert!(cached_or_bundled(Some(accepted), 2000).is_err());
}
