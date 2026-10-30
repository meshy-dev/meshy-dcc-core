use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use meshy_dcc_host_rpc::HostRpcError;
use meshy_dcc_jsonrpc::negotiate_protocol_version;
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

use super::feedback::FEEDBACK_TOOL_NAME;
use super::trace::trace_context_from_headers;
use super::{SIDECAR_SERVER_NAME, SidecarMcpState};

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: Option<String>,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct ToolsCallParams {
    name: String,
    #[serde(default)]
    arguments: Value,
    #[serde(rename = "_meta", default)]
    meta: Option<Value>,
}

pub(super) async fn handle_health(State(state): State<SidecarMcpState>) -> Response {
    (
        StatusCode::OK,
        axum::Json(json!({"ok": true, "job_persistence": state.job_persistence})),
    )
        .into_response()
}

pub(super) async fn handle_healthz() -> Response {
    (StatusCode::OK, "ok").into_response()
}

pub(super) async fn handle_v1_healthz() -> Response {
    (StatusCode::OK, axum::Json(json!({"ok": true}))).into_response()
}

pub(super) async fn handle_v1_readyz(State(state): State<SidecarMcpState>) -> Response {
    let dispatcher_ready = match state.host_rpc.try_lock() {
        Ok(guard) => guard.is_alive(),
        // A locked dispatcher is busy serving a call, not unavailable. Keep
        // readiness probes non-blocking so long DCC calls do not look like a
        // dead sidecar.
        Err(_) => true,
    };
    let status = if dispatcher_ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        axum::Json(json!({
            "process": true,
            "dispatcher": dispatcher_ready,
            "dcc": dispatcher_ready,
        })),
    )
        .into_response()
}

pub(super) async fn handle_mcp_post(
    State(state): State<SidecarMcpState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let session_id = headers
        .get("Mcp-Session-Id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("sc-{}", uuid::Uuid::new_v4().simple()));

    let value: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => return parse_error(&session_id, format!("parse error: {e}")),
    };

    let req: JsonRpcRequest = match serde_json::from_value(value) {
        Ok(r) => r,
        Err(e) => return parse_error(&session_id, format!("not a JSON-RPC request: {e}")),
    };

    // Notifications have no id - we accept and discard.
    if req.id.is_none() {
        return (StatusCode::ACCEPTED).into_response();
    }

    let id = req.id.clone().unwrap_or(Value::Null);
    let body = dispatch(&state, &headers, &req, id).await;
    let mut response = axum::Json(body).into_response();
    attach_session(&mut response, &session_id);
    response
}

async fn dispatch(
    state: &SidecarMcpState,
    headers: &HeaderMap,
    req: &JsonRpcRequest,
    id: Value,
) -> Value {
    match req.method.as_str() {
        "initialize" => initialize_response(id, &state.server_version, req),
        "ping" => json!({"jsonrpc": "2.0", "id": id, "result": {}}),
        "tools/call" => handle_tools_call(state, headers, id, req).await,
        other => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": -32601,
                "message": format!("method not found: {other:?}"),
                "data": {
                    "supported": ["initialize", "ping", "tools/call"],
                    "note": "sidecar serves dispatch only; use the gateway for discovery"
                }
            }
        }),
    }
}

fn initialize_response(id: Value, server_version: &str, req: &JsonRpcRequest) -> Value {
    let requested_protocol_version = req
        .params
        .as_ref()
        .and_then(|params| params.get("protocolVersion"))
        .and_then(Value::as_str);
    let protocol_version = negotiate_protocol_version(requested_protocol_version);
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "protocolVersion": protocol_version,
            "capabilities": {
                "tools": {"listChanged": false}
            },
            "serverInfo": {
                "name": SIDECAR_SERVER_NAME,
                "version": server_version
            },
            "instructions": "meshy-dcc-server sidecar — dispatches tools/call to a single DCC instance via its native RPC channel. Discovery happens at the gateway."
        }
    })
}

