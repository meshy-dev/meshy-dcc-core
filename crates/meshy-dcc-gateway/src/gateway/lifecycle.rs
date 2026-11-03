//! Bounded native lifecycle surface; backend execution retains its four tools.

use meshy_dcc_lifecycle::application::lifecycle::{self, CallerContext, LifecycleRequest};
use serde_json::{Value, json};
use std::time::Duration;

use super::admin::trace::AgentContext;
use super::state::GatewayState;

pub fn tool_definition() -> Value {
    json!({
        "name":"lifecycle",
        "description":"Configure native projects and open, reuse or safely stop Blender, Godot, Unity and other DCC applications through Core. Use configure_project for an existing native project after its adapter has been prepared, then start with the same project. Each mutation requires a stable request_id and promptly returns an operation_id; query operation (also by original request_id after a lost reply) or wait. Keep the original handle; a timeout never authorizes replay. software and active_work are observations. stop requires the original owned launch operation_id and a new request_id; it never discards unsaved work. Use search/describe/load_skill/call for actual host work and readback.",
        "inputSchema":{
            "type":"object","additionalProperties":false,"required":["action"],
            "properties":{
                "action":{"type":"string","enum":["software","configure_project","start","operation","wait","stop","active_work"]},
                "dcc_type":{"type":"string"},
                "project":{"type":"string","description":"Absolute existing native project directory; configure_project and start require it."},
                "dcc_path":{"type":"string","description":"Optional executable for configure_project only; the adapter must honor the selection, while start uses its published launch plan."},
                "request_id":{"type":"string","maxLength":128,"description":"Stable identifier for one configure_project, start or stop; reuse only with identical arguments."},
                "operation_id":{"type":"string","description":"Original handle for operation/wait, or the owned launch handle for stop."},
                "instance_id":{"type":"string","description":"Optional exact instance UUID to narrow start reuse or validate stop."},
                "timeout_ms":{"type":"integer","minimum":1,"maximum":900000,"description":"Operation deadline; wait caps its own transport wait at 30 seconds. Expiry does not guarantee stopping the host."},
                "agent_name":{"type":"string","description":"Optional exact caller attribution filter for active_work; never grants authority."},
                "session_id":{"type":"string"},"turn_id":{"type":"string"},
                "discard_unsaved":{"type":"boolean","description":"True is refused; explicit operator confirmation uses the CLI stop workflow."}
            }
        },
        "annotations":{"readOnlyHint":false,"destructiveHint":false,"idempotentHint":false,"openWorldHint":true}
    })
}

fn caller(context: Option<&AgentContext>, session: &str) -> CallerContext {
    let raw = context
        .and_then(|c| serde_json::to_value(c).ok())
        .unwrap_or_else(|| json!({}));
    let mut origin = serde_json::Map::new();
    for key in [
        "actor_id",
        "auth_subject",
        "agent_id",
        "agent_name",
        "agent_version",
        "session_id",
        "turn_id",
        "metadata",
    ] {
        if let Some(value) = raw.get(key) {
            origin.insert(key.into(), value.clone());
        }
    }
    let mut identity = json!({"actor_id":raw.get("actor_id"),"auth_subject":raw.get("auth_subject"),"agent_id":raw.get("agent_id"),"session_id":raw.get("session_id")});
    if raw
        .get("session_id")
        .and_then(Value::as_str)
        .is_none_or(str::is_empty)
    {
        identity["mcp_session"] = json!(session);
    }
    CallerContext {
        owner: identity.to_string(),
        agent_context: Value::Object(origin),
    }
}

