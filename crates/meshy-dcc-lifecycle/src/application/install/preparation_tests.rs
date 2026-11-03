use super::*;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};

#[cfg(unix)]
#[tokio::test]
async fn preparation_needs_no_project_and_later_configuration_uses_the_verified_local_package() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("data");
    let _env = meshy_dcc_test_utils::EnvVarGuard::set("MESHY_DCC_DATA_DIR", data.to_str());
    let project = temp.path().join("native project with spaces");
    std::fs::create_dir(&project).unwrap();
    let host_a = temp.path().join("host-a");
    std::fs::write(&host_a, "fixture").unwrap();
    let host_b = temp.path().join("host-b");
    std::fs::write(&host_b, "fixture").unwrap();
    let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    archive
        .start_file(
            "bin/installer",
            zip::write::SimpleFileOptions::default().unix_permissions(0o755),
        )
        .unwrap();
    archive.write_all(b"#!/bin/sh\nif [ \"$1\" = \"--global\" ]; then printf prepared > \"$2/global\"; else printf configured > \"$1/project-configured\"; printf '%s' \"$2\" > \"$1/host-chosen\"; fi\n").unwrap();
    let bytes = archive.finish().unwrap().into_inner();
    let sha = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut request = [0; 4096];
        let _ = socket.read(&mut request);
        write!(
            socket,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            bytes.len()
        )
        .unwrap();
        socket.write_all(&bytes).unwrap();
    });
    let catalog = temp.path().join("catalog.json");
    let entry = json!({"name":"fixture-adapter","description":"fixture","dcc":["fixture"],"tags":["adapter"],"version":"1.0.0",
        "policy":{"installation":"available"},"install":{"type":"host-package","url":format!("http://127.0.0.1:{port}/fixture.zip"),"sha256":sha,
        "host":{"extract":true,"dcc_paths":[temp.path().join("host-*")],"software_homepage":"https://example.invalid/software",
            "steps":[["{package_dir}/bin/installer","--global",temp.path()]],"project_steps":[["{package_dir}/bin/installer","{project}","{dcc_path}"]]}}});
    std::fs::write(&catalog, json!({"entries":[entry]}).to_string()).unwrap();
    let observed = observe_software(Some(&catalog)).unwrap();
    assert_eq!(observed["software"][0]["detection"], "multiple");
    assert_eq!(observed["software"][0]["adapter"]["prepared"], false);
    assert!(observed["software"][0].get("project").is_none());
    let result = prepare_adapter("fixture", None, Some(&catalog), true)
        .await
        .unwrap();
    assert_eq!(result["status"], "prepared");
    server.join().unwrap();
    assert!(temp.path().join("global").is_file());
    assert!(!project.join("project-configured").exists());
    assert_eq!(
        observe_software(Some(&catalog)).unwrap()["software"][0]["adapter"]["prepared"],
        true
    );
    // The only HTTP server has exited. Project setup must not download again.
    assert!(
        configure_prepared_project("fixture", &project, None)
            .unwrap_err()
            .to_string()
            .contains("ambiguous")
    );
    assert!(!project.join("project-configured").exists());
    assert_eq!(
        configure_prepared_project("fixture", &project, Some(host_b.clone())).unwrap()["status"],
        "configured"
    );
    assert_eq!(
        std::fs::read_to_string(project.join("project-configured")).unwrap(),
        "configured"
    );
    assert_eq!(
        std::fs::read_to_string(project.join("host-chosen")).unwrap(),
        host_b.to_string_lossy()
    );
    let package = PathBuf::from(result["package_dir"].as_str().unwrap());
    std::fs::remove_file(package.join("bin/installer")).unwrap();
    assert_eq!(
        observe_software(Some(&catalog)).unwrap()["software"][0]["adapter"]["prepared"],
        false
    );
    assert!(configure_prepared_project("fixture", &project, None).is_err());
}

#[test]
fn missing_scan_patterns_are_unknown_not_software_absence() {
    let temp = tempfile::tempdir().unwrap();
    let _env = meshy_dcc_test_utils::EnvVarGuard::set("MESHY_DCC_DATA_DIR", temp.path().to_str());
    let catalog = temp.path().join("catalog.json");
    std::fs::write(&catalog,json!({"entries":[{"name":"unknown-fixture","description":"fixture","dcc":["fixture"],"tags":["adapter"],"policy":{"installation":"not_available"}}]}).to_string()).unwrap();
    let value = observe_software(Some(&catalog)).unwrap();
    assert_eq!(value["software"][0]["detection"], "unknown");
    assert_eq!(value["software"][0]["adapter"]["available"], false);
}

