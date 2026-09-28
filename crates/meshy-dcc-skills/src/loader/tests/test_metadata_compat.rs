//! Tests for issue #356: metadata.meshy-dcc.* parsing and strict
//! rejection of legacy top-level extension keys.
//!
//! The pre-0.15 flat-form shorthand (`metadata: { "meshy-dcc.dcc": ... }`)
//! is no longer recognised; only the nested form
//! (`metadata: { meshy-dcc: { dcc: ... } }`) drives the typed
//! `SkillMetadata` extensions.
use super::fixtures::write_skill;
use super::*;

#[test]
fn legacy_top_level_keys_are_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("legacy");
    write_skill(
        &dir,
        "---\nname: legacy\ndcc: maya\nversion: \"2.0.0\"\ntags: [a, b]\n---\n# body\n",
    );
    assert!(
        parse_skill_md(&dir).is_none(),
        "legacy top-level keys must cause the skill to be rejected"
    );
}

#[test]
fn top_level_version_rejection_reports_nested_replacement() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("maya-mgear");
    write_skill(
        &dir,
        "---\nname: maya-mgear\ndescription: mGear tools\nversion: \"1.0.0\"\n---\n# body\n",
    );

    let diagnostic = parse_skill_md_with_diagnostic(&dir).unwrap_err();

    assert_eq!(diagnostic.skill_name, "maya-mgear");
    assert_eq!(diagnostic.directory_name, "maya-mgear");
    assert_eq!(diagnostic.reason_code, "non_spec_top_level_keys");
    assert_eq!(diagnostic.offending_keys, vec!["version".to_string()]);
    assert!(diagnostic.message.contains("non-spec top-level key"));
    assert!(
        diagnostic
            .suggested_fix
            .contains("metadata.meshy-dcc.version")
    );
    assert!(
        !diagnostic
            .message
            .contains(tmp.path().to_string_lossy().as_ref())
    );
}

#[test]
fn legacy_flat_form_does_not_populate_typed_fields() {
    // The pre-0.15 flat shorthand is no longer parsed into
    // `SkillMetadata` typed fields; the skill parses (the YAML itself
    // is valid) but every typed override stays at its default so the
    // author notices the gap and migrates.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("flat_legacy");
    let body = r#"---
name: flat-legacy
description: pre-0.15 shorthand
metadata:
  meshy-dcc.dcc: maya
  meshy-dcc.version: "2.0.0"
  meshy-dcc.tags: "a, b"
---
# body
"#;
    write_skill(&dir, body);
    let meta = parse_skill_md(&dir).expect("parsed");
    assert_ne!(
        meta.dcc, "maya",
        "flat-form `meshy-dcc.dcc: maya` must NOT populate the typed field; \
         it should fall back to the serde default"
    );
    assert_ne!(
        meta.version, "2.0.0",
        "flat-form `meshy-dcc.version` must NOT populate the typed field"
    );
    assert!(
        meta.tags.is_empty(),
        "flat-form tags must NOT be recognised"
    );
}

#[test]
fn nested_form_parses_successfully() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("new");
    let body = r#"---
name: new
description: new-form skill
metadata:
  meshy-dcc:
    dcc: maya
    version: "2.0.0"
    tags: "a, b"
    search-hint: "hint words"
    search-aliases: [make sphere, primitive ball]
    depends: "other-skill"
---
# body
"#;
    write_skill(&dir, body);
    let meta = parse_skill_md(&dir).expect("parsed");
    assert_eq!(meta.dcc, "maya");
    assert_eq!(meta.version, "2.0.0");
    assert_eq!(meta.tags, vec!["a".to_string(), "b".to_string()]);
    assert_eq!(meta.search_hint, "hint words");
    assert_eq!(
        meta.search_aliases,
        vec!["make sphere".to_string(), "primitive ball".to_string()]
    );
    assert_eq!(meta.depends, vec!["other-skill".to_string()]);
}

#[test]
fn nested_form_with_inline_tools_list_parses() {
    // Canonical agentskills.io shape — `metadata.meshy-dcc` is a nested map.
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("nested");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("tools.yaml"),
        "tools:\n  - name: create_sphere\n    description: make a sphere\n    search_aliases: [primitive ball, mesh globe]\n",
    )
    .unwrap();
    let body = r#"---
name: nested
description: nested metadata form
metadata:
  meshy-dcc:
    dcc: maya
    version: "1.0.0"
    tags: [maya, animation]
    search-hint: "keyframe, timeline"
    tools: tools.yaml
