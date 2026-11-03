//! Read-only activity attribution. These values filter observations; they do
//! not authenticate a caller or grant execution/stop authority.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallerFilter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
}

impl CallerFilter {
    pub fn is_scoped(&self) -> bool {
        self.agent_name.is_some() || self.session_id.is_some() || self.turn_id.is_some()
    }

    pub fn attributable(&self, context: Option<&Value>) -> bool {
        [&self.agent_name, &self.session_id, &self.turn_id]
            .iter()
            .zip(["agent_name", "session_id", "turn_id"])
            .all(|(wanted, key)| {
                wanted.is_none()
                    || context
                        .and_then(|c| c.get(key))
                        .and_then(Value::as_str)
                        .is_some()
            })
    }

    pub fn matches(&self, context: Option<&Value>) -> bool {
        [&self.agent_name, &self.session_id, &self.turn_id]
            .iter()
            .zip(["agent_name", "session_id", "turn_id"])
            .all(|(wanted, key)| {
                wanted.as_deref().is_none_or(|s| {
                    context.and_then(|c| c.get(key)).and_then(Value::as_str) == Some(s)
                })
            })
    }
}

/// Retain identity and release metadata without copying prompts into job rows.
pub fn job_agent_context(meta: Option<&Value>) -> Option<Value> {
    let source = meta?.get("agent_context")?.as_object()?;
    let mut output = serde_json::Map::new();
    for key in [
        "actor_id",
        "agent_id",
        "agent_name",
        "agent_version",
        "session_id",
        "turn_id",
        "metadata",
    ] {
        if let Some(value) = source.get(key) {
            output.insert(key.into(), value.clone());
        }
    }
    let output = Value::Object(output);
    (serde_json::to_vec(&output).ok()?.len() <= 16 * 1024).then_some(output)
}
