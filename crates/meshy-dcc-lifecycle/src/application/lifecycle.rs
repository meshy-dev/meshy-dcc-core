//! Caller-scoped admission on the existing lifecycle store. There is no second
//! scheduler or job database: accepted work uses the same launch records as CLI.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::instance_launch::{self, LifecycleStore};
use crate::domain::rest::StopInstanceRequest;
use crate::domain::start_instance::{
    Invocation, LifecycleOperation, ManagedOperation, StartInstanceRequest,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleRequest {
    pub action: String,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub operation_id: Option<String>,
    #[serde(default)]
    pub dcc_type: Option<String>,
    #[serde(default)]
    pub project: Option<PathBuf>,
    #[serde(default)]
    pub dcc_path: Option<PathBuf>,
    #[serde(default)]
    pub instance_id: Option<String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Always refused over MCP. Explicit operator confirmation stays on CLI.
    #[serde(default)]
    pub discard_unsaved: bool,
    #[serde(default)]
    pub agent_name: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub turn_id: Option<String>,
}

/// Identity is supplied by the protocol ingress, never by tool arguments.
#[derive(Debug, Clone)]
pub struct CallerContext {
    pub owner: String,
    pub agent_context: Value,
}

pub fn configured_catalog() -> Option<PathBuf> {
    std::env::var_os("MESHY_DCC_CATALOG_PATH")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn operation_id(owner: &str, request_id: &str) -> String {
    uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_OID,
        format!("{owner}\0{request_id}").as_bytes(),
    )
    .to_string()
}

fn list_operations(registry: &Path) -> anyhow::Result<Vec<LifecycleOperation>> {
    let dir = registry.join("start-instance");
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let store = LifecycleStore::new(registry);
    let mut result = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if path.extension().is_none_or(|e| e != "json") || uuid::Uuid::parse_str(id).is_err() {
            continue;
        }
        if let Some(operation) = store.load(id)? {
            result.push(operation);
        }
    }
    Ok(result)
}

/// Derived associations from actual lifecycle execution, never a second binding
/// registry and never ownership evidence for a reused external process.
pub fn instance_associations(
    registry: &Path,
    entry: &meshy_dcc_transport::discovery::types::ServiceEntry,
) -> Vec<Value> {
    let Some(project) = instance_launch::entry_project(entry) else {
        return Vec::new();
    };
    list_operations(registry).unwrap_or_default().into_iter().filter_map(|operation| {
        let managed = operation.management.as_ref()?;
        let same_project = operation.project.canonicalize().ok()? == project.canonicalize().ok()?;
        (managed.action == "start" && managed.state == "completed"
            && managed.result.as_ref()?.get("ok") == Some(&Value::Bool(true))
            && operation.instance_id.as_deref() == Some(entry.instance_id.to_string().as_str())
            && operation.pid.is_some_and(|pid| entry.host_pid == Some(pid) || entry.pid == Some(pid))
            && operation.dcc_type.eq_ignore_ascii_case(&entry.dcc_type) && same_project)
            .then(|| json!({"operation_id":operation.operation_id,"source":"lifecycle","agent_context":managed.agent_context}))
    }).collect()
}

/// A CLI inspection never starts a gateway. Without its pending-call evidence,
/// direct host/operation observations cannot establish global idleness.
pub async fn inspect_active_work(
    registry: &Path,
    gateway_base: &str,
    scope: &meshy_dcc_models::CallerFilter,
) -> Value {
    let url = format!("{}/v1/activity", gateway_base.trim_end_matches('/'));
    let mut parsed = reqwest::Url::parse(&url);
    if let Ok(url) = &mut parsed {
        for (key, value) in [
            ("agent_name", &scope.agent_name),
            ("session_id", &scope.session_id),
            ("turn_id", &scope.turn_id),
        ] {
            if let Some(value) = value {
                url.query_pairs_mut().append_pair(key, value);
            }
        }
    }
    if parsed
        .as_ref()
        .ok()
        .is_some_and(|u| matches!(u.host_str(), Some("127.0.0.1" | "localhost" | "::1")))
    {
        let result = async {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .build()?;
            let response = client
                .get(parsed.as_ref().unwrap().clone())
                .send()
                .await?
                .error_for_status()?;
            let bytes = response.bytes().await?;
            anyhow::ensure!(bytes.len() <= 1024 * 1024, "activity response too large");
            let value: Value = serde_json::from_slice(&bytes)?;
            anyhow::ensure!(
                value["schema_version"] == 1
                    && matches!(value["state"].as_str(), Some("idle" | "busy" | "unknown"))
                    && value["scope"] == serde_json::to_value(scope)?,
                "unsupported activity response"
            );
            Ok::<_, anyhow::Error>(value)
        }
        .await;
        if let Ok(value) = result {
            return value;
        }
    }
    active_work_scoped(registry, None, scope, &[], &[]).await
}