#[test]
fn refreshed_catalog_drives_new_software_and_preparation_without_a_core_release() {
    use crate::domain::install_catalog::{CatalogProvenance, CatalogSource};

    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("data");
    let _env = meshy_dcc_test_utils::EnvVarGuard::set("MESHY_DCC_DATA_DIR", data.to_str());
    let project_plan = root.path().join("existing-project-launch-plan.json");
    std::fs::write(&project_plan, "existing project setup remains unchanged").unwrap();
    let editor = root.path().join("new-editor");
    std::fs::write(&editor, "fixture, never executed").unwrap();
    let entry: CatalogEntry = serde_json::from_value(json!({
        "name":"new-game-editor", "description":"new published adapter", "dcc":["new-editor"],
        "tags":["adapter"], "version":"9.8.7", "policy":{"installation":"available"},
        "install":{"type":"host-package", "url":"https://example.invalid/new-editor.zip",
            "sha256":"a".repeat(64), "host":{"extract":true, "dcc_paths":[editor],
            "project_steps":[["{package_dir}/installer", "{project}"]]}}
    }))
    .unwrap();
    // The existing resolver verifies signatures before constructing this service snapshot.
    let mut service = InstallService::bundled();
    service.require_fresh_catalog = true;
    service.catalog_snapshot = Some(super::super::catalog::ResolvedCatalog {
        entries: vec![entry],
        provenance: CatalogProvenance {
            source: CatalogSource::Remote,
            latest_checked: true,
            issued_at: Some(1),
            expires_at: Some(u64::MAX),
            sha256: Some("b".repeat(64)),
            source_revision: Some("c".repeat(40)),
        },
    });
    let software = service.observe_software(None).unwrap();
    assert_eq!(software["software"][0]["dcc_type"], "new-editor");
    assert_eq!(software["software"][0]["detection"], "found");
    assert_eq!(software["software"][0]["adapter"]["version"], "9.8.7");
    let planned = service
        .prepare_adapter("new-editor", None, None, false)
        .unwrap();
    assert_eq!(planned["plan"]["catalog"]["source"], "remote");
    assert_eq!(
        planned["plan"]["adapter"]["install"]["sha256"],
        "a".repeat(64)
    );

    let snapshot = service.catalog_snapshot.as_mut().unwrap();
    snapshot.provenance.source = CatalogSource::Cache;
    snapshot.provenance.latest_checked = false;
    assert_eq!(service.observe_software(None).unwrap(), software);
    let refused = service
        .prepare_adapter("new-editor", None, None, true)
        .unwrap();
    assert_eq!(refused["status"], "failed");
    assert_eq!(
        refused["report"]["error"]["code"],
        "INSTALL_CATALOG_UNAVAILABLE"
    );

    let snapshot = service.catalog_snapshot.as_mut().unwrap();
    snapshot.entries[0].policy.as_mut().unwrap().installation = "not_available".into();
    assert_eq!(
        service.observe_software(None).unwrap()["software"][0]["adapter"]["available"],
        false
    );
    assert!(
        service
            .prepare_adapter("new-editor", None, None, true)
            .is_err()
    );
    assert!(
        !data.exists(),
        "refusal must not acquire or prepare any adapter"
    );
    assert_eq!(
        std::fs::read_to_string(project_plan).unwrap(),
        "existing project setup remains unchanged"
    );
}

#[tokio::test]
async fn rejected_default_cache_blocks_preparation_but_explicit_catalog_stays_independent() {
    let root = tempfile::tempdir().unwrap();
    let cache = root.path().join("catalog-cache.json");
    let data = root.path().join("data");
    let _env = meshy_dcc_test_utils::EnvVarsGuard::set(&[
        ("MESHY_DCC_INSTALL_CACHE", cache.to_str()),
        ("MESHY_DCC_DATA_DIR", data.to_str()),
    ]);
    std::fs::write(&cache, "invalid catalog bytes").unwrap();
    assert!(observe_software(None).is_err());
    // Cache authentication precedes any remote refresh or package acquisition.
    assert!(prepare_adapter("godot", None, None, true).await.is_err());
    assert!(!data.exists());
    assert_eq!(
        std::fs::read_to_string(&cache).unwrap(),
        "invalid catalog bytes"
    );
    assert!(!cache.with_extension("lock").exists());

    let catalog = root.path().join("explicit-catalog.json");
    std::fs::write(
        &catalog,
        json!({"entries":[{
            "name":"explicit-fixture", "description":"fixture", "dcc":["fixture"],
            "tags":["adapter"], "version":"1", "policy":{"installation":"available"},
            "install":{"type":"host-package", "url":"https://example.invalid/fixture.zip",
                "sha256":"a".repeat(64), "host":{"extract":true}}
        }]})
        .to_string(),
    )
    .unwrap();
    assert_eq!(
        observe_software(Some(&catalog)).unwrap()["software"][0]["dcc_type"],
        "fixture"
    );
    assert_eq!(
        prepare_adapter("fixture", None, Some(&catalog), false)
            .await
            .unwrap()["status"],
        "planned"
    );
    assert!(!data.exists());
}