async fn handle_tools_call(
    state: &SidecarMcpState,
    headers: &HeaderMap,
    id: Value,
    req: &JsonRpcRequest,
) -> Value {
    let started = Instant::now();
    let params: ToolsCallParams = match req
        .params
        .clone()
        .map(serde_json::from_value::<ToolsCallParams>)
        .transpose()
    {
        Ok(Some(p)) => p,
        Ok(None) => {
            return json!({
                "jsonrpc": "2.0", "id": id,
                "error": {"code": -32602, "message": "tools/call requires params"}
            });
        }
        Err(e) => {
            return json!({
                "jsonrpc": "2.0", "id": id,
                "error": {"code": -32602, "message": format!("invalid params: {e}")}
            });
        }
    };

    // Use the JSON-RPC `id` (stringified) as the request_id the
    // HostRpcClient propagates to the DCC. The DCC echoes it back
    // in the result envelope so async correlation works end-to-end.
    let request_id = match &id {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };

    if params.name == FEEDBACK_TOOL_NAME {
        let payload = match state.feedback.as_ref() {
            Some(feedback) => feedback.forward(params.arguments).await,
            None => json!({
                "success": false,
                "message": "Gateway feedback endpoint is unavailable.",
                "error": "gateway_feedback_unavailable",
            }),
        };
        return json!({
            "jsonrpc": "2.0", "id": id,
            "result": payload
        });
    }

    let budget =
        meshy_dcc_jsonrpc::timeout_ms_from_meta(params.meta.as_ref()).map(Duration::from_millis);
    let remaining = || budget.map(|budget| budget.saturating_sub(started.elapsed()));
    let lock = state.host_rpc.clone().lock_owned();
    let guard = match remaining() {
        Some(left) => match tokio::time::timeout(left, lock).await {
            Ok(guard) => guard,
            Err(_) => return deadline_error(id, false),
        },
        None => lock.await,
    };
    let trace_context = trace_context_from_headers(headers, &request_id);
    let task_id = id.clone();
    // Retain the RPC and its lock until its correlated reply is consumed,
    // even if the HTTP caller stops waiting. Dropping a stream read mid-call
    // could let the next request consume this call's delayed response.
    let task = tokio::spawn(async move {
        let mut meta = params.meta;
        if let Some(budget) = budget {
            let left = budget.saturating_sub(started.elapsed());
            let host_ms = u64::try_from(left.as_millis())
                .unwrap_or(u64::MAX)
                .saturating_sub(meshy_dcc_jsonrpc::DEADLINE_HOP_MARGIN_MS);
            if host_ms == 0 {
                return deadline_error(task_id, false);
            }
            meta = meshy_dcc_jsonrpc::meta_with_timeout_ms(meta, host_ms);
        }
        match guard
            .call_with_metadata(
                &params.name,
                params.arguments,
                &request_id,
                trace_context,
                meta,
            )
            .await
        {
            Ok(payload) => json!({"jsonrpc": "2.0", "id": task_id, "result": payload}),
            Err(err) => host_rpc_error_to_jsonrpc(task_id, err),
        }
    });
    let result = match remaining() {
        Some(left) => match tokio::time::timeout(left, task).await {
            Ok(result) => result,
            Err(_) => return deadline_error(id, true),
        },
        None => task.await,
    };
    match result {
        Ok(response) => response,
        Err(error) => host_rpc_error_to_jsonrpc(id, HostRpcError::transport(error.to_string())),
    }
}

fn deadline_error(id: Value, dispatched: bool) -> Value {
    let (kind, message) = if dispatched {
        (
            "outcome-unknown",
            "The host did not answer before the deadline and may still run the call. Read the scene back before repeating it.",
        )
    } else {
        (
            "not-applied",
            "The deadline passed before dispatch. The call was not sent to the host and is safe to retry.",
        )
    };
    json!({
        "jsonrpc": "2.0", "id": id,
        "error": {"code": -32000, "message": message,
            "data": {"kind": kind, "message": message, "retryable": !dispatched}},
    })
}

fn host_rpc_error_to_jsonrpc(id: Value, err: HostRpcError) -> Value {
    let (code, message, data) = match err {
        HostRpcError::HostDied {
            last_call_slug,
            last_call_args,
        } => (
            -32000,
            "host-died".to_string(),
            json!({
                "kind": "host-died",
                "last_call_slug": last_call_slug,
                "last_call_args": last_call_args,
                "guidance": "the DCC process disconnected mid-call; the gateway will evict this backend"
            }),
        ),
        HostRpcError::TransportError { message } => (
            -32000,
            "transport-error".to_string(),
            json!({"kind": "transport-error", "message": message}),
        ),
        HostRpcError::Timeout {} => return deadline_error(id, true),
        HostRpcError::Cancelled {} => (
            -32000,
            "cancelled".to_string(),
            json!({"kind": "cancelled"}),
        ),
        HostRpcError::BackendError { envelope } => (
            -32000,
            "backend-error".to_string(),
            json!({"kind": "backend-error", "envelope": envelope}),
        ),
    };

    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {"code": code, "message": message, "data": data}
    })
}

fn parse_error(session_id: &str, message: String) -> Response {
    let body = json!({
        "jsonrpc": "2.0",
        "id": null,
        "error": {"code": -32700, "message": message}
    });
    let mut response = axum::Json(body).into_response();
    attach_session(&mut response, session_id);
    response
}

fn attach_session(response: &mut Response, session_id: &str) {
    if let Ok(value) = HeaderValue::from_str(session_id) {
        response.headers_mut().insert("Mcp-Session-Id", value);
    }
}
