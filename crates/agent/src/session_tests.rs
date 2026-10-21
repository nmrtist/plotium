use super::session::*;
use plotium_core::{
    automation::CallerType,
    state::{CanvasDocument, PlotiumApp},
};
use serde_json::{Value, json};

fn app() -> PlotiumApp {
    let mut app = PlotiumApp::new();
    app.doc
        .canvases
        .push(CanvasDocument::new("Figure".into(), [120.0, 80.0]));
    app
}
fn plan(
    session: &mut Session,
    app: &mut PlotiumApp,
    tool: &str,
    parameters: Value,
    ids: Value,
) -> Result<Value, Failure> {
    let context = app.session.project_context;
    let revision = app.doc.automation_revision;
    session.call(app, "plotium_plan", json!({"tool_id": tool, "parameters": parameters, "ids": ids, "expected_project_context":context,"expected_revision": revision}))
}
fn typography(session: &mut Session, app: &mut PlotiumApp, value: f64) -> Value {
    plan(
        session,
        app,
        "properties.set",
        json!({"key": "document.figure.typography.tick_pt", "value": value}),
        json!(["document"]),
    )
    .unwrap()
}
fn execute(session: &mut Session, app: &mut PlotiumApp, plan: &Value) -> Result<Value, Failure> {
    session.call(app, "plotium_execute", json!({"plan_id": plan["plan_id"]}))
}
#[test]
fn read_only_denial_is_audited_and_consumes_plan() {
    let mut app = app();
    let mut session = Session::default();
    let p = typography(&mut session, &mut app, 15.0);
    assert_eq!(
        execute(&mut session, &mut app, &p).unwrap_err().code,
        "permission_denied"
    );
    session.allow_modify = true;
    assert_eq!(
        execute(&mut session, &mut app, &p).unwrap_err().code,
        "expired_plan"
    );
    assert_eq!(app.doc.automation_revision, 0);
    assert_eq!(
        app.doc.automation_runs.last().unwrap().caller,
        CallerType::Agent
    );
    assert!(!app.doc.automation_runs.last().unwrap().errors.is_empty());
}
#[test]
fn typography_edit_undo_redo_and_same_value_skip() {
    let mut app = app();
    let mut session = Session::default();
    session.allow_modify = true;
    let before = app.doc.style_library.figure_typography;
    let p = typography(&mut session, &mut app, 15.0);
    let result = execute(&mut session, &mut app, &p).unwrap();
    assert!(result["after_revision"].as_u64().unwrap() > 0);
    assert_eq!(app.doc.style_library.figure_typography.tick_pt, 15.0);
    let p = typography(&mut session, &mut app, 15.0);
    let result = execute(&mut session, &mut app, &p).unwrap();
    assert_eq!(result["targets"][0]["outcome"], "skipped");
    app.undo();
    assert_eq!(app.doc.style_library.figure_typography, before);
    app.redo();
    assert_eq!(app.doc.style_library.figure_typography.tick_pt, 15.0);
}
#[test]
fn axis_strokes_and_tick_direction_are_settable() {
    let mut app = app();
    let mut session = Session::default();
    session.allow_modify = true;
    for (key, value) in [
        ("document.figure.typography.axis_line_pt", json!(0.6)),
        ("document.figure.typography.tick_length_pt", json!(2.5)),
        ("document.figure.typography.tick_direction", json!("in")),
    ] {
        let p = plan(
            &mut session,
            &mut app,
            "properties.set",
            json!({"key": key, "value": value}),
            json!(["document"]),
        )
        .unwrap();
        execute(&mut session, &mut app, &p).unwrap();
    }
    let typography = app.doc.style_library.figure_typography;
    assert_eq!(typography.axis_line_pt, 0.6);
    assert_eq!(typography.tick_length_pt, 2.5);
    assert_eq!(typography.tick_outset(), 0.0);
    assert_eq!(typography.tick_reach(), -2.5);
}
#[test]
fn manual_edit_undo_delete_and_permission_revocation_block_old_plans() {
    let mut app = app();
    let mut session = Session::default();
    session.allow_modify = true;
    let p = typography(&mut session, &mut app, 15.0);
    let id = app.doc.canvases[0].resource_id.to_string();
    let rename = plan(
        &mut session,
        &mut app,
        "resource.rename",
        json!({"name": "New"}),
        json!([id]),
    )
    .unwrap();
    execute(&mut session, &mut app, &rename).unwrap();
    assert_eq!(app.doc.canvases[0].name, "New");
    assert_eq!(
        execute(&mut session, &mut app, &p).unwrap_err().code,
        "stale_revision"
    );
    let p = typography(&mut session, &mut app, 15.0);
    app.undo();
    assert_eq!(
        execute(&mut session, &mut app, &p).unwrap_err().code,
        "stale_revision"
    );
    let p = typography(&mut session, &mut app, 15.0);
    session.allow_modify = false;
    assert_eq!(
        execute(&mut session, &mut app, &p).unwrap_err().code,
        "permission_denied"
    );
}
#[test]
fn closed_capability_boundary_and_parameter_validation() {
    let mut app = app();
    let context = app.session.project_context;
    let mut session = Session::default();
    for key in [
        "settings.appearance.accent.color",
        "dataset.processing.apodization.lb_hz",
        "series.contour.ratio",
        "missing",
    ] {
        assert_eq!(
            plan(
                &mut session,
                &mut app,
                "properties.set",
                json!({"key": key, "value": 2}),
                json!(["document"])
            )
            .unwrap_err()
            .code,
            "unsupported_property"
        );
        assert_eq!(
            session
                .call(
                    &mut app,
                    "plotium_properties_inspect",
                    json!({"key":key,"ids":["document"],"expected_project_context":context,"expected_revision":0})
                )
                .unwrap_err()
                .code,
            "unsupported_property"
        );
    }
    assert_eq!(
        plan(
            &mut session,
            &mut app,
            "data.import",
            json!({"paths":[]}),
            json!(["document"])
        )
        .unwrap_err()
        .code,
        "permission_denied"
    );
    assert_eq!(
        plan(
            &mut session,
            &mut app,
            "resource.rename",
            json!({"name":"bad"}),
            json!(["document"])
        )
        .unwrap_err()
        .code,
        "unsupported_operation"
    );
    assert_eq!(
        plan(
            &mut session,
            &mut app,
            "properties.set",
            json!({"key":"document.figure.typography.tick_pt","value":9999}),
            json!(["document"])
        )
        .unwrap_err()
        .code,
        "invalid_parameters"
    );
    assert_eq!(
        plan(
            &mut session,
            &mut app,
            "properties.reset",
            json!({"key":"document.figure.typography.tick_pt"}),
            json!([])
        )
        .unwrap_err()
        .code,
        "invalid_parameters"
    );
    assert_eq!(
        session
            .call(
                &mut app,
                "plotium_execute",
                json!({"plan_id":"anything","authority":"destructive"})
            )
            .unwrap_err()
            .code,
        "invalid_parameters"
    );
}
#[test]
fn busy_plan_consumption_limit_and_query_isolation() {
    let mut app = app();
    let mut session = Session::default();
    session
        .call(&mut app, "plotium_get_state", json!({}))
        .unwrap();
    assert!(!app.doc.dirty);
    assert!(app.doc.automation_runs.is_empty());
    let p = typography(&mut session, &mut app, 15.0);
    app.session.ui.phase_edit_active = true;
    assert_eq!(
        execute(&mut session, &mut app, &p).unwrap_err().code,
        "busy"
    );
    assert_eq!(
        session
            .call(&mut app, "plotium_get_state", json!({}))
            .unwrap()["availability"]["project_read"]["reason"]["code"],
        "busy"
    );
    app.session.ui.phase_edit_active = false;
    assert_eq!(
        execute(&mut session, &mut app, &p).unwrap_err().code,
        "expired_plan"
    );
    for _ in 0..32 {
        typography(&mut session, &mut app, 15.0);
    }
    assert_eq!(
        plan(
            &mut session,
            &mut app,
            "properties.reset",
            json!({"key":"document.figure.typography.tick_pt"}),
            json!(["document"])
        )
        .unwrap_err()
        .code,
        "plan_limit"
    );
}

