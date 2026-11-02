use super::*;
use meshy_dcc_transport::discovery::file_registry::FileRegistry;
use meshy_dcc_transport::discovery::types::{DispatchStatus, ServiceEntry};

fn caller(owner: &str) -> CallerContext {
    CallerContext {
        owner: owner.into(),
        agent_context: json!({"agent_name":"fixture-client","session_id":owner,"turn_id":"run-1","metadata":{"release":"fixture"}}),
    }
}

fn request(action: &str, project: &Path, request_id: &str) -> LifecycleRequest {
    serde_json::from_value(
        json!({"action":action,"dcc_type":"fixture","project":project,
        "request_id":request_id,"timeout_ms":500}),
    )
    .unwrap()
}

#[tokio::test]
async fn admission_is_durable_idempotent_scoped_and_reconciles_without_dispatch_replay() {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("native project");
    std::fs::create_dir(&project).unwrap();
    let registry = temp.path().join("registry");
    let request = request("start", &project, "stable-1");
    // No launch plan: the worker must fail before process creation, while
    // still retaining the original accepted handle across reconnect/retry.
    let accepted = submit(&registry, request.clone(), caller("session-a")).unwrap();
    assert_eq!(accepted["state"], "accepted");
    assert_eq!(accepted["agent_context"]["turn_id"], "run-1");
    let store = LifecycleStore::new(&registry);
    assert!(
        store
            .load(accepted["operation_id"].as_str().unwrap())
            .unwrap()
            .is_some()
    );
    assert!(store.find_owned("fixture", &project).unwrap().is_none());
    let duplicate = submit(&registry, request.clone(), caller("session-a")).unwrap();
    assert_eq!(duplicate["operation_id"], accepted["operation_id"]);
    let done = wait_operation(
        &registry,
        &request,
        &caller("session-a"),
        Duration::from_secs(2),
    )
    .await
    .unwrap();
    assert_eq!(done["state"], "failed");
    assert_eq!(done["result"]["blocking_state"], "launch_plan_missing");
    let retried = submit(&registry, request.clone(), caller("session-a")).unwrap();
    assert_eq!(retried, done);
    let mut changed = request.clone();
    changed.timeout_ms = Some(600);
    let error = submit(&registry, changed, caller("session-a")).unwrap_err();
    assert!(
        error.to_string().contains("different arguments"),
        "{error:#}"
    );
    let mut query = request.clone();
    query.operation_id = accepted["operation_id"].as_str().map(str::to_owned);
    assert!(query_operation(&registry, &query, &caller("session-b")).is_err());
    let second = submit(&registry, request, caller("session-b")).unwrap();
    assert_ne!(second["operation_id"], accepted["operation_id"]);
}

#[tokio::test]
async fn expired_admission_and_model_discard_do_not_create_operation_records() {
    let temp = tempfile::tempdir().unwrap();
    let registry = temp.path().join("registry");
    let req = request("start", temp.path(), "expired");
    assert!(submit_before(&registry, req, caller("a"), Some(std::time::Instant::now())).is_err());
    assert!(list_operations(&registry).unwrap().is_empty());
    let mut req = request("stop", temp.path(), "discard");
    req.discard_unsaved = true;
    assert!(
        submit(&registry, req, caller("a"))
            .unwrap_err()
            .to_string()
            .contains("explicit operator")
    );
    assert!(list_operations(&registry).unwrap().is_empty());
}

#[tokio::test]
async fn admission_contention_cannot_outlive_the_callers_budget_and_dispatch_later() {
    let temp = tempfile::tempdir().unwrap();
    let registry = temp.path().join("registry");
    let dir = LifecycleStore::new(&registry).dir().to_path_buf();
    std::fs::create_dir_all(&dir).unwrap();
    let held = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join(".admission.lock"))
        .unwrap();
    held.lock().unwrap();
    let result = submit_before(
        &registry,
        request("start", temp.path(), "contended"),
        caller("a"),
        Some(std::time::Instant::now() + Duration::from_millis(5)),
    );
    assert!(result.is_err());
    drop(held);
    assert!(list_operations(&registry).unwrap().is_empty());
}

