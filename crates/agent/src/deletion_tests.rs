use crate::Session;
use plotium_core::{automation::*, state::*};
use serde_json::{Value, json};

fn fixture() -> PlotiumApp {
    let mut app = PlotiumApp::new();
    let table = materialized_float_series_table(
        ("x".into(), "s".into(), vec![Some(0.), Some(1.), Some(2.)]),
        [vec![1., 2., 3.], vec![2., 3., 4.]]
            .into_iter()
            .enumerate()
            .map(|(i, v)| FloatSeries {
                name: format!("signal_{i}"),
                unit: "mA".into(),
                values: v.into_iter().map(Some).collect(),
                uncertainty: None,
                fit: None,
            })
            .collect(),
        "plotium.test.deletion.agent",
    )
    .unwrap();
    app.doc.datasets.push(Dataset::Table(Box::new(table)));
    let table = app.doc.datasets[0].as_table().unwrap();
    let r = &table.typed_state.envelope.revision;
    let cols = &r.snapshot.schema.columns;
    let series = (1..=2)
        .map(|i| NewCurve {
            y_axis: Default::default(),
            kind: CurveKind::Line,
            source: TableCurveSource {
                dataset_id: table.resource_id,
                field_id: app.doc.datasets[0].default_field_id().unwrap(),
                table_id: r.table_id,
                table_revision: r.id,
                x_column_id: cols[0].id,
                y_column_id: cols[i].id,
                y_error_column_id: None,
            },
        })
        .collect();
    let p = app
        .plan_plot_creation(
            DocumentRevision(app.doc.automation_revision),
            &PlotCreateParams {
                parent: document_root_target(),
                name: "same".into(),
                canvas: None,
                panel_name: None,
                panel_frame_pt: None,
                series,
            },
        )
        .unwrap();
    app.commit_curve_bindings(p).unwrap();
    for s in &mut app.doc.canvases[0].objects[0]
        .plot_mut()
        .unwrap()
        .binding
        .series
    {
        s.label = Some("same".into());
    }
    app.clear_history();
    app
}
fn args(app: &PlotiumApp) -> Value {
    json!({"expected_project_context":app.session.project_context,"expected_revision":app.doc.automation_revision,"tool_id":FIGURE_EDIT,"parameters":{"canvas":TargetRef::resource(app.doc.canvases[0].resource_id.into()),"edits":[{"op":"series_delete","target":app.series_targets(0,app.doc.canvases[0].objects[0].id)[0]}]}})
}

#[test]
fn deletion_discovery_and_actual_series_transaction_are_precise() {
    let mut app = fixture();
    let mut s = Session::default();
    s.allow_modify = true;
    let state = s
        .call(
            &mut app,
            "plotium_capabilities_get",
            json!({"section":"operation_constraints","key":FIGURE_EDIT}),
        )
        .unwrap();
    let variants = &state["contract"]["schema"]["properties"]["edits"]["items"]["oneOf"];
    for op in ["content_delete", "panel_delete", "series_delete"] {
        assert!(
            variants
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v["properties"]["op"]["const"] == op)
        );
    }
    let rev = app.doc.automation_revision;
    let a = args(&app);
    let target = a["parameters"]["edits"][0]["target"].clone();
    let p = s.call(&mut app, "plotium_plan", a).unwrap();
    assert_eq!(p["preflight"]["figure_plan"]["deleted"], json!([target]));
    assert_eq!(app.doc.automation_revision, rev);
    let before = app.doc.canvases[0].objects[0]
        .plot()
        .unwrap()
        .binding
        .clone();
    let result = s
        .call(&mut app, "plotium_execute", json!({"plan_id":p["plan_id"]}))
        .unwrap();
    assert_eq!(result["value"]["deleted_targets"], json!([target]));
    assert_eq!(app.doc.automation_revision, rev + 1);
    assert_eq!(
        app.doc.canvases[0].objects[0]
            .plot()
            .unwrap()
            .binding
            .series,
        &before.series[1..]
    );
    assert_eq!(app.session.undo_stack.len(), 1);
    let last = args(&app);
    assert_eq!(
        s.call(&mut app, "plotium_plan", last).unwrap_err().code,
        "invalid_parameters"
    );
    assert_eq!(
        s.call(&mut app, "plotium_execute", json!({"plan_id":p["plan_id"]}))
            .unwrap_err()
            .code,
        "expired_plan"
    );
    app.undo();
    assert_eq!(
        app.doc.canvases[0].objects[0].plot().unwrap().binding,
        before
    );
    app.redo();
    assert_eq!(
        app.doc.canvases[0].objects[0]
            .plot()
            .unwrap()
            .binding
            .series
            .len(),
        1
    );
}