fn view(operation: &LifecycleOperation, registry: &Path) -> Value {
    let Some(managed) = operation.management.as_ref() else {
        return json!({"schema_version":1,"operation_id":operation.operation_id,
            "state":"legacy","result":instance_launch::operation_summary(operation)});
    };
    let state = if matches!(managed.state.as_str(), "accepted" | "running")
        && !meshy_dcc_gateway_ensure::is_process_alive(managed.controller_pid)
    {
        "outcome_unknown"
    } else {
        &managed.state
    };
    let mut output = json!({"schema_version":1,"operation_id":operation.operation_id,
        "request_id":managed.request_id,"action":managed.action,"state":state,
        "dcc_type":operation.dcc_type,"project":operation.project,
        "agent_context":managed.agent_context,
        "stopping":operation.instance_id.as_deref().and_then(|id| uuid::Uuid::parse_str(id).ok())
            .is_some_and(|id| LifecycleStore::new(registry).is_stopping(&id)),
    });
    if let Some(result) = &managed.result {
        output["result"] = result.clone();
    }
    if let Some(error) = &managed.error {
        output["error"] = json!(error);
    }
    output
}

pub fn query_operation(
    registry: &Path,
    request: &LifecycleRequest,
    caller: &CallerContext,
) -> anyhow::Result<Value> {
    let id = match &request.operation_id {
        Some(id) => {
            uuid::Uuid::parse_str(id).context("operation_id must be a UUID")?;
            id.clone()
        }
        None => operation_id(
            &caller.owner,
            request
                .request_id
                .as_deref()
                .context("operation_id or original request_id is required")?,
        ),
    };
    let operation = LifecycleStore::new(registry)
        .load(&id)?
        .context("operation not found; do not replay an uncertain request")?;
    anyhow::ensure!(
        operation
            .management
            .as_ref()
            .is_some_and(|m| m.owner == caller.owner),
        "operation belongs to another caller or a legacy CLI launch"
    );
    Ok(view(&operation, registry))
}

fn owned_stop_target(
    registry: &Path,
    target_id: &str,
    owner: &str,
) -> anyhow::Result<LifecycleOperation> {
    let target = instance_launch::resolve_owned_operation(registry, None, None, Some(target_id))?
        .context("launch operation not found")?;
    anyhow::ensure!(
        target.launched
            && target.owned
            && target
                .management
                .as_ref()
                .is_some_and(|m| m.owner == owner && m.action == "start"),
        "this caller does not own the original launch; nothing was stopped"
    );
    let id = target
        .instance_id
        .as_deref()
        .context("launch has not registered an instance")?;
    let entry =
        super::local_instance::select_one_entry(registry, Some(&target.dcc_type), Some(id))?;
    let project = target
        .project
        .canonicalize()
        .context("launch project cannot be verified; nothing was stopped")?;
    anyhow::ensure!(
        entry.instance_id.to_string() == id
            && target.pid.is_some()
            && target.pid == entry.host_pid.or(entry.pid)
            && instance_launch::entry_metadata(
                &entry,
                crate::domain::start_instance::OPERATION_METADATA_KEYS
            ) == Some(target_id)
            && instance_launch::entry_project(&entry).and_then(|p| p.canonicalize().ok())
                == Some(project),
        "live instance no longer matches the original launch PID, operation and project; nothing was stopped"
    );
    Ok(target)
}

/// Persist acceptance before dispatch. Repeating the same request returns its
/// original operation, including after reconnect; changed payloads are refused.
pub fn submit(
    registry: &Path,
    request: LifecycleRequest,
    caller: CallerContext,
) -> anyhow::Result<Value> {
    submit_before(registry, request, caller, None)
}