#[tokio::test]
async fn legacy_unrelated_backend_does_not_poison_scoped_idle_but_relevant_unknown_does() {
    let temp = tempfile::tempdir().unwrap();
    let registry = FileRegistry::new(temp.path()).unwrap();
    let mut entry = ServiceEntry::new("fixture", "127.0.0.1", 9);
    entry.pid = Some(std::process::id());
    entry
        .metadata
        .insert("dispatch_status".into(), DispatchStatus::Ready.to_string());
    registry.register(entry.clone()).unwrap();
    let scope = meshy_dcc_models::CallerFilter {
        agent_name: Some("fixture-client".into()),
        ..Default::default()
    };
    let unrelated = active_work_scoped(temp.path(), Some(0), &scope, &[], &[]).await;
    assert_eq!(unrelated["state"], "idle");
    assert_eq!(
        unrelated["unobserved_instances"].as_array().unwrap().len(),
        1
    );
    let relevant = active_work_scoped(
        temp.path(),
        Some(0),
        &scope,
        &[super::super::local_instance::mcp_url(&entry)],
        &[],
    )
    .await;
    assert_eq!(relevant["state"], "unknown");
    let disconnected = active_work_scoped(temp.path(), None, &scope, &[], &[]).await;
    assert_eq!(disconnected["state"], "unknown");
}

#[cfg(unix)]
#[tokio::test]
async fn reuse_cannot_adopt_another_callers_owned_process_and_projects_stay_exact() {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir_all(project.join(".meshy-dcc")).unwrap();
    std::fs::write(
        project.join(".meshy-dcc/launch-plan.json"),
        json!({"schema_version":1,"dcc_type":"fixture",
        "executable":"/bin/sh","argv":["{executable}","-c","exit 0"],"project_markers":[]})
        .to_string(),
    )
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let router = axum::Router::new().route(
        "/v1/readyz",
        axum::routing::get(|| async {
            axum::Json(json!({"process":true,"dcc":true,"skill_catalog":true,"dispatcher":true}))
        }),
    );
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let registry_path = temp.path().join("registry");
    let registry = FileRegistry::new(&registry_path).unwrap();
    let mut entry = ServiceEntry::new("fixture", "127.0.0.1", port);
    entry.pid = Some(std::process::id());
    entry
        .metadata
        .insert("project".into(), project.to_string_lossy().into_owned());
    entry
        .metadata
        .insert("dispatch_status".into(), DispatchStatus::Ready.to_string());
    registry.register(entry.clone()).unwrap();
    let mut original = LifecycleOperation::new(
        uuid::Uuid::new_v4().to_string(),
        "fixture",
        &project,
        Path::new("/bin/sh"),
        None,
    );
    original.launched = true;
    original.owned = true;
    original.pid = entry.pid;
    original.instance_id = Some(entry.instance_id.to_string());
    original.management = Some(ManagedOperation {
        request_id: "original".into(),
        action: "start".into(),
        owner: "owner-a".into(),
        payload_digest: "fixture".into(),
        state: "completed".into(),
        controller_pid: std::process::id(),
        agent_context: caller("owner-a").agent_context,
        result: Some(json!({"ok":true})),
        error: None,
    });
    LifecycleStore::new(&registry_path).save(&original).unwrap();
    let req = request("start", &project, "reuse");
    submit(&registry_path, req.clone(), caller("owner-b")).unwrap();
    let done = wait_operation(
        &registry_path,
        &req,
        &caller("owner-b"),
        Duration::from_secs(2),
    )
    .await
    .unwrap();
    assert_eq!(done["state"], "completed");
    assert_eq!(done["result"]["reused"], true);
    assert_eq!(done["result"]["owned"], false);
    assert_eq!(done["result"]["launched"], false);
    let associations = instance_associations(&registry_path, &entry);
    assert!(
        associations
            .iter()
            .any(|a| a["agent_context"]["session_id"] == "owner-b")
    );
    assert_eq!(
        LifecycleStore::new(&registry_path)
            .find_owned("fixture", &project)
            .unwrap()
            .unwrap()
            .operation_id,
        original.operation_id
    );
    let mut stop = request("stop", &project, "stop-foreign");
    stop.operation_id = done["operation_id"].as_str().map(str::to_owned);
    assert!(submit(&registry_path, stop, caller("owner-b")).is_err());
    server.abort();
}