#[test]
fn game_engine_observations_find_all_editors_without_selecting_a_project_version() {
    for (dcc, platform, first, second) in [
        (
            "unity",
            "macos-arm64",
            "absolute/Applications/Unity/Hub/Editor/2022.3.46f1c1/Unity.app/Contents/MacOS/Unity",
            "absolute/Applications/Unity/Hub/Editor/6000.6.5f1/Unity.app/Contents/MacOS/Unity",
        ),
        (
            "unity",
            "windows-x86_64",
            "drive-c/Program Files/Unity/Hub/Editor/6000.6.5f1/Editor/Unity.exe",
            "drive-c/Program Files (x86)/Unity/Hub/Editor/2022.3.46f1c1/Editor/Unity.exe",
        ),
        (
            "unreal",
            "macos-arm64",
            "absolute/Users/Shared/Epic Games/UE_5.6/Engine/Binaries/Mac/UnrealEditor.app/Contents/MacOS/UnrealEditor",
            "absolute/Users/Shared/Epic Games/UE_5.7/Engine/Binaries/Mac/UnrealEditor.app/Contents/MacOS/UnrealEditor",
        ),
        (
            "unreal",
            "windows-x86_64",
            "drive-c/Program Files/Epic Games/UE_5.6/Engine/Binaries/Win64/UnrealEditor.exe",
            "drive-c/Program Files/Epic Games/UE_5.7/Engine/Binaries/Win64/UnrealEditor.exe",
        ),
    ] {
        let root = tempfile::tempdir().unwrap();
        let _env =
            meshy_dcc_test_utils::EnvVarGuard::set("MESHY_DCC_DATA_DIR", root.path().to_str());
        let mut entry = meshy_dcc_catalog::load_from_str(super::super::BUNDLED_CATALOG)
            .unwrap()
            .into_iter()
            .find(|entry| entry.dcc.iter().any(|name| name == dcc))
            .unwrap();
        let install = entry.install.as_mut().unwrap();
        install.select_platform(platform).unwrap();
        // Exercise either platform on the test runner, using only fixture files.
        install.platforms.clear();
        for pattern in &mut install.host.as_mut().unwrap().dcc_paths {
            let relative = if let Some(rest) = pattern.strip_prefix("C:/") {
                format!("drive-c/{rest}")
            } else {
                format!("absolute/{}", pattern.trim_start_matches('/'))
            };
            *pattern = root.path().join(relative).to_string_lossy().into_owned();
        }
        let catalog = root.path().join("catalog.json");
        std::fs::write(&catalog, json!({"entries": [entry]}).to_string()).unwrap();
        let missing = observe_software(Some(&catalog)).unwrap();
        assert_eq!(missing["software"][0]["detection"], "not_observed");

        let editors = [root.path().join(first), root.path().join(second)];
        for (index, path) in editors.iter().enumerate() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "fixture editor, never executed").unwrap();
            let observed = observe_software(Some(&catalog)).unwrap();
            let row = &observed["software"][0];
            assert_eq!(
                row["detection"],
                if index == 0 { "found" } else { "multiple" }
            );
            assert_eq!(row["candidates"].as_array().unwrap().len(), index + 1);
            assert!(
                row["candidates"]
                    .as_array()
                    .unwrap()
                    .contains(&json!({"path": path}))
            );
            assert_eq!(row["adapter"]["per_project"], true);
        }

        // Preparing a project adapter downloads its package, not a chosen editor.
        // Multiple observed versions must not introduce a global selection gate.
        let planned = InstallService::bundled()
            .prepare_adapter(dcc, None, Some(&catalog), false)
            .unwrap();
        let plan: crate::domain::install::InstallPlan =
            serde_json::from_value(planned["plan"].clone()).unwrap();
        assert!(plan.dcc_path.is_none());
        assert!(plan.steps.iter().any(|step| matches!(
            &step.action,
            Some(InstallStepAction::HostPackage { dcc_path: None, project: None, project_steps, .. })
                if project_steps.is_empty()
        )));
        assert!(!plan.steps.iter().any(|step| step.name == "locate-host"));
    }
}

