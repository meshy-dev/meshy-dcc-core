use serde_json::{Value, json};

use super::admin::trace::AgentContext;

/// Per-call hints may supplement protocol metadata, but cannot replace the
/// caller context established at ingress or extend its remaining budget.
pub(crate) fn merge_call_meta(protocol: Option<&Value>, call: Option<&Value>) -> Option<Value> {
    let mut merged = protocol
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if let Some(call) = call.and_then(Value::as_object) {
        for (key, value) in call {
            if !matches!(
                key.as_str(),
                "agent_context"
                    | "agentContext"
                    | "agent"
                    | "caller_context"
                    | "callerContext"
                    | "meshy_dcc"
                    | meshy_dcc_jsonrpc::TIMEOUT_MS_META_KEY
            ) {
                merged.insert(key.clone(), value.clone());
            }
        }
    }
    if let Some(timeout) = [protocol, call]
        .into_iter()
        .filter_map(meshy_dcc_jsonrpc::timeout_ms_from_meta)
        .min()
    {
        merged.insert(
            meshy_dcc_jsonrpc::TIMEOUT_MS_META_KEY.into(),
            timeout.into(),
        );
    }
    (!merged.is_empty()).then_some(Value::Object(merged))
}

/// Age a relative budget without clocks, overflow, or resetting it on a route
/// refresh. Zero remains an expired budget, rather than becoming unbounded.
pub(crate) fn meta_after_elapsed(
    meta: Option<Value>,
    elapsed: std::time::Duration,
) -> Option<Value> {
    match meshy_dcc_jsonrpc::timeout_ms_from_meta(meta.as_ref()) {
        Some(timeout) => meshy_dcc_jsonrpc::meta_with_timeout_ms(
            meta,
            timeout.saturating_sub(u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)),
        ),
        None => meta,
    }
}

/// Allowed top-level keys in client-supplied `_meta` that may be passed
/// through to adapter tool params. `agent_context` is always server-derived
/// and is appended separately.
const DEFAULT_META_ALLOWLIST: &[&str] = &[
    "credential_profile",
    "permission_hint",
    "project_scope",
    "search_id",
];

/// Filter a client-supplied `_meta` object to only contain allowlisted keys.
/// Prevents inline secrets or arbitrary client data from piggybacking on the
/// meta passthrough channel.
fn bounded_meta(meta: Value) -> Value {
    match meta {
        Value::Object(map) => {
            let filtered: serde_json::Map<String, Value> = map
                .into_iter()
                .filter(|(k, _)| DEFAULT_META_ALLOWLIST.contains(&k.as_str()))
                .collect();
            Value::Object(filtered)
        }
        _ => Value::Object(serde_json::Map::new()),
    }
}

