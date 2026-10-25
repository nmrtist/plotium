use super::*;
use crate::automation::*;
use crate::state::*;
use plotium_data::*;
use serde_json::json;
use std::sync::atomic::AtomicBool;

fn fixture(derived: bool) -> (PlotiumApp, usize) {
    let mut app = PlotiumApp::new();
    let table = TableDataset::from_typed(
        TypedTableState::materialized(
            vec![
                materialized_float_column("x", "", [Some(0.), Some(1.)]),
                materialized_float_column("y", "", [Some(2.), Some(3.)]),
                materialized_float_column("sigma", "", [Some(0.1), Some(0.2)]),
            ],
            vec![],
            "plotium.test.table-update.v1",
        )
        .unwrap(),
    );
    app.doc.datasets.push(Dataset::Table(Box::new(table)));
    let index = if derived {
        let revision = &app.doc.datasets[0]
            .as_table()
            .unwrap()
            .typed_state
            .envelope
            .revision;
        let plan = RelPlanV1::new(Relation::Project {
            input: Box::new(Relation::SnapshotRead(SnapshotRead {
                table: revision.table_id,
                revision: revision.id,
                fingerprint: revision.snapshot.fingerprint,
            })),
            columns: revision
                .snapshot
                .schema
                .columns
                .iter()
                .map(|c| c.id)
                .collect(),
        });
        app.derive_table_from_plan(plan, &[0], "Projected".into(), 16777216)
            .unwrap()
    } else {
        0
    };
    let dataset = &app.doc.datasets[index];
    let revision = &dataset.as_table().unwrap().typed_state.envelope.revision;
    let columns = &revision.snapshot.schema.columns;
    let parameters = PlotCreateParams {
        parent: document_root_target(),
        name: "Curve with errors".into(),
        canvas: None,
        panel_name: None,
        panel_frame_pt: None,
        series: vec![NewCurve {
            y_axis: Default::default(),
            kind: CurveKind::Line,
            source: TableCurveSource {
                dataset_id: dataset.resource_id(),
                field_id: dataset.default_field_id().unwrap(),
                table_id: revision.table_id,
                table_revision: revision.id,
                x_column_id: columns[0].id,
                y_column_id: columns[1].id,
                y_error_column_id: Some(columns[2].id),
            },
        }],
        intent: None,
    };
    let plan = app
        .plan_plot_creation(DocumentRevision(app.doc.automation_revision), &parameters)
        .unwrap();
    app.commit_curve_bindings(plan).unwrap();
    app.clear_history();
    (app, index)
}

fn patch(
    app: &mut PlotiumApp,
    index: usize,
    value: f64,
    automation: bool,
) -> std::result::Result<(), String> {
    let dataset = &app.doc.datasets[index];
    let table = dataset.as_table().unwrap();
    let revision = &table.typed_state.envelope.revision;
    let column = revision.snapshot.schema.columns[2].id;
    let row = table.typed_rows(1, &[column]).unwrap().row_ids[0];
    if automation {
        let request = ToolRequest {
            tool_id: TABLE_PATCH.into(),
            tool_version: 1,
            parameters: json!({"table_revision": revision.id, "edits": [{
                "row_id": row, "column_id": column, "value": {"kind": "float64", "value": value}
            }]}),
            targets: TargetSelector::Explicit {
                ids: vec![dataset.resource_id().to_string()],
            },
            expected_revision: DocumentRevision(app.doc.automation_revision),
            caller: CallerType::Human,
        };
        let plan = plan_tool(app, request).map_err(|e| e.to_string())?;
        let prepared = TableOperationSnapshot::capture(app, &plan.request)
            .and_then(|snapshot| snapshot.prepare(&AtomicBool::new(false)))
            .map_err(|e| e.to_string())?;
        execute_prepared_table_operation(app, plan, prepared, ExecutionAuthority::ReversibleModify)
            .map_err(|e| e.to_string())?;
        Ok(())
    } else {
        let after = prepare_table_cells(
            &table.typed_state,
            &[CellPatch {
                row,
                column,
                value: LiteralValue::Float64(FiniteOrSpecial::new(value)),
            }],
        )?;
        app.commit_table_revision(dataset.resource_id(), revision.id, after)
    }
}

fn assert_saveable(app: &PlotiumApp) {
    for snapshots in [false, true] {
        let path = std::env::temp_dir().join(format!(
            "plotium-table-update-{}.plotium",
            uuid::Uuid::new_v4()
        ));
        crate::project::save_project(app, &path, snapshots).unwrap();
        let reopened = crate::project::load_project(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(
            reopened.doc.canvases[0].objects[0].plot().unwrap().binding,
            app.doc.canvases[0].objects[0].plot().unwrap().binding
        );
    }
}

#[test]
fn sheet_and_automation_reject_invalid_bound_error_atomically() {
    for automation in [false, true] {
        let (mut app, _) = fixture(false);
        patch(&mut app, 0, 0.4, automation).unwrap();
        app.undo();
        let before = app.doc.datasets[0]
            .as_table()
            .unwrap()
            .typed_state
            .envelope
            .clone();
        let revision = app.doc.automation_revision;
        let figure = app.doc.canvases[0].objects[0]
            .plot()
            .unwrap()
            .figure()
            .clone();
        let error = patch(&mut app, 0, -1., automation).unwrap_err();
        assert!(
            error.contains("Y error must be a finite non-negative"),
            "{error}"
        );
        assert_eq!(
            app.doc.datasets[0].as_table().unwrap().typed_state.envelope,
            before
        );
        assert_eq!(app.doc.automation_revision, revision);
        assert!(!app.can_undo());
        assert!(app.can_redo());
        assert_eq!(
            app.doc.canvases[0].objects[0]
                .plot()
                .unwrap()
                .figure()
                .series[0]
                .points,
            figure.series[0].points
        );
        assert_saveable(&app);
        app.redo();
        assert!(app.can_undo());
        assert!(!app.can_redo());
        assert_saveable(&app);
        patch(&mut app, 0, 0., automation).unwrap();
        assert_saveable(&app);
    }
}

#[test]
fn refresh_rejects_invalid_bound_error_without_adopting_candidate() {
    let (mut app, derived) = fixture(true);
    // The unbound source accepts negative numeric values; the plotted result cannot.
    patch(&mut app, 0, -1., false).unwrap();
    let before = app.doc.datasets[derived]
        .as_table()
        .unwrap()
        .typed_state
        .envelope
        .clone();
    let revision = app.doc.automation_revision;
    let undo = app.session.undo_stack.len();
    let error = app
        .refresh_derived_table(derived, &[0], 16777216)
        .unwrap_err();
    assert!(
        error.contains("Y error must be a finite non-negative"),
        "{error}"
    );
    assert_eq!(
        app.doc.datasets[derived]
            .as_table()
            .unwrap()
            .typed_state
            .envelope,
        before
    );
    assert_eq!(app.doc.automation_revision, revision);
    assert_eq!(app.session.undo_stack.len(), undo);
    assert_saveable(&app);
    patch(&mut app, 0, 0.5, false).unwrap();
    app.refresh_derived_table(derived, &[0], 16777216).unwrap();
    assert_saveable(&app);
}