#[test]
fn shell_busy_barrier_consumes_submitted_plan() {
    let mut app = app();
    let mut session = Session::default();
    session.allow_modify = true;
    let p = typography(&mut session, &mut app, 15.0);
    assert_eq!(
        session
            .call_when_available(
                &mut app,
                "plotium_execute",
                json!({"plan_id": p["plan_id"]}),
                true
            )
            .unwrap_err()
            .code,
        "busy"
    );
    assert_eq!(
        execute(&mut session, &mut app, &p).unwrap_err().code,
        "expired_plan"
    );
    assert_eq!(app.doc.automation_revision, 0);
}

#[test]
fn queued_import_blocks_project_access_before_background_insertion() {
    let mut app = app();
    let mut session = Session::default();
    let p = typography(&mut session, &mut app, 15.0);
    app.queue_data_import(std::path::PathBuf::from("fixture"), || Ok(vec![]));
    assert_eq!(
        session
            .call(&mut app, "plotium_get_state", json!({}))
            .unwrap()["availability"]["project_read"]["reason"]["code"],
        "busy"
    );
    assert_eq!(
        execute(&mut session, &mut app, &p).unwrap_err().code,
        "busy"
    );
    app.session.data_imports = Default::default();
    assert_eq!(
        execute(&mut session, &mut app, &p).unwrap_err().code,
        "expired_plan"
    );
}

#[test]
fn repeated_state_reads_preserve_a_saved_project() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("state.plotium");
    plotium_core::project::save_project(&app(), &path, false).unwrap();
    let mut app = plotium_core::project::load_project(&path).unwrap();
    let mut session = Session::default();
    let before = (
        app.doc.dirty,
        app.doc.edit_generation,
        app.doc.automation_revision,
        app.doc.automation_runs.len(),
    );
    assert!(!before.0);
    for _ in 0..10 {
        session
            .call(&mut app, "plotium_get_state", json!({}))
            .unwrap();
        assert_eq!(
            (
                app.doc.dirty,
                app.doc.edit_generation,
                app.doc.automation_revision,
                app.doc.automation_runs.len()
            ),
            before
        );
    }
}