pub async fn dispatch_value(
    gs: &GatewayState,
    args: &Value,
    meta: Option<&Value>,
    session: &str,
    context: Option<&AgentContext>,
) -> (Value, bool) {
    let result = async {
        anyhow::ensure!(
            gs.own_host == "127.0.0.1" || gs.own_host == "localhost" || gs.own_host == "::1",
            "lifecycle management requires a loopback-only gateway"
        );
        let budget = meshy_dcc_jsonrpc::timeout_ms_from_meta(meta);
        anyhow::ensure!(
            budget != Some(0),
            "request deadline expired; no lifecycle operation was admitted"
        );
        let request: LifecycleRequest = serde_json::from_value(args.clone())?;
        let registry = gs.registry.registry_dir();
        if matches!(
            request.action.as_str(),
            "configure_project" | "start" | "stop"
        ) {
            anyhow::ensure!(
                !gs.policy.read_only,
                "gateway is read-only; lifecycle mutation was not admitted"
            );
            let target_dcc = if request.action == "stop" {
                lifecycle::query_operation(registry, &request, &caller(context, session))?
                    .get("dcc_type")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            } else {
                request.dcc_type.clone()
            };
            anyhow::ensure!(
                target_dcc
                    .as_deref()
                    .is_some_and(|d| gs.policy.allows_dcc(d)),
                "DCC type is not allowed by gateway policy"
            );
            anyhow::ensure!(
                gs.policy
                    .allows_tool_slug(&format!("lifecycle.{}", request.action)),
                "lifecycle action is not allowed by gateway policy"
            );
        }

        match request.action.as_str() {
            "software" => meshy_dcc_lifecycle::application::install::observe_software(
                lifecycle::configured_catalog().as_deref(),
            ),
            "active_work" => Ok(scoped_activity(
                gs,
                &meshy_dcc_models::CallerFilter {
                    agent_name: request.agent_name,
                    session_id: request.session_id,
                    turn_id: request.turn_id,
                },
            )
            .await),
            "operation" => {
                lifecycle::query_operation(registry, &request, &caller(context, session))
            }
            "wait" => {
                let maximum = request
                    .timeout_ms
                    .unwrap_or(1000)
                    .min(budget.unwrap_or(30_000))
                    .min(30_000);
                lifecycle::wait_operation(
                    registry,
                    &request,
                    &caller(context, session),
                    Duration::from_millis(maximum),
                )
                .await
            }
            _ => lifecycle::submit_before(
                registry,
                request,
                caller(context, session),
                budget.map(|ms| std::time::Instant::now() + Duration::from_millis(ms)),
            ),
        }
    }
    .await;
    match result {
        Ok(value) => (value, false),
        Err(error) => (
            json!({"schema_version":1,"error":error.to_string(),"not_admitted":true}),
            true,
        ),
    }
}

async fn scoped_activity(gs: &GatewayState, scope: &meshy_dcc_models::CallerFilter) -> Value {
    let (pending, urls, unattributed_urls) = {
        let calls = gs.pending_calls.read().await;
        let rows: Vec<_> = calls.values().filter(|c| c.work).collect();
        let matched: Vec<_> = rows
            .iter()
            .filter(|c| scope.matches(c.agent_context.as_ref()))
            .collect();
        (
            matched.len(),
            matched
                .iter()
                .map(|c| c.backend_url.clone())
                .collect::<Vec<_>>(),
            rows.iter()
                .filter(|c| !scope.attributable(c.agent_context.as_ref()))
                .map(|c| c.backend_url.clone())
                .collect::<Vec<_>>(),
        )
    };
    let mut result = lifecycle::active_work_scoped(
        gs.registry.registry_dir(),
        Some(pending),
        scope,
        &urls,
        &unattributed_urls,
    )
    .await;
    result["unattributed_calls"] = json!(unattributed_urls.len());
    result
}

pub async fn activity_endpoint(
    axum::extract::State(gs): axum::extract::State<GatewayState>,
    axum::extract::Query(scope): axum::extract::Query<meshy_dcc_models::CallerFilter>,
) -> axum::Json<Value> {
    axum::Json(scoped_activity(&gs, &scope).await)
}

pub async fn dispatch(
    gs: &GatewayState,
    args: &Value,
    meta: Option<&Value>,
    session: &str,
    context: Option<&AgentContext>,
) -> (String, bool) {
    let (value, failed) = dispatch_value(gs, args, meta, session, context).await;
    (value.to_string(), failed)
}
