use super::*;
use plotium_core::{
    data::*,
    state::{Dataset, TableDataset},
};

fn fixture() -> (PlotiumApp, Session, Value) {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let columns = [
        ("x", vec![Some(0.), Some(1.), Some(2.)]),
        ("y", vec![Some(1.), Some(3.), Some(2.)]),
    ]
    .into_iter()
    .map(|(name, values)| {
        (
            ColumnSchema::new(name, LogicalType::Float64),
            ColumnChunk::optional_f64(values),
        )
    })
    .collect();
    let table =
        TableDataset::from_materialized(columns, vec![], None, vec![], "plotium.test.apply")
            .unwrap();
    app.doc.datasets.push(Dataset::Table(Box::new(table)));
    let table = app.doc.datasets[0].as_table().unwrap();
    let r = &table.typed_state.envelope.revision;
    let c = &r.snapshot.schema.columns;
    let parameters = json!({"parent":plotium_core::automation::document_root_target(),"name":"P",
        "series":[{"kind":"line","source":{"dataset_id":table.resource_id,
        "field_id":app.doc.datasets[0].default_field_id().unwrap(),"table_id":r.table_id,
        "table_revision":r.id,"x_column_id":c[0].id,"y_column_id":c[1].id,"y_error_column_id":null}}]});
    let mut session = Session::default();
    session.set_allow_modify(true);
    (app, session, parameters)
}

fn call(
    session: &mut Session,
    app: &mut PlotiumApp,
    name: &str,
    tool: &str,
    parameters: Value,
) -> std::result::Result<Value, Failure> {
    let request = args(app, tool, parameters);
    session.call(app, name, request)
}

fn args(app: &PlotiumApp, tool: &str, parameters: Value) -> Value {
    json!({"expected_project_context":app.session.project_context,
        "expected_revision":app.doc.automation_revision,"tool_id":tool,"parameters":parameters})
}

#[test]
fn apply_commits_like_plan_then_execute() {
    let (mut two_step, mut s2, create) = fixture();
    let plan = call(
        &mut s2,
        &mut two_step,
        "plotium_plan",
        PLOT_CREATE,
        create.clone(),
    )
    .unwrap();
    let executed = s2
        .call(
            &mut two_step,
            "plotium_execute",
            json!({"plan_id":plan["plan_id"]}),
        )
        .unwrap();
    let (mut one_step, mut s1, create) = fixture();
    let applied = call(&mut s1, &mut one_step, "plotium_apply", PLOT_CREATE, create).unwrap();
    assert!(s1.plans.is_empty());
    for key in ["tool_id", "before_revision", "after_revision", "summary"] {
        assert_eq!(applied[key], executed[key], "{key}");
    }
    assert_eq!(
        applied["project_context"],
        json!(one_step.session.project_context)
    );
    assert_eq!(
        one_step.doc.automation_runs.len(),
        two_step.doc.automation_runs.len()
    );
    assert_eq!(
        one_step.session.undo_stack.len(),
        two_step.session.undo_stack.len()
    );
    let target = applied["value"]["bindings"][0]["target"].clone();
    let write = json!({"writes":[{"target":target,"key":"series.line.color","value":"#123456"}]});
    let styled = call(
        &mut s1,
        &mut one_step,
        "plotium_apply",
        TOOL_SET_MANY,
        write,
    )
    .unwrap();
    assert_eq!(styled["summary"]["succeeded"], 1);
    let steps = one_step.session.undo_stack.len();
    one_step.undo();
    assert_eq!(one_step.session.undo_stack.len(), steps - 1);
}

#[test]
fn apply_refuses_deletions_files_read_only_and_stale_requests_without_retaining_plans() {
    let (mut app, mut session, create) = fixture();
    let dataset = app.doc.datasets[0].resource_id().to_string();
    for (tool, parameters) in [
        (DATASET_DELETE, json!({})),
        ("project.save", json!({"path":"/tmp/never.plotium"})),
        ("data.import", json!({"paths":["/tmp/never.csv"]})),
    ] {
        let mut request = args(&app, tool, parameters);
        if tool == DATASET_DELETE {
            request["ids"] = json!([dataset]);
        }
        let error = session
            .call(&mut app, "plotium_apply", request)
            .unwrap_err();
        assert_eq!(error.code, "unsupported_operation", "{tool}");
    }
    let deletion = json!({"canvas":{"resource":{"id":uuid::Uuid::new_v4(),"kind":"plotium.canvas"}},
        "edits":[{"op":"content_delete","target":{"resource":{"id":"c/1","kind":"plotium.canvas.object"}}}]});
    let error = call(
        &mut session,
        &mut app,
        "plotium_apply",
        FIGURE_EDIT,
        deletion,
    )
    .unwrap_err();
    assert_eq!(error.code, "unsupported_operation");
    let mut stale = args(&app, PLOT_CREATE, create.clone());
    stale["expected_revision"] = json!(app.doc.automation_revision + 1);
    let error = session.call(&mut app, "plotium_apply", stale).unwrap_err();
    assert_eq!(error.code, "stale_revision");
    let recovery = error.value()["error"]["recovery"].clone();
    assert_eq!(recovery["committed"], false);
    assert_eq!(recovery["submission"], "not_submitted");
    session.set_allow_modify(false);
    let error = call(&mut session, &mut app, "plotium_apply", PLOT_CREATE, create).unwrap_err();
    assert_eq!(error.code, "permission_denied");
    assert!(session.plans.is_empty());
    assert!(app.doc.canvases.is_empty());
    assert_eq!(app.doc.automation_revision, 0);
}

#[test]
fn interrupted_apply_reports_an_unknown_outcome() {
    let request = json!({"tool_id":PLOT_CREATE,"parameters":{}});
    for code in ["request_cancelled", "connection_stopped"] {
        let recovery = Failure::new(code, "interrupted")
            .for_request("plotium_apply", &request)
            .value()["error"]["recovery"]
            .clone();
        assert_eq!(recovery["submission"], "unknown", "{code}");
        assert_eq!(recovery["committed"], Value::Null, "{code}");
        assert_eq!(recovery["completion"], "unknown", "{code}");
        assert_eq!(recovery["replay_allowed"], false, "{code}");
        let planned = Failure::new(code, "interrupted")
            .for_request("plotium_plan", &request)
            .value()["error"]["recovery"]
            .clone();
        assert_eq!(planned["committed"], false, "{code}");
    }
    let rejected = Failure::new("busy", "Desktop queue is full")
        .for_request("plotium_apply", &request)
        .value()["error"]["recovery"]
        .clone();
    assert_eq!(rejected["committed"], false);
}