#[test]
fn global_preparation_publishes_its_existing_launch_recipe_without_a_project() {
    let temp = tempfile::tempdir().unwrap();
    let registry = temp.path().join("registry");
    let data = temp.path().join("data");
    let _env = meshy_dcc_test_utils::EnvVarsGuard::set(&[
        ("MESHY_DCC_DATA_DIR", data.to_str()),
        ("MESHY_DCC_REGISTRY_DIR", registry.to_str()),
    ]);
    let executable = temp.path().join("host");
    std::fs::write(&executable, "fixture executable; never launched").unwrap();
    let catalog = temp.path().join("catalog.json");
    std::fs::write(&catalog, json!({"entries":[{"name":"fixture-adapter","description":"fixture","dcc":["fixture"],"tags":["adapter"],"version":"1.0.0",
        "policy":{"installation":"available"},"install":{"type":"host-package","url":"https://example.invalid/fixture.zip","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "host":{"extract":true,"dcc_paths":[executable],"launch":["{executable}","--fixture"]}}}]}).to_string()).unwrap();
    let planned = InstallService::bundled()
        .prepare_adapter("fixture", None, Some(&catalog), false)
        .unwrap();
    let plan: crate::domain::install::InstallPlan =
        serde_json::from_value(planned["plan"].clone()).unwrap();
    assert_eq!(
        plan.steps
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        ["install-host-package", "write-launch-plan"]
    );
    // The neighboring preparation test covers real checksum acquisition. Here
    // isolate the retained launch-plan executor and never invoke a host.
    let report = InstallService::bundled().execute_plan_with(
        &plan,
        true,
        |action, _| match action {
            InstallStepAction::HostPackage { .. } => {
                Ok(super::super::StepExecution::Completed(None))
            }
            _ => super::super::execute_action(action),
        },
        super::super::execute_rollback,
    );
    assert_eq!(report.status, "ok");
    let published: Value =
        serde_json::from_slice(&std::fs::read(registry.join("launch-plans/fixture.json")).unwrap())
            .unwrap();
    assert_eq!(
        published["executable"],
        executable.to_string_lossy().as_ref()
    );
    assert_eq!(published["argv"], json!(["{executable}", "--fixture"]));
    assert!(published.get("project").is_none());
    let prepared = PreparedAdapter {
        schema_version: 1,
        entry: plan.adapter.clone(),
        dcc_path: Some(executable.clone()),
        package_dir: None,
        prepared_at_unix: 0,
    };
    let receipt = receipt_path("fixture");
    std::fs::create_dir_all(receipt.parent().unwrap()).unwrap();
    std::fs::write(&receipt, serde_json::to_vec(&prepared).unwrap()).unwrap();
    let other = temp.path().join("other-host");
    std::fs::write(&other, "never launched").unwrap();
    assert!(
        validate_prepared_project_target("fixture", temp.path(), Some(&executable), &registry)
            .is_ok()
    );
    assert!(
        configure_prepared_project_before(
            "fixture",
            temp.path(),
            Some(other.clone()),
            None,
            &registry
        )
        .unwrap_err()
        .to_string()
        .contains("cannot select dcc_path")
    );
    let req = serde_json::from_value(json!({"action":"configure_project","dcc_type":"fixture","project":temp.path(),"request_id":"unsupported-selector","dcc_path":other})).unwrap();
    let caller = crate::application::lifecycle::CallerContext {
        owner: "fixture".into(),
        agent_context: json!({"session_id":"fixture"}),
    };
    assert!(
        crate::application::lifecycle::submit(&registry, req, caller)
            .unwrap_err()
            .to_string()
            .contains("cannot select dcc_path")
    );
    assert!(
        !std::fs::read_dir(registry.join("start-instance"))
            .unwrap()
            .any(|e| {
                e.unwrap()
                    .path()
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| uuid::Uuid::parse_str(s).is_ok())
            })
    );
    let mut no_selector = prepared.clone();
    no_selector
        .entry
        .install
        .as_mut()
        .unwrap()
        .host
        .as_mut()
        .unwrap()
        .project_steps = vec![vec!["{package_dir}/setup".into(), "{project}".into()]];
    std::fs::write(&receipt, serde_json::to_vec(&no_selector).unwrap()).unwrap();
    assert!(
        configure_prepared_project_before(
            "fixture",
            temp.path(),
            Some(other.clone()),
            None,
            &registry
        )
        .unwrap_err()
        .to_string()
        .contains("cannot select dcc_path"),
        "nonempty project steps must not imply executable selection support"
    );
    std::fs::write(&receipt, serde_json::to_vec(&prepared).unwrap()).unwrap();
    // A newer published recipe wins over an older preparation receipt.
    let mut changed = published;
    changed["executable"] = json!(other);
    std::fs::write(
        registry.join("launch-plans/fixture.json"),
        changed.to_string(),
    )
    .unwrap();
    assert!(
        validate_prepared_project_target("fixture", temp.path(), Some(&executable), &registry)
            .is_err()
    );
    assert!(
        validate_prepared_project_target("fixture", temp.path(), Some(&other), &registry).is_ok()
    );
}