#[tokio::test]
async fn safe_stop_reconciles_one_callback_and_refuses_changed_launch_identity() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    let router = axum::Router::new().route(
        "/stop",
        axum::routing::post(move |axum::Json(value): axum::Json<Value>| {
            let seen = seen.clone();
            async move {
                assert_eq!(value["discard_unsaved"], false);
                seen.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(250)).await;
                axum::Json(json!({"ok":true}))
            }
        }),
    );
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let registry_path = temp.path().join("registry");
    let registry = FileRegistry::new(&registry_path).unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    let mut entry = ServiceEntry::new("fixture", "127.0.0.1", port);
    entry.pid = Some(std::process::id());
    entry
        .metadata
        .insert("project".into(), project.display().to_string());
    entry
        .metadata
        .insert("meshy_dcc_operation_id".into(), id.clone());
    entry.metadata.insert(
        "safe_stop_url".into(),
        format!("http://127.0.0.1:{port}/stop"),
    );
    entry
        .metadata
        .insert("dispatch_status".into(), DispatchStatus::Ready.to_string());
    registry.register(entry.clone()).unwrap();
    let mut original =
        LifecycleOperation::new(&id, "fixture", &project, Path::new("fixture"), None);
    original.launched = true;
    original.owned = true;
    original.pid = entry.pid;
    original.instance_id = Some(entry.instance_id.to_string());
    original.management = Some(ManagedOperation {
        request_id: "launch".into(),
        action: "start".into(),
        owner: "owner".into(),
        payload_digest: "fixture".into(),
        state: "completed".into(),
        controller_pid: std::process::id(),
        agent_context: caller("owner").agent_context,
        result: Some(json!({"ok":true})),
        error: None,
    });
    let store = LifecycleStore::new(&registry_path);
    store.save(&original).unwrap();
    let mut stop = request("stop", &project, "stable-stop");
    stop.timeout_ms = Some(100);
    stop.operation_id = Some(id.clone());
    stop.instance_id = Some(entry.instance_id.to_string());
    let accepted = submit(&registry_path, stop.clone(), caller("owner")).unwrap();
    let mut query = stop.clone();
    query.operation_id = accepted["operation_id"].as_str().map(str::to_owned);
    let uncertain = wait_operation(
        &registry_path,
        &query,
        &caller("owner"),
        Duration::from_secs(2),
    )
    .await
    .unwrap();
    assert_eq!(uncertain["state"], "outcome_unknown");
    assert_eq!(
        submit(&registry_path, stop.clone(), caller("owner")).unwrap()["operation_id"],
        accepted["operation_id"]
    );
    let done = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let value = query_operation(&registry_path, &query, &caller("owner")).unwrap();
            if value["state"] == "completed" {
                break value;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(done["state"], "completed");
    assert_eq!(done["result"]["stop_scope"], "owned_operation");
    assert_eq!(
        submit(&registry_path, stop.clone(), caller("owner")).unwrap()["operation_id"],
        accepted["operation_id"]
    );
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert!(store.is_stopping(&entry.instance_id));
    assert_eq!(
        store
            .find_owned("fixture", &project)
            .unwrap()
            .unwrap()
            .operation_id,
        id
    );
    original.pid = Some(std::process::id() + 1);
    store.save(&original).unwrap();
    stop.request_id = Some("refuse-changed-pid".into());
    assert!(submit(&registry_path, stop, caller("owner")).is_err());
    assert_eq!(count.load(Ordering::SeqCst), 1);
    server.abort();
}
