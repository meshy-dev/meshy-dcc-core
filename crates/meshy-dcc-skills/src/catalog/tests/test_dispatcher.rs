//! Tests for Skills-First dispatcher integration.
use super::fixtures::{make_catalog_with_dispatcher, make_test_skill};
use super::*;

#[test]
fn test_load_skill_registers_dispatcher_handler_for_scripts() {
    let (catalog, dispatcher) = make_catalog_with_dispatcher();

    // Skill with no tool declarations — script-only path
    let mut skill = make_test_skill("echo-skill", "python", &[]);
    skill.scripts = vec!["/fake/echo.py".to_string()];
    catalog.add_skill(skill);

    let actions = catalog.load_skill("echo-skill").unwrap();
    assert_eq!(actions.len(), 1);
    // Handler auto-registered in dispatcher
    assert!(dispatcher.has_handler("echo_skill__echo"));
}

#[test]
fn test_unload_skill_removes_dispatcher_handlers() {
    let (catalog, dispatcher) = make_catalog_with_dispatcher();

    let mut skill = make_test_skill("rm-skill", "python", &[]);
    skill.scripts = vec!["/fake/run.py".to_string()];
    catalog.add_skill(skill);

    catalog.load_skill("rm-skill").unwrap();
    assert!(dispatcher.has_handler("rm_skill__run"));

    catalog.unload_skill("rm-skill").unwrap();
    assert!(!dispatcher.has_handler("rm_skill__run"));
}

#[test]
fn test_load_skill_with_tool_decl_and_source_file() {
    let (catalog, dispatcher) = make_catalog_with_dispatcher();

    let skill = SkillMetadata {
        name: "explicit-skill".to_string(),
        description: "Explicit source file".to_string(),
        tools: vec![meshy_dcc_models::ToolDeclaration {
            name: "do_thing".to_string(),
            source_file: "/fake/do_thing.py".to_string(),
            ..Default::default()
        }],
        ..Default::default()
    };
    catalog.add_skill(skill);

    let actions = catalog.load_skill("explicit-skill").unwrap();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0], "explicit_skill__do_thing");
    assert!(dispatcher.has_handler("explicit_skill__do_thing"));
    // Verify source_file propagated to ToolMeta
    let meta = dispatcher
        .registry()
        .get_action("explicit_skill__do_thing", None)
        .unwrap();
    assert_eq!(meta.source_file, Some("/fake/do_thing.py".to_string()));
}

/// A script that starts a background child which writes `late.txt` a second
/// later, the shape of a skill that shells out to a host binary.
#[cfg(unix)]
fn late_writer_skill(dir: &std::path::Path) -> SkillMetadata {
    let script = dir.join("late.sh");
    std::fs::write(
        &script,
        "dir=\"$2\"\n( sleep 1; echo late > \"$dir/late.txt\" ) &\n: > \"$dir/ready\"\nwait\n",
    )
    .unwrap();
    let mut skill = make_test_skill("stop-probe", "python", &[]);
    skill.scripts = vec![script.to_string_lossy().into_owned()];
    skill
}

#[cfg(unix)]
#[test]
fn test_terminate_all_cancels_running_script_and_its_children() {
    let dir = tempfile::tempdir().unwrap();
    let (catalog, dispatcher) = make_catalog_with_dispatcher();
    catalog.add_skill(late_writer_skill(dir.path()));
    catalog.load_skill("stop-probe").unwrap();

    let params = serde_json::json!({"dir": dir.path().to_string_lossy()});
    let call = {
        let dispatcher = dispatcher.clone();
        let params = params.clone();
        std::thread::spawn(move || dispatcher.dispatch("stop_probe__late", params, None))
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !dir.path().join("ready").exists() {
        assert!(std::time::Instant::now() < deadline, "script never started");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    let processes = catalog.script_processes();
    assert_eq!(processes.terminate_all(), 1);
    let err = call
        .join()
        .unwrap()
        .expect_err("a terminated call must fail");
    assert!(err.to_string().contains("CANCELLED"), "{err}");
    assert_eq!(processes.running(), 0);

    // The background child died with the script: nothing writes after the stop.
    std::thread::sleep(std::time::Duration::from_millis(1500));
    assert!(
        !dir.path().join("late.txt").exists(),
        "late write after stop"
    );

    // A stopped server starts nothing new until it starts again.
    let refused = dispatcher
        .dispatch("stop_probe__late", params, None)
        .expect_err("a stopping server must not start scripts");
    assert!(refused.to_string().contains("not started"), "{refused}");
    processes.reopen();
}
