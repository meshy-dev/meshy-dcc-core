//! Serialization / serde round-trip tests for ToolMeta.

use super::*;

// ── Serialization ───────────────────────────────────────────────────────────

#[test]
fn test_action_meta_serde_round_trip() {
    let meta = ToolMeta {
        name: "render_scene".into(),
        description: "Renders the active scene".into(),
        category: "rendering".into(),
        tags: vec!["render".into(), "output".into()],
        search_aliases: vec!["batch render".into()],
        dcc: "houdini".into(),
        version: "3.1.0".into(),
        input_schema: serde_json::json!({"type": "object"}),
        output_schema: serde_json::json!({"type": "string"}),
        source_file: Some("render.py".into()),
        skill_name: None,
        group: String::new(),
        enabled: true,
        required_capabilities: vec!["scene".into(), "render".into()],
        execution: meshy_dcc_models::ExecutionMode::Async,
        timeout_hint_secs: Some(900),
        job_strategy: meshy_dcc_models::JobStrategy::Isolated,
        thread_affinity: meshy_dcc_models::ThreadAffinity::Any,
        enforce_thread_affinity: false,
        annotations: meshy_dcc_models::SkillToolAnnotations::default(),
        next_tools: meshy_dcc_models::NextTools::default(),
    };
    let json = serde_json::to_string(&meta).unwrap();
    let back: ToolMeta = serde_json::from_str(&json).unwrap();
    assert_eq!(meta, back);
}

#[test]
fn test_action_meta_execution_defaults_to_sync() {
    let meta = ToolMeta::default();
    assert_eq!(meta.execution, meshy_dcc_models::ExecutionMode::Sync);
    assert_eq!(meta.timeout_hint_secs, None);
    assert_eq!(meta.job_strategy, meshy_dcc_models::JobStrategy::Monolithic);
}

#[test]
fn test_action_meta_execution_and_timeout_serde() {
    // Issue #317 — new fields must round-trip and be recognised on input.
    let json = r#"{
        "name": "render",
        "description": "Render",
        "execution": "async",
        "timeout_hint_secs": 600,
        "job_strategy": "chunked"
    }"#;
    let meta: ToolMeta = serde_json::from_str(json).unwrap();
    assert_eq!(meta.execution, meshy_dcc_models::ExecutionMode::Async);
    assert_eq!(meta.timeout_hint_secs, Some(600));
    assert_eq!(meta.job_strategy, meshy_dcc_models::JobStrategy::Chunked);
}

#[test]
fn test_action_meta_default_serialization() {
    let meta = ToolMeta::default();
    let json = serde_json::to_string(&meta).unwrap();
    let back: ToolMeta = serde_json::from_str(&json).unwrap();
    assert_eq!(meta, back);
}
