use super::*;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};

#[cfg(unix)]
#[test]
fn preparation_needs_no_project_and_later_configuration_uses_the_verified_local_package() {
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
    let result = prepare_adapter("fixture", None, Some(&catalog), true).unwrap();
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
    let planned = prepare_adapter("fixture", None, Some(&catalog), false).unwrap();
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
}