---
# body
"#;
    std::fs::write(dir.join(SKILL_METADATA_FILE), body).unwrap();
    let meta = parse_skill_md(&dir).expect("parsed");
    assert_eq!(meta.dcc, "maya");
    assert_eq!(meta.version, "1.0.0");
    assert_eq!(meta.tags, vec!["maya".to_string(), "animation".to_string()]);
    assert_eq!(meta.search_hint, "keyframe, timeline");
    assert_eq!(meta.tools.len(), 1);
    assert_eq!(meta.tools[0].name, "create_sphere");
    assert_eq!(
        meta.tools[0].search_aliases,
        vec!["primitive ball".to_string(), "mesh globe".to_string()]
    );
}

#[test]
fn nested_form_sibling_tools_yaml_resolves() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("sidecar");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("tools.yaml"),
        "tools:\n  - name: create_sphere\n    description: make a sphere\n  - ping\ngroups:\n  - name: advanced\n    default-active: false\n    tools: [create_sphere]\n",
    )
    .unwrap();
    let body = r#"---
name: sidecar
metadata:
  meshy-dcc:
    dcc: maya
    tools: tools.yaml
---
# body
"#;
    std::fs::write(dir.join(SKILL_METADATA_FILE), body).unwrap();
    let meta = parse_skill_md(&dir).expect("parsed");
    assert_eq!(meta.tools.len(), 2);
    assert_eq!(meta.tools[0].name, "create_sphere");
    assert_eq!(meta.tools[0].description, "make a sphere");
    assert_eq!(meta.tools[1].name, "ping");
    assert_eq!(meta.groups.len(), 1);
    assert_eq!(meta.groups[0].name, "advanced");
    assert!(!meta.groups[0].default_active);
}

#[test]
fn nested_form_resources_file_reference_resolves() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("resources-sidecar");
    let body = r#"---
name: resources-sidecar
metadata:
  meshy-dcc:
    resources: resources/
---
# body
"#;
    write_skill(&dir, body);
    let meta = parse_skill_md(&dir).expect("parsed");
    assert_eq!(meta.resources_file.as_deref(), Some("resources/"));
}

#[test]
fn nested_form_parses_optional_runtime_descriptors() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("runtime");
    let body = r#"---
name: runtime
metadata:
  meshy-dcc:
    runtimes:
      - name: usd-core
        type: python_package
        package: usd-core
        module: pxr
        optional: true
        feature_level: full-usd
        install_hint: "pip install meshy-dcc-openusd[usd-core]"
      - name: usdcat
        type: binary
        binary: usdcat
        optional: true
        guidance: "Install OpenUSD command-line tools."
      - name: HFS
        type: env_var
        env: HFS
        optional: true
        description: Houdini Solaris runtime.
---
# body
"#;
    write_skill(&dir, body);
    let meta = parse_skill_md(&dir).expect("parsed");
    assert_eq!(meta.runtimes.len(), 3);
    assert_eq!(meta.runtimes[0].name, "usd-core");
    assert_eq!(
        meta.runtimes[0].kind,
        meshy_dcc_models::SkillRuntimeKind::PythonPackage
    );
    assert_eq!(
        meta.runtimes[0].guidance.as_deref(),
        Some("pip install meshy-dcc-openusd[usd-core]")
    );
    assert_eq!(meta.runtimes[1].binary.as_deref(), Some("usdcat"));
    assert_eq!(meta.runtimes[2].env.as_deref(), Some("HFS"));
}

#[test]
fn nested_form_parses_runtime_descriptors_from_sibling_file() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("runtime-sidecar");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("runtimes.yaml"),
        "runtimes:\n  - name: usdcat\n    type: binary\n    binary: usdcat\n    optional: true\n",
    )
    .unwrap();
    let body = r#"---
name: runtime-sidecar
metadata:
  meshy-dcc:
    runtimes: runtimes.yaml
---
# body
"#;
    std::fs::write(dir.join(SKILL_METADATA_FILE), body).unwrap();
    let meta = parse_skill_md(&dir).expect("parsed");
    assert_eq!(meta.runtimes.len(), 1);
    assert_eq!(meta.runtimes[0].name, "usdcat");
    assert_eq!(meta.runtimes[0].binary.as_deref(), Some("usdcat"));
}

#[test]
fn nested_form_products_and_implicit_invocation() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("policy");
    let body = r#"---
name: policy
metadata:
  meshy-dcc:
    products: "maya, houdini"
    allow-implicit-invocation: "false"
---
# body
"#;
    write_skill(&dir, body);
    let meta = parse_skill_md(&dir).expect("parsed");
    let policy = meta.policy.expect("policy must be set");
    assert_eq!(
        policy.products,
        vec!["maya".to_string(), "houdini".to_string()]
    );
    assert_eq!(policy.allow_implicit_invocation, Some(false));
}
