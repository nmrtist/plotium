use super::*;
use plotium_core::{
    data::*,
    state::{Dataset, TableDataset},
};

fn fixture() -> (PlotiumApp, Session, Value) {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let columns = [
        ("x", vec![Some(0.), Some(1.), Some(2.)]),
        ("y", vec![Some(1.), None, Some(3.)]),
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
        TableDataset::from_materialized(columns, vec![], None, vec![], "plotium.test.creation")
            .unwrap();
    app.doc.datasets.push(Dataset::Table(Box::new(table)));
    let table = app.doc.datasets[0].as_table().unwrap();
    let r = &table.typed_state.envelope.revision;
    let c = &r.snapshot.schema.columns;
    let curve = json!({"kind":"line","source":{
        "dataset_id":table.resource_id,"field_id":app.doc.datasets[0].default_field_id().unwrap(),
        "table_id":r.table_id,"table_revision":r.id,"x_column_id":c[0].id,
        "y_column_id":c[1].id,"y_error_column_id":null
    }});
    let mut session = Session::default();
    session.set_allow_modify(true);
    (app, session, curve)
}

#[test]
fn five_series_across_thirteen_panels_keep_frozen_identity_diagnostics_and_undo() {
    let (mut app, mut session, curve) = fixture();
    let mut parent = json!(document_root_target());
    for panel in 0..13 {
        let mut parameters =
            json!({"parent":parent,"name":format!("Panel {panel}"),"series":vec![curve.clone();5]});
        if panel != 0 {
            parameters["panel_frame_pt"] = json!([10, 10, 100, 80]);
        }
        let args = json!({"expected_project_context":app.session.project_context,
            "expected_revision":app.doc.automation_revision,"tool_id":PLOT_CREATE,"parameters":parameters});
        let response = session.call(&mut app, "plotium_plan", args).unwrap();
        let id = response["plan_id"].as_str().unwrap();
        let stored = session.plans[id].clone();
        let binding = stored.binding_plan.as_ref().unwrap();
        let creation = binding.creation.as_ref().unwrap();
        assert_eq!(
            response["project_context"],
            json!(app.session.project_context)
        );
        assert_eq!(
            response["basis_revision"],
            json!(app.doc.automation_revision)
        );
        assert_eq!(
            response["preflight"]["binding_plan"]["creation"],
            json!(creation)
        );
        assert_eq!(
            response["preflight"]["created_target_count"],
            creation.created.len()
        );
        assert_eq!(response["preflight"]["binding_plan"]["binding_count"], 5);
        assert_eq!(
            response["preflight"]["binding_plan"]["row_counts"],
            json!({"total_rows":15,"plotted_rows":10})
        );
        let rows = response["preflight"]["binding_plan"]["bindings"]
            .as_array()
            .unwrap();
        assert_eq!(rows.len(), 5);
        for (row, original) in rows.iter().zip(&binding.bindings) {
            assert_eq!(row["target"], json!(original.target));
            assert_eq!(row["diagnostics"], json!(original.diagnostics));
            assert!(row.get("source").is_none());
            assert_eq!(row["diagnostics"]["missing_xy"], 1);
        }
        assert!(response["preflight"].get("request").is_none());
        assert!(response["preflight"].get("frozen_targets").is_none());
        assert!(
            response["preflight"]["targets"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_eq!(json!(binding.bindings[0].source), curve["source"]);
        let result = session
            .call(&mut app, "plotium_execute", json!({"plan_id":id}))
            .unwrap();
        assert_eq!(result["project_context"], response["project_context"]);
        assert_eq!(result["before_revision"], response["basis_revision"]);
        assert_eq!(result["after_revision"], json!(app.doc.automation_revision));
        assert_eq!(result["summary"]["succeeded"], creation.created.len());
        assert!(result["targets"].as_array().unwrap().is_empty());
        assert!(result["value"]["creation"].get("series_defaults").is_none());
        let expected_produced: Vec<_> = creation
            .created
            .iter()
            .filter(|t| t.component.is_none())
            .map(|t| &t.resource)
            .collect();
        assert_eq!(result["produced"], json!(expected_produced));
        assert_eq!(
            result["value"]["bindings"].as_array().unwrap().len(),
            binding.bindings.len()
        );
        for (row, b) in result["value"]["bindings"]
            .as_array()
            .unwrap()
            .iter()
            .zip(&binding.bindings)
        {
            assert_eq!(row["target"], json!(b.target));
            let mut counts = json!(b.diagnostics);
            counts.as_object_mut().unwrap().remove("affected_rows");
            assert_eq!(row["diagnostics"], counts);
        }
        parent = result["value"]["creation"]["canvas"].clone();
        let before = app.doc.canvases[0].clone();
        app.undo();
        app.redo();
        let after = &app.doc.canvases[0];
        assert_eq!(after.resource_id, before.resource_id);
        assert_eq!(after.panels, before.panels);
        assert_eq!(after.objects.len(), before.objects.len());
        for (a, b) in after.objects.iter().zip(&before.objects) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.frame, b.frame);
            assert_eq!(a.plot().unwrap().binding, b.plot().unwrap().binding);
            assert_eq!(
                json!(a.plot().unwrap().figure()),
                json!(b.plot().unwrap().figure())
            );
        }
    }
    assert_eq!(app.doc.canvases.len(), 1);
    assert_eq!(app.doc.canvases[0].panels.len(), 13);
}

#[test]
fn clean_bindings_omit_preflight_issue_rows_but_return_all_execution_targets() {
    let (mut app, mut session, mut curve) = fixture();
    curve["source"]["y_column_id"] = curve["source"]["x_column_id"].clone();
    let context = app.session.project_context;
    let response = session.call(&mut app,"plotium_plan",json!({
        "expected_project_context":context,"expected_revision":0,
        "tool_id":PLOT_CREATE,"parameters":{"parent":document_root_target(),"name":"Clean","series":[curve]}
    })).unwrap();
    assert_eq!(response["preflight"]["binding_plan"]["binding_count"], 1);
    assert_eq!(response["preflight"]["binding_plan"]["bindings"], json!([]));
    let result = session
        .call(
            &mut app,
            "plotium_execute",
            json!({"plan_id":response["plan_id"]}),
        )
        .unwrap();
    assert_eq!(result["value"]["bindings"].as_array().unwrap().len(), 1);
    assert_eq!(
        result["value"]["bindings"][0]["diagnostics"]["plotted_rows"],
        3
    );
}

#[test]
fn execute_projection_preserves_exceptional_targets_diagnostics_and_verification() {
    let (_, _, curve) = fixture();
    let target = document_root_target();
    let result = ToolResult {
        tool_id: PLOT_CREATE.into(),
        before_revision: DocumentRevision(0),
        after_revision: DocumentRevision(1),
        targets: vec![TargetResult {
            target: target.clone(),
            outcome: TargetOutcome::Failed,
            message: "test failure".into(),
            skip_reason: Some("test reason".into()),
            fingerprints: vec![],
        }],
        produced: vec![],
        modified: vec![],
        diagnostics: vec!["warning".into()],
        verification: vec![VerificationRecord {
            check: "test".into(),
            passed: true,
            message: "verified".into(),
        }],
        value: json!({"creation":{"series_defaults":[],"plot":target},"bindings":[curve]}),
    };
    let full = json!(result);
    let compact = execute_response(result);
    for field in [
        "targets",
        "diagnostics",
        "verification",
        "produced",
        "modified",
    ] {
        assert_eq!(compact[field], full[field]);
    }
    assert_eq!(compact["value"]["bindings"], full["value"]["bindings"]);
    assert_eq!(compact["summary"]["failed"], 1);
}
