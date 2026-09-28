//! Skill search path entries for admin UI and embedder snapshots.

use serde::{Deserialize, Serialize};

/// One resolved skill directory with a human-readable source label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillPathEntry {
    /// Absolute or normalised directory path.
    pub path: String,
    /// Origin label, e.g. `cli`, `env:MESHY_DCC_SKILL_PATHS`, `bundled`, `admin_custom`.
    pub source: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_roundtrip() {
        let entry = SkillPathEntry {
            path: "/opt/skills/maya".to_string(),
            source: "cli".to_string(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: SkillPathEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, back);
    }

    #[test]
    fn deserialize_from_json_literal() {
        let json =
            r#"{"path":"/home/user/.meshy-dcc/skills","source":"env:MESHY_DCC_SKILL_PATHS"}"#;
        let entry: SkillPathEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.path, "/home/user/.meshy-dcc/skills");
        assert_eq!(entry.source, "env:MESHY_DCC_SKILL_PATHS");
    }
}