pub(crate) fn meta_with_agent_context(
    meta: Option<Value>,
    agent_context: Option<&AgentContext>,
) -> Option<Value> {
    let Some(agent_context) = agent_context else {
        return meta.map(bounded_meta);
    };
    let agent_context_value = serde_json::to_value(agent_context).ok()?;
    match meta {
        Some(m) => {
            let mut filtered = bounded_meta(m);
            if let Value::Object(ref mut map) = filtered {
                map.insert("agent_context".to_string(), agent_context_value);
            }
            Some(filtered)
        }
        None => Some(json!({ "agent_context": agent_context_value })),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn call_meta_merges_hints_without_replacing_protocol_context_or_budget() {
        let protocol = json!({"agent_context": {"agent_id": "trusted"},
            "search_id": "search-1", "meshy-dcc/timeout_ms": 1000});
        for call in [
            json!(null),
            json!([]),
            json!({"meshy-dcc/timeout_ms": null}),
            json!({"meshy-dcc/timeout_ms": 10000}),
            json!({"meshy-dcc/timeout_ms": -1}),
        ] {
            let meta = merge_call_meta(Some(&protocol), Some(&call)).unwrap();
            assert_eq!(meta["agent_context"]["agent_id"], "trusted");
            assert_eq!(meta["meshy-dcc/timeout_ms"], 1000);
        }
        let meta = merge_call_meta(
            Some(&protocol),
            Some(&json!({
                "agent_context": null, "agent": {"agent_id": "fake"},
                "callerContext": {}, "meshy_dcc": {"agent_context": {}},
                "lease_owner": "owner", "permission_hint": "read", "meshy-dcc/timeout_ms": 0,
            })),
        )
        .unwrap();
        assert_eq!(meta["agent_context"]["agent_id"], "trusted");
        assert!(meta.get("agent").is_none());
        assert!(meta.get("callerContext").is_none());
        assert!(meta.get("meshy_dcc").is_none());
        assert_eq!(meta["lease_owner"], "owner");
        assert_eq!(meta["search_id"], "search-1");
        assert_eq!(meta["meshy-dcc/timeout_ms"], 0);
    }

    #[test]
    fn relative_budgets_saturate_and_can_be_narrowed_by_a_call() {
        let meta = merge_call_meta(None, Some(&json!({"meshy-dcc/timeout_ms": 1000}))).unwrap();
        let expired = meta_after_elapsed(Some(meta), std::time::Duration::from_secs(2)).unwrap();
        assert_eq!(expired["meshy-dcc/timeout_ms"], 0);
        let huge = json!({"meshy-dcc/timeout_ms": u64::MAX});
        assert_eq!(
            meta_after_elapsed(Some(huge), std::time::Duration::from_millis(1)).unwrap()["meshy-dcc/timeout_ms"],
            u64::MAX - 1
        );
    }

    #[test]
    fn bounded_meta_strips_unknown_fields() {
        let filtered = bounded_meta(json!({
            "credential_profile": "prod",
            "permission_hint": "read-only",
            "project_scope": "movie-42",
            "secret_token": "should-be-stripped",
            "inline_secret": "also-stripped",
        }));
        let obj = filtered.as_object().unwrap();
        assert!(obj.contains_key("credential_profile"));
        assert!(obj.contains_key("permission_hint"));
        assert!(obj.contains_key("project_scope"));
        assert!(!obj.contains_key("secret_token"));
        assert!(!obj.contains_key("inline_secret"));
    }

    #[test]
    fn bounded_meta_preserves_search_id() {
        let filtered = bounded_meta(json!({"search_id": "abc-123", "evil": "no"}));
        let obj = filtered.as_object().unwrap();
        assert_eq!(obj["search_id"], "abc-123");
        assert!(!obj.contains_key("evil"));
    }

    #[test]
    fn bounded_meta_drops_non_object_meta() {
        let filtered = bounded_meta(json!(["not", "an", "object"]));
        let obj = filtered.as_object().unwrap();
        assert!(obj.is_empty());
    }

    #[test]
    fn meta_with_agent_context_client_agent_context_is_replaced_by_server() {
        let agent_ctx = AgentContext {
            actor_id: Some("server-artist".to_string()),
            session_id: Some("s1".into()),
            ..AgentContext::default()
        };
        let merged = meta_with_agent_context(
            Some(json!({
                "agent_context": {"actor_id": "fake-client"},
                "credential_profile": "prod",
            })),
            Some(&agent_ctx),
        )
        .expect("merged meta");

        assert_eq!(merged["agent_context"]["actor_id"], "server-artist");
        assert_eq!(merged["agent_context"]["session_id"], "s1");
        assert_eq!(merged["credential_profile"], "prod");
    }

    #[test]
    fn meta_with_agent_context_no_meta_is_backward_compatible() {
        assert!(meta_with_agent_context(None, None).is_none());
    }

    #[test]
    fn meta_with_agent_context_only_agent_context_no_client_meta() {
        let agent_ctx = AgentContext {
            actor_id: Some("a1".to_string()),
            ..AgentContext::default()
        };
        let merged = meta_with_agent_context(None, Some(&agent_ctx)).expect("merged");
        assert_eq!(merged["agent_context"]["actor_id"], "a1");
        assert!(merged.get("credential_profile").is_none());
    }

    #[test]
    fn meta_with_agent_context_drops_non_object_client_meta() {
        let agent_ctx = AgentContext {
            actor_id: Some("server-artist".to_string()),
            ..AgentContext::default()
        };
        let merged = meta_with_agent_context(Some(json!("inline-secret")), Some(&agent_ctx))
            .expect("merged");

        assert_eq!(merged["agent_context"]["actor_id"], "server-artist");
        assert!(merged.get("upstream_meta").is_none());
    }
}