#[test]
fn deletion_plans_fail_after_revoke_mutation_busy_or_equal_revision_project_switch() {
    for fault in ["revoke", "revision", "busy", "project"] {
        let mut app = fixture();
        let mut s = Session::default();
        s.allow_modify = true;
        let a = args(&app);
        let p = s.call(&mut app, "plotium_plan", a).unwrap();
        let rev = app.doc.automation_revision;
        let expected = match fault {
            "revoke" => {
                s.allow_modify = false;
                "permission_denied"
            }
            "revision" => {
                app.doc.automation_revision += 1;
                "stale_revision"
            }
            "project" => {
                app = fixture();
                app.doc.automation_revision = rev;
                "project_changed"
            }
            _ => "busy",
        };
        let before = app.doc.canvases[0].objects[0]
            .plot()
            .unwrap()
            .binding
            .clone();
        assert_eq!(
            s.call_when_available(
                &mut app,
                "plotium_execute",
                json!({"plan_id":p["plan_id"]}),
                fault == "busy"
            )
            .unwrap_err()
            .code,
            expected
        );
        assert_eq!(
            app.doc.canvases[0].objects[0].plot().unwrap().binding,
            before
        );
        assert!(!app.can_undo());
        s.allow_modify = true;
        if fault != "project" {
            assert_eq!(
                s.call(&mut app, "plotium_execute", json!({"plan_id":p["plan_id"]}))
                    .unwrap_err()
                    .code,
                "expired_plan"
            );
        }
    }
}

#[test]
fn deletion_rejects_bad_targets_and_obeys_native_plan_and_response_budgets() {
    let mut app = fixture();
    let mut s = Session::default();
    let a = args(&app);
    assert_eq!(
        s.call(&mut app, "plotium_plan", a.clone())
            .unwrap_err()
            .code,
        "permission_denied"
    );
    s.allow_modify = true;
    for edit in [
        json!({"op":"content_delete","target":a["parameters"]["edits"][0]["target"]}),
        json!({"op":"series_delete","target":{"resource":a["parameters"]["canvas"]["resource"]}}),
    ] {
        let mut bad = a.clone();
        bad["parameters"]["edits"] = json!([edit]);
        assert_eq!(
            s.call(&mut app, "plotium_plan", bad).unwrap_err().code,
            "invalid_parameters"
        );
    }
    let plot = app.doc.canvases[0].objects[0].id;
    app.doc.canvases[0].objects[0].locked = true;
    assert_eq!(
        s.call(&mut app, "plotium_plan", a.clone())
            .unwrap_err()
            .code,
        "invalid_parameters"
    );
    app.doc.canvases[0].objects[0].locked = false;
    for _ in 0..32 {
        s.call(&mut app, "plotium_plan", a.clone()).unwrap();
    }
    assert_eq!(
        s.call(&mut app, "plotium_plan", a).unwrap_err().code,
        "plan_limit"
    );
    let mut s = Session::default();
    s.allow_modify = true;
    // A whole plot can own enough stable identities to exceed the complete manifest budget.
    let p = app.doc.canvases[0].objects[0].plot_mut().unwrap();
    let template = p.binding.series[0].clone();
    for _ in 0..1800 {
        let mut curve = template.clone();
        curve.id = p.allocate_series_id();
        p.binding.series.push(curve);
    }
    let a = json!({"expected_project_context":app.session.project_context,"expected_revision":app.doc.automation_revision,"tool_id":FIGURE_EDIT,"parameters":{"canvas":TargetRef::resource(app.doc.canvases[0].resource_id.into()),"edits":[{"op":"content_delete","target":TargetRef::resource(canvas_object_ref(app.doc.canvases[0].resource_id,plot))}]}});
    assert_eq!(
        s.call(&mut app, "plotium_plan", a).unwrap_err().code,
        "response_too_large"
    );
    assert_eq!(app.doc.canvases[0].objects.len(), 1);
    assert!(!app.can_undo());
}
