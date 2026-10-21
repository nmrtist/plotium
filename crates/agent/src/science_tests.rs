use crate::session::{Failure, Session};
use plotium_core::{automation::*, state::*};
use serde_json::{Value, json};

fn call(
    s: &mut Session,
    app: &mut PlotiumApp,
    name: &str,
    mut arguments: Value,
) -> Result<Value, Failure> {
    arguments["expected_project_context"] = json!(app.session.project_context);
    arguments["expected_revision"] = json!(app.doc.automation_revision);
    s.call(app, name, arguments)
}
fn settle(app: &mut PlotiumApp) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.compute_busy() && std::time::Instant::now() < deadline {
        app.poll_compute();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    app.poll_compute();
    assert!(!app.compute_busy());
}
fn fixture() -> (PlotiumApp, tempfile::TempDir, std::path::PathBuf) {
    let (app, _) = crate::display_tests::fixture(false);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("frozen.plotiumproc");
    plotium_core::project::save_scheme(&path, &app.doc.datasets[0]).unwrap();
    (app, dir, path)
}
fn submit(s: &mut Session, app: &mut PlotiumApp, path: &std::path::Path) -> (Value, Value) {
    let owner = app.doc.datasets[0].resource_id().to_string();
    let plan = call(
        s,
        app,
        "plotium_plan",
        json!({"tool_id":"processing.apply_scheme","parameters":{"path":path},"ids":[owner]}),
    )
    .unwrap();
    let result = s
        .call(app, "plotium_execute", json!({"plan_id":plan["plan_id"]}))
        .unwrap();
    (plan, result)
}
#[test]
fn scientific_receipts_survive_connection_loss_and_consumed_plan_retries() {
    let (mut app, _dir, path) = fixture();
    let original = app.doc.datasets[0]
        .as_nmr()
        .unwrap()
        .processed
        .values()
        .to_vec();
    let mut session = Session::default();
    session.set_allow_modify(true);
    let (plan, submitted) = submit(&mut session, &mut app, &path);
    let task_id = submitted["value"]["task"]["task_id"].clone();
    assert_eq!(submitted["value"]["task"]["status"], "queued");
    assert_eq!(submitted["value"]["task"]["plan_id"], plan["plan_id"]);
    let repeat = session
        .call(
            &mut app,
            "plotium_execute",
            json!({"plan_id":plan["plan_id"]}),
        )
        .unwrap();
    assert_eq!(repeat["value"]["task"]["task_id"], task_id);
    drop(session);
    let mut reconnected = Session::default();
    let listed = reconnected
        .call(&mut app, "plotium_tasks_list", json!({"limit":1}))
        .unwrap();
    assert_eq!(listed["tasks"][0]["task_id"], task_id);
    settle(&mut app);
    let receipt = reconnected
        .call(&mut app, "plotium_tasks_get", json!({"task_id":task_id}))
        .unwrap();
    assert_eq!(receipt["task"]["status"], "succeeded", "{receipt}");
    assert_eq!(
        app.doc.datasets[0].as_nmr().unwrap().processed.values(),
        original
    );
    assert_eq!(app.session.undo_stack.len(), 1);
    app.undo();
    app.redo();
    assert!(
        !app.compute_busy(),
        "Redo must reuse the candidate snapshot"
    );
    let audit = json!(app.doc.automation_runs);
    assert_eq!(audit.as_array().unwrap().len(), 1);
    assert_eq!(audit[0]["nodes"][0]["tool_id"], "processing.apply_scheme");
    let project = _dir.path().join("science.plotium");
    plotium_core::project::save_project(&app, &project, false).unwrap();
    let restored = plotium_core::project::load_project(&project).unwrap();
    assert_eq!(json!(restored.doc.automation_runs), audit);
}
#[test]
fn scientific_cancellation_and_permission_revocation_prevent_commit() {
    for cancel in [false, true] {
        let (mut app, _dir, path) = fixture();
        let revision = app.doc.automation_revision;
        let mut session = Session::default();
        session.set_allow_modify(true);
        let (_, submitted) = submit(&mut session, &mut app, &path);
        let task_id = submitted["value"]["task"]["task_id"].clone();
        if cancel {
            session
                .call(&mut app, "plotium_tasks_cancel", json!({"task_id":task_id}))
                .unwrap();
            session
                .call(&mut app, "plotium_tasks_cancel", json!({"task_id":task_id}))
                .unwrap();
        } else {
            session.set_allow_modify(false);
        }
        settle(&mut app);
        let receipt = session
            .call(&mut app, "plotium_tasks_get", json!({"task_id":task_id}))
            .unwrap();
        assert_eq!(
            receipt["task"]["status"],
            if cancel { "cancelled" } else { "failed" }
        );
        assert_eq!(app.doc.automation_revision, revision);
        assert!(app.session.undo_stack.is_empty());
    }
}
#[test]
fn scientific_discovery_is_core_derived_and_cancel_requires_modify() {
    let manifest = crate::capabilities::manifest(false);
    let operations = manifest["scientific_operations"].as_array().unwrap();
    for id in [PROCESSING_EDIT, REGIONS_EXTRACT, DIFFUSION_FIT, DOSY_BUILD] {
        let descriptor = ToolRegistry::built_in().get(id).cloned().unwrap();
        let operation = operations
            .iter()
            .find(|o| o["descriptor"]["id"] == id)
            .unwrap();
        assert_eq!(operation["descriptor"], json!(descriptor));
        assert_eq!(operation["currently_permitted"], false);
    }
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let mut session = Session::default();
    assert_eq!(
        session
            .call(
                &mut app,
                "plotium_tasks_cancel",
                json!({"task_id":"unknown"})
            )
            .unwrap_err()
            .code,
        "permission_denied"
    );
    assert_eq!(
        session
            .call(&mut app, "plotium_tasks_get", json!({"task_id":"unknown"}))
            .unwrap_err()
            .code,
        "task_not_found"
    );
}

#[test]
fn disconnected_permission_lease_is_revoked_by_application_access_changes() {
    let (mut app, _dir, path) = fixture();
    let revision = app.doc.automation_revision;
    let mut session = Session::default();
    session.set_allow_modify(true);
    let (_, result) = submit(&mut session, &mut app, &path);
    let id: TaskId = serde_json::from_value(result["value"]["task"]["task_id"].clone()).unwrap();
    drop(session);
    app.session.scientific_tasks.revoke_external_permissions();
    settle(&mut app);
    let receipt = app.session.scientific_tasks.get(&id).unwrap();
    assert_eq!(receipt.status, TaskStatus::Failed);
    assert_eq!(receipt.failure_code.as_deref(), Some("permission_denied"));
    assert_eq!(app.doc.automation_revision, revision);
}