pub fn submit_before(
    registry: &Path,
    request: LifecycleRequest,
    caller: CallerContext,
    deadline: Option<std::time::Instant>,
) -> anyhow::Result<Value> {
    anyhow::ensure!(
        !request.discard_unsaved,
        "Discarding unsaved work requires explicit operator confirmation through stop-instance --discard-unsaved; nothing was stopped"
    );
    anyhow::ensure!(
        matches!(
            request.action.as_str(),
            "configure_project" | "start" | "stop"
        ),
        "unsupported lifecycle action"
    );
    let request_id = request
        .request_id
        .as_deref()
        .context("a stable request_id is required")?;
    anyhow::ensure!(
        serde_json::to_vec(&caller.agent_context)?.len() <= 16 * 1024,
        "caller context exceeds the lifecycle origin limit"
    );
    anyhow::ensure!(
        !request_id.is_empty()
            && request_id.len() <= 128
            && request_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_:.".contains(&b)),
        "invalid request_id"
    );
    anyhow::ensure!(
        request.timeout_ms != Some(0) && request.timeout_ms.is_none_or(|ms| ms <= 900_000),
        "timeout_ms must be between 1 and 900000"
    );
    let id = operation_id(&caller.owner, request_id);
    let digest = Sha256::digest(serde_json::to_vec(&request)?)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let store = LifecycleStore::new(registry);
    std::fs::create_dir_all(store.dir())?;
    let admission = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(store.dir().join(".admission.lock"))?;
    let contention_deadline = std::time::Instant::now() + Duration::from_millis(100);
    let contention_deadline = deadline.map_or(contention_deadline, |d| d.min(contention_deadline));
    loop {
        match admission.try_lock() {
            Ok(()) => break,
            Err(std::fs::TryLockError::WouldBlock)
                if std::time::Instant::now() < contention_deadline =>
            {
                // Admission only persists a small receipt. Bound contention so
                // another caller cannot hold an MCP request indefinitely.
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(error) => {
                return Err(anyhow::Error::new(error)).context(
                    "lifecycle admission is busy; reconcile or retry the same request_id",
                );
            }
        }
    }
    anyhow::ensure!(
        deadline.is_none_or(|d| std::time::Instant::now() < d),
        "request deadline expired before admission; nothing was dispatched"
    );
    if let Some(old) = store.load(&id)? {
        anyhow::ensure!(
            old.management
                .as_ref()
                .is_some_and(|m| m.owner == caller.owner && m.payload_digest == digest),
            "request_id was already used with different arguments"
        );
        return Ok(view(&old, registry));
    }
    let (dcc, project) = if request.action == "stop" {
        let target_id = request
            .operation_id
            .as_deref()
            .context("stop requires the original launch operation_id")?;
        uuid::Uuid::parse_str(target_id)?;
        let target = owned_stop_target(registry, target_id, &caller.owner)?;
        if let Some(id) = &request.instance_id {
            anyhow::ensure!(
                target.instance_id.as_ref() == Some(id),
                "instance_id does not match the launch"
            );
        }
        (target.dcc_type, target.project)
    } else {
        let dcc = request
            .dcc_type
            .as_deref()
            .context("dcc_type is required")?
            .trim()
            .to_ascii_lowercase();
        anyhow::ensure!(
            !dcc.is_empty()
                && dcc.len() <= 128
                && dcc
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b)),
            "invalid dcc_type"
        );
        let project = request.project.as_deref().context("project is required")?;
        anyhow::ensure!(
            project.is_absolute() && project.is_dir(),
            "project must be an existing absolute directory; create the native project first"
        );
        (dcc, project.canonicalize()?)
    };
    let mut operation = LifecycleOperation::new(&id, &dcc, &project, Path::new(""), None);
    operation.management = Some(ManagedOperation {
        request_id: request_id.into(),
        action: request.action.clone(),
        owner: caller.owner,
        payload_digest: digest,
        state: "accepted".into(),
        controller_pid: std::process::id(),
        agent_context: caller.agent_context,
        result: None,
        error: None,
    });
    store.save(&operation)?;
    let accepted = view(&operation, registry);
    drop(admission);
    let registry = registry.to_path_buf();
    tokio::spawn(async move {
        execute(registry, operation, request).await;
    });
    Ok(accepted)
}

fn update(
    registry: &Path,
    id: &str,
    state: &str,
    result: Option<Value>,
    error: Option<String>,
) -> anyhow::Result<()> {
    let store = LifecycleStore::new(registry);
    let mut operation = store.load(id)?.context("accepted operation was removed")?;
    let managed = operation.management.as_mut().context("missing admission")?;
    managed.state = state.into();
    managed.result = result;
    managed.error = error;
    operation.touch();
    store.save(&operation)
}

