use super::*;

fn plan(
    app: &mut PlotiumApp,
    session: &mut Session,
    prior: &Value,
    value: f64,
) -> Result<Value, Failure> {
    session.call(
        app,
        "plotium_plan",
        json!({
            "expected_project_context":prior["project_context"],
            "expected_revision":prior["after_revision"],
            "ids":["document"],"tool_id":"properties.set",
            "parameters":{"key":"document.figure.typography.tick_pt","value":value}
        }),
    )
}

fn execute(app: &mut PlotiumApp, session: &mut Session, plan: &Value) -> Result<Value, Failure> {
    session.call(app, "plotium_execute", json!({"plan_id":plan["plan_id"]}))
}

#[test]
fn committed_results_chain_without_state_and_still_reject_stale_or_revoked_work() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let mut session = Session::default();
    session.set_allow_modify(true);
    let initial = json!({"project_context":app.session.project_context,"after_revision":app.doc.automation_revision});
    let first = plan(&mut app, &mut session, &initial, 15.).unwrap();
    let first = execute(&mut app, &mut session, &first).unwrap();
    assert_eq!(first["project_context"], initial["project_context"]);
    assert_eq!(first["before_revision"], initial["after_revision"]);
    assert_eq!(first["after_revision"], json!(app.doc.automation_revision));
    let second = plan(&mut app, &mut session, &first, 16.).unwrap();
    let second = execute(&mut app, &mut session, &second).unwrap();
    assert_eq!(second["before_revision"], first["after_revision"]);
    assert!(second["after_revision"].as_u64().unwrap() > first["after_revision"].as_u64().unwrap());
    let pending = plan(&mut app, &mut session, &second, 17.).unwrap();
    app.undo();
    assert_eq!(
        plan(&mut app, &mut session, &second, 18.).unwrap_err().code,
        "stale_revision"
    );
    assert_eq!(
        execute(&mut app, &mut session, &pending).unwrap_err().code,
        "stale_revision"
    );
    let current = json!({"project_context":app.session.project_context,"after_revision":app.doc.automation_revision});
    let pending = plan(&mut app, &mut session, &current, 19.).unwrap();
    session.set_allow_modify(false);
    assert_eq!(
        execute(&mut app, &mut session, &pending).unwrap_err().code,
        "permission_denied"
    );
    let mut other = PlotiumApp::new_with_settings(Default::default());
    other.doc.automation_revision = app.doc.automation_revision;
    assert_eq!(
        plan(&mut other, &mut session, &current, 20.)
            .unwrap_err()
            .code,
        "project_changed"
    );
}

#[test]
fn only_current_synchronous_results_gain_context() {
    let app = PlotiumApp::new_with_settings(Default::default());
    for mut value in [
        json!({"tool_id":"processing.apply_scheme","before_revision":0,"after_revision":0,"value":{"task":{"status":"queued"}}}),
        json!({"tool_id":"processing.apply_scheme","before_revision":0,"after_revision":0,"value":{"task":{"status":"succeeded"}}}),
        json!({"save":{"status":"awaiting_authorization"}}),
        json!({"export":{"status":"completed"}}),
        json!({"tool_id":"figure.edit","before_revision":0,"after_revision":1}),
    ] {
        annotate(&app, "plotium_execute", &mut value);
        assert!(value.get("project_context").is_none());
    }
    let mut read = json!({"tool_id":"query","before_revision":0,"after_revision":0});
    annotate(&app, "plotium_resources_inspect", &mut read);
    assert!(read.get("project_context").is_none());
}

#[test]
fn ordinary_figure_edit_result_also_reports_its_committed_context() {
    let (mut app, mut session, _) = crate::guide_fixture::fixture();
    let context = app.session.project_context;
    let before = app.doc.automation_revision;
    let canvas = TargetRef::resource(app.doc.canvases[0].resource_id.into());
    let plan = session.call(&mut app, "plotium_plan", json!({
        "expected_project_context":context,"expected_revision":before,"tool_id":"figure.edit",
        "parameters":{"canvas":canvas,"edits":[{"op":"page","size_mm":[180,90]}]}
    })).unwrap();
    let result = execute(&mut app, &mut session, &plan).unwrap();
    assert_eq!(result["project_context"], json!(context));
    assert_eq!(result["before_revision"], json!(before));
    assert_eq!(result["after_revision"], json!(app.doc.automation_revision));
}
