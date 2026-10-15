//! Shared skill-catalog reload hook used by admin handlers that mutate skill paths.

use serde_json::{Value, json};

use super::super::state::AdminState;
use crate::gateway::aggregator::skill_mgmt_dispatch;
use crate::gateway::capability::RefreshReason;
use crate::gateway::capability_service::refresh_all_live_backends;

/// The backend tool that makes an app re-scan its skill folders. Loaded skills
/// stay loaded; a loaded skill whose files changed is reloaded.
const RELOAD_SKILLS_TOOL: &str = "dcc_admin__reload_skills";

/// Run the optional disk-catalog reload callback, then refresh live backends.
///
/// After a change to skill folders (`ToolsListChanged`: a path added or
/// removed, a package installed) the open apps first re-scan their folders;
/// they read them only at startup otherwise. Returns their per-instance
/// results in that case: `{"instances": [{instance_id, dcc_type, result | error}]}`.
pub async fn reload_skill_paths_and_refresh_backends(
    state: &AdminState,
    reason: RefreshReason,
) -> Option<Value> {
    if let Some(cb) = state.skill_paths_reload.clone() {
        cb();
    }
    let reloaded = if matches!(reason, RefreshReason::ToolsListChanged) {
        let (text, is_error) =
            skill_mgmt_dispatch(&state.gateway, RELOAD_SKILLS_TOOL, &json!({})).await;
        Some(match serde_json::from_str::<Value>(&text) {
            Ok(value) if !is_error => value,
            // No open app, or the fan-out could not start.
            _ => json!({"instances": [], "message": text}),
        })
    } else {
        None
    };
    refresh_all_live_backends(&state.gateway, reason).await;
    reloaded
}
