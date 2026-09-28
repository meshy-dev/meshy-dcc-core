//! Tests for metadata.meshy-dcc.layer field parsing.
//!
//! Since the flat-form shorthand was dropped, only the canonical nested
//! `metadata.meshy-dcc.*` shape feeds the typed `SkillMetadata::layer`
//! field. The legacy flat form (`metadata: { "meshy-dcc.layer": ... }`)
//! no longer populates the typed field — the frontmatter key has a
//! literal dot inside and is treated as an opaque custom key; it is
//! still preserved in `SkillMetadata::metadata` for callers that
//! inspect the raw map but does NOT drive the typed extensions.
use super::fixtures::write_skill;
use super::*;

#[test]
fn flat_form_layer_is_no_longer_parsed_into_typed_field() {
    // Legacy flat form: the loader no longer recognises
    // `metadata.meshy-dcc.layer`. The field must stay `None`, surfacing
    // the migration need clearly (vs. silently using a stale value).
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("flat_legacy");
    write_skill(
        &dir,
        r#"---
name: flat-legacy
description: A pre-0.15 skill using the dropped flat-form shorthand.
metadata:
  meshy-dcc.dcc: maya
  meshy-dcc.layer: domain
---
"#,
    );
    let meta = parse_skill_md(&dir).expect("parsed");
    assert!(
        meta.layer.is_none(),
        "legacy flat-form meshy-dcc.layer must not populate the typed field any more; \
         migrate to `metadata: {{ meshy-dcc: {{ layer: domain }} }}`",
    );
}

#[test]
fn layer_field_is_parsed_nested_form() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("nested_layer");
    write_skill(
        &dir,
        r#"---
name: nested-layer
description: An infrastructure skill.
metadata:
  meshy-dcc:
    dcc: python
    layer: infrastructure
---
"#,
    );
    let meta = parse_skill_md(&dir).expect("parsed");
    assert_eq!(
        meta.layer.as_deref(),
        Some("infrastructure"),
        "nested meshy-dcc.layer must be parsed"
    );
}

#[test]
fn layer_field_none_when_absent() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("no_layer");
    write_skill(
        &dir,
        "---\nname: no-layer\ndescription: no layer key set\n---\n",
    );
    let meta = parse_skill_md(&dir).expect("parsed");
    assert!(
        meta.layer.is_none(),
        "layer must be None when not declared in SKILL.md"
    );
}