async fn execute(registry: PathBuf, operation: LifecycleOperation, request: LifecycleRequest) {
    let id = operation.operation_id.clone();
    if update(&registry, &id, "running", None, None).is_err() {
        return;
    }
    let work_registry = registry.clone();
    let duration = Duration::from_millis(request.timeout_ms.unwrap_or(600_000));
    let mut work = tokio::spawn(async move {
        match request.action.as_str() {
            "configure_project" => {
                let deadline = std::time::Instant::now() + duration;
                let _lock = tokio::time::timeout(
                    duration,
                    instance_launch::project_lock(
                        &work_registry,
                        &operation.dcc_type,
                        &operation.project,
                    ),
                )
                .await
                .context("project operation deadline elapsed while queued")??;
                tokio::task::spawn_blocking(move || {
                    super::install::configure_prepared_project_before(
                        &operation.dcc_type,
                        &operation.project,
                        request.dcc_path,
                        Some(deadline),
                    )
                })
                .await?
            }
            "start" => {
                instance_launch::start_instance_with_id(
                    StartInstanceRequest {
                        dcc_type: operation.dcc_type,
                        project: operation.project,
                        launch_plan: None,
                        version: None,
                        wait_ready: true,
                        timeout: duration,
                        interval: Duration::from_millis(200),
                        required: vec![],
                        authorized: true,
                        dry_run: false,
                        registry_dir: work_registry,
                        instance_id: request.instance_id,
                        invocation: Invocation::default(),
                    },
                    operation.operation_id,
                )
                .await
            }
            "stop" => {
                let target = owned_stop_target(
                    &work_registry,
                    request.operation_id.as_deref().unwrap(),
                    &operation
                        .management
                        .as_ref()
                        .context("missing caller context")?
                        .owner,
                )?;
                let instance_id = target
                    .instance_id
                    .context("launch has not registered an instance")?;
                super::local_control::stop_instance_with_tracking(
                    work_registry,
                    StopInstanceRequest {
                        dcc_type: target.dcc_type,
                        instance_id,
                        expected_owner: None,
                        expected_session: None,
                        operation_id: request.operation_id,
                        discard_unsaved: false,
                    },
                )
                .await
            }
            _ => bail!("unsupported lifecycle action"),
        }
    });
    match tokio::time::timeout(duration, &mut work).await {
        Err(_) => {
            // Dropping a wait does not stop a host or installer. Keep the original
            // task and handle; a later completion can still reconcile the outcome.
            let _ = update(
                &registry,
                &id,
                "outcome_unknown",
                None,
                Some("operation deadline elapsed; query this handle, do not resubmit".into()),
            );
            finish(&registry, &id, work.await);
        }
        Ok(result) => finish(&registry, &id, result),
    }
}

fn finish(
    registry: &Path,
    id: &str,
    result: Result<anyhow::Result<Value>, tokio::task::JoinError>,
) {
    let (state, value, error) = match result {
        Ok(Ok(value)) => {
            let state = if value.get("ok") == Some(&Value::Bool(false)) {
                "failed"
            } else {
                "completed"
            };
            (state, Some(value), None)
        }
        Ok(Err(error)) => ("outcome_unknown", None, Some(error.to_string())),
        Err(error) => ("outcome_unknown", None, Some(error.to_string())),
    };
    let _ = update(registry, id, state, value, error);
}

