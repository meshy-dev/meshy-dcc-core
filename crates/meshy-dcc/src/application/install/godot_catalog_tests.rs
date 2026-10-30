use super::*;

fn godot_entry(platform: &str) -> CatalogEntry {
    let mut entry = meshy_dcc_catalog::load_from_str(BUNDLED_CATALOG)
        .unwrap()
        .into_iter()
        .find(|entry| entry.name == "meshy-dcc-godot")
        .unwrap();
    entry
        .install
        .as_mut()
        .unwrap()
        .select_platform(platform)
        .unwrap();
    entry
}

fn plan(entry: CatalogEntry, dcc_path: Option<PathBuf>, project: &Path) -> InstallPlan {
    InstallPlanner::plan(
        &[entry],
        InstallRequest {
            dcc_type: "godot".into(),
            version: None,
            catalog_path: None,
            python: None,
            dcc_path,
            plugin_source: None,
            adobe_debug_root: None,
            project: Some(project.into()),
        },
    )
    .unwrap()
}

fn assert_editor_argument(plan: &InstallPlan, expected: &Path, package: &Path) {
    let action = plan.steps.iter().find_map(|step| match &step.action {
        Some(InstallStepAction::HostPackage {
            dcc_path,
            extract_to,
            project,
            project_steps,
            ..
        }) => Some((dcc_path, extract_to, project, project_steps)),
        _ => None,
    });
    let (dcc_path, extract_to, project, project_steps) = action.expect("executable install plan");
    assert_eq!(dcc_path.as_deref(), Some(expected));
    let placeholders = HostPlaceholders {
        targets: HostTargets {
            dcc_path: dcc_path.as_deref(),
            extract_to: extract_to.as_deref(),
            project: project.as_deref(),
        },
        package,
    };
    assert!(!project_steps.is_empty());
    for argv in project_steps {
        let command = placeholders.command(argv).unwrap();
        let args: Vec<_> = command.get_args().collect();
        let selected = args
            .windows(2)
            .find(|pair| pair[0] == "--dcc-path")
            .expect("the adapter installer must receive the selected editor");
        assert_eq!(selected[1], expected.as_os_str());
    }
}

fn editor(path: &Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, b"fixture editor, never executed").unwrap();
}

#[test]
fn godot_catalog_forwards_the_explicit_editor_for_each_platform() {
    let root = tempfile::tempdir().unwrap();
    let selected = root.path().join("custom editor with spaces/Godot");
    editor(&selected);
    for platform in ["macos-arm64", "windows-x86_64"] {
        let plan = plan(godot_entry(platform), Some(selected.clone()), root.path());
        assert_editor_argument(&plan, &selected, &root.path().join("adapter.zip"));
    }
}

#[test]
fn godot_catalog_discovers_one_editor_and_requires_a_choice_for_several() {
    for (platform, first, second) in [
        (
            "macos-arm64",
            "absolute/Applications/Godot.app/Contents/MacOS/Godot",
            "home/Applications/Godot.app/Contents/MacOS/Godot",
        ),
        (
            "windows-x86_64",
            "drive-c/Program Files/Godot/Godot_v4.7.2-stable_win64.exe",
            "home/Downloads/Godot_v4.6.1-stable_win64.exe",
        ),
    ] {
        let root = tempfile::tempdir().unwrap();
        let mut entry = godot_entry(platform);
        let host = entry.install.as_mut().unwrap().host.as_mut().unwrap();
        assert!(
            !host.dcc_paths.is_empty(),
            "{platform} needs default discovery"
        );
        // Relocate the real catalog patterns into a fixture tree, without
        // reading installed editors or changing the process home directory.
        for pattern in &mut host.dcc_paths {
            let relative = if let Some(rest) = pattern.strip_prefix("~/") {
                format!("home/{rest}")
            } else if let Some(rest) = pattern.strip_prefix("C:/") {
                format!("drive-c/{rest}")
            } else {
                format!("absolute/{}", pattern.trim_start_matches('/'))
            };
            *pattern = root.path().join(relative).to_string_lossy().into_owned();
        }
        let missing = plan(entry.clone(), None, root.path());
        assert!(
            missing
                .steps
                .iter()
                .any(|step| step.name == "locate-host" && step.action.is_none())
        );

        let selected = root.path().join(first);
        editor(&selected);
        if platform == "windows-x86_64" {
            editor(&selected.with_file_name("Godot_v4.7.2-stable_win64_console.exe"));
        }
        let found = plan(entry.clone(), None, root.path());
        assert_editor_argument(&found, &selected, &root.path().join("adapter.zip"));

        editor(&root.path().join(second));
        let ambiguous = plan(entry, None, root.path());
        let locate = ambiguous
            .steps
            .iter()
            .find(|step| step.name == "locate-host")
            .unwrap();
        assert!(locate.action.is_none());
        assert!(locate.description.contains("Found more than one godot"));
    }
}