pub async fn wait_operation(
    registry: &Path,
    request: &LifecycleRequest,
    caller: &CallerContext,
    maximum: Duration,
) -> anyhow::Result<Value> {
    let deadline = tokio::time::Instant::now() + maximum;
    loop {
        let value = query_operation(registry, request, caller)?;
        if !matches!(value["state"].as_str(), Some("accepted" | "running"))
            || tokio::time::Instant::now() >= deadline
        {
            return Ok(value);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Public read-only activity evidence. Missing/legacy backend counters and
/// unreadable operation records remain unknown rather than being called idle.
pub async fn active_work(registry: &Path, pending_calls: Option<usize>) -> Value {
    active_work_scoped(
        registry,
        pending_calls,
        &meshy_dcc_models::CallerFilter::default(),
        &[],
        &[],
    )
    .await
}

pub async fn active_work_scoped(
    registry: &Path,
    pending_calls: Option<usize>,
    scope: &meshy_dcc_models::CallerFilter,
    related_urls: &[String],
    unattributed_call_urls: &[String],
) -> Value {
    let mut unknown = Vec::<String>::new();
    let mut unobserved = Vec::<String>::new();
    let mut operations = Vec::new();
    let mut related_ids = std::collections::HashSet::new();
    let mut unattributed_operations = 0_u64;
    match list_operations(registry) {
        Ok(records) => {
            for operation in records {
                if let Some(managed) = &operation.management {
                    if !scope.matches(Some(&managed.agent_context)) {
                        if matches!(
                            managed.state.as_str(),
                            "accepted" | "running" | "outcome_unknown"
                        ) && !scope.attributable(Some(&managed.agent_context))
                        {
                            unattributed_operations += 1;
                        }
                        continue;
                    }
                    if let Some(id) = &operation.instance_id {
                        related_ids.insert(id.clone());
                    }
                    if matches!(
                        managed.state.as_str(),
                        "accepted" | "running" | "outcome_unknown"
                    ) {
                        let row = view(&operation, registry);
                        if row["state"] == "outcome_unknown" {
                            unknown.push(operation.operation_id.clone());
                        }
                        operations.push(row);
                    }
                } else if operation.launched && operation.blocking_state.is_none() {
                    if scope.is_scoped() {
                        unattributed_operations += 1;
                    } else {
                        unknown.push(operation.operation_id);
                    }
                }
            }
        }
        Err(_) => unknown.push("lifecycle_store".into()),
    }
    let mut pending = 0_u64;
    let mut running = 0_u64;
    let mut unattributed_jobs = 0_u64;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    match super::local_instance::snapshot_dcc_entries(registry) {
        Ok((entries, _)) => {
            for entry in entries {
                let related = !scope.is_scoped()
                    || related_ids.contains(&entry.instance_id.to_string())
                    || related_urls
                        .iter()
                        .any(|u| u == &super::local_instance::mcp_url(&entry));
                if related
                    && unattributed_call_urls
                        .iter()
                        .any(|u| u == &super::local_instance::mcp_url(&entry))
                {
                    unknown.push(entry.instance_id.to_string());
                }
                if !matches!(entry.host.as_str(), "127.0.0.1" | "localhost" | "::1") {
                    if related {
                        unknown.push(entry.instance_id.to_string());
                    } else {
                        unobserved.push(entry.instance_id.to_string());
                    }
                    continue;
                }
                let health = format!(
                    "http://{}:{}/health",
                    if entry.host == "::1" {
                        "[::1]"
                    } else {
                        &entry.host
                    },
                    entry.port
                );
                let observation = async {
                    let mut health = reqwest::Url::parse(&health)?;
                    for (key, value) in [
                        ("agent_name", &scope.agent_name),
                        ("session_id", &scope.session_id),
                        ("turn_id", &scope.turn_id),
                    ] {
                        if let Some(value) = value {
                            health.query_pairs_mut().append_pair(key, value);
                        }
                    }
                    let response = client.get(health).send().await?.error_for_status()?;
                    let bytes = response.bytes().await?;
                    anyhow::ensure!(bytes.len() <= 64 * 1024, "health response too large");
                    let value: Value = serde_json::from_slice(&bytes)?;
                    if scope.is_scoped() {
                        anyhow::ensure!(
                            value.pointer("/activity/scope") == Some(&serde_json::to_value(scope)?),
                            "backend cannot attribute activity"
                        );
                    }
                    let p = value
                        .pointer("/activity/pending_jobs")
                        .and_then(Value::as_u64)
                        .context("missing pending counter")?;
                    let r = value
                        .pointer("/activity/running_jobs")
                        .and_then(Value::as_u64)
                        .context("missing running counter")?;
                    let u = value
                        .pointer("/activity/unattributed_jobs")
                        .and_then(Value::as_u64)
                        .unwrap_or(0);
                    let x = value
                        .pointer("/activity/unknown_jobs")
                        .and_then(Value::as_u64)
                        .unwrap_or(0);
                    Ok::<_, anyhow::Error>((p, r, u, x))
                }
                .await;
                match observation {
                    Ok((p, r, u, x)) => {
                        pending += p;
                        running += r;
                        unattributed_jobs += u;
                        if x > 0 || (related && u > 0) {
                            unknown.push(entry.instance_id.to_string());
                        }
                    }
                    Err(_) if related => unknown.push(entry.instance_id.to_string()),
                    Err(_) => unobserved.push(entry.instance_id.to_string()),
                }
            }
        }
        Err(_) => unknown.push("registry".into()),
    }
    if pending_calls.is_none() {
        unknown.push("gateway_calls".into());
    }
    let busy = pending_calls.unwrap_or_default() > 0
        || pending + running > 0
        || operations
            .iter()
            .any(|v| matches!(v["state"].as_str(), Some("accepted" | "running")));
    json!({"schema_version":1,"state":if busy {"busy"} else if unknown.is_empty() {"idle"} else {"unknown"},
        "scope":scope,"lifecycle_operations":operations,"pending_calls":pending_calls.unwrap_or_default(),
        "jobs":{"pending":pending,"running":running,"unattributed":unattributed_jobs},
        "unknown_instances":unknown,"unobserved_instances":unobserved,"unattributed_operations":unattributed_operations})
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;
