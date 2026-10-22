use super::*;
use crate::automation::*;
use plotium_data::{ColumnChunk, ColumnId, ColumnSchema, LogicalType, RevisionId, UnitRegistry};
use plotium_figure::{SeriesEncoding, SeriesKind};
use serde_json::json;

pub(crate) fn fixture(multiple: bool) -> PlotiumApp {
    let mut app = PlotiumApp::new();
    let names = [
        ("x_s", "s"),
        ("y_mA", "mA"),
        ("error_mA", "mA"),
        ("x_V", "V"),
        ("other_mA", "mA"),
        ("bad_error", "mA"),
        ("y_A", "A"),
        ("x_ms", "ms"),
        ("error_A", "A"),
    ];
    let values = [
        vec![0., 1., 2.],
        vec![1., 2., 4.],
        vec![0.1, 0.2, 0.3],
        vec![0., 0.2, 0.4],
        vec![2., 3., 5.],
        vec![0.1, -0.1, 0.2],
        vec![0.002, 0.004, 0.008],
        vec![0., 1000., 2000.],
        vec![0.0001, 0.0002, 0.0003],
    ];
    let columns = names
        .into_iter()
        .zip(values)
        .map(|((name, unit), values)| {
            let mut c = ColumnSchema::new(name, LogicalType::Float64);
            c.unit = Some(UnitRegistry::plotium_v1().resolve(unit).unwrap());
            (c, ColumnChunk::optional_f64(values.into_iter().map(Some)))
        })
        .collect::<Vec<_>>();
    let ids = columns.iter().map(|c| c.0.id).collect::<Vec<_>>();
    let mut defaults = vec![TableSeriesBinding {
        value_column: ids[1],
        uncertainty_column: Some(ids[2]),
        fit: None,
    }];
    if multiple {
        defaults.push(TableSeriesBinding {
            value_column: ids[4],
            uncertainty_column: None,
            fit: None,
        });
    }
    let mut table =
        TableDataset::from_materialized(columns, vec![], Some(ids[0]), defaults, "plotium.test.r4")
            .unwrap();
    table.name = Some("R4 shared numeric table".into());
    table.board_sheet_visible = false;
    app.doc.datasets.push(Dataset::Table(Box::new(table)));
    let mut c = CanvasDocument::new("R4 explicit bindings".into(), [180., 90.]);
    for left in [8., 264.] {
        let id = c.allocate_object_id();
        let o = app.build_plot_object(
            0,
            ObjectFrame::new(left, 22., 235., 195.),
            id,
            "R4 curve".into(),
        );
        c.objects.push(o);
        c.create_panel_for_plot(id).unwrap();
    }
    app.doc.canvases.push(c);
    app.session.active_canvas = Some(0);
    app.set_active_dataset(Some(0));
    app
}
fn plot(app: &PlotiumApp, index: usize) -> &PlotObject {
    app.doc.canvases[0].objects[index].plot().unwrap()
}
fn table(app: &PlotiumApp) -> &TableDataset {
    app.doc.datasets[0].as_table().unwrap()
}
fn target(app: &PlotiumApp, object: usize, series: SeriesId) -> TargetRef {
    app.series_target(0, app.doc.canvases[0].objects[object].id, series)
        .unwrap()
}
fn source(app: &PlotiumApp, x: usize, y: usize, error: Option<usize>) -> TableCurveSource {
    let t = table(app);
    let r = &t.typed_state.envelope.revision;
    let c = &r.snapshot.schema.columns;
    TableCurveSource {
        dataset_id: t.resource_id,
        field_id: app.doc.datasets[0].default_field_id().unwrap(),
        table_id: r.table_id,
        table_revision: r.id,
        x_column_id: c[x].id,
        y_column_id: c[y].id,
        y_error_column_id: error.map(|i| c[i].id),
    }
}
fn bind(
    app: &mut PlotiumApp,
    object: usize,
    series: SeriesId,
    x: usize,
    y: usize,
    error: Option<usize>,
) {
    let e = CurveBindingEdit {
        target: target(app, object, series),
        source: source(app, x, y, error),
    };
    let p = app
        .plan_curve_bindings(DocumentRevision(app.doc.automation_revision), &[e])
        .unwrap();
    app.commit_curve_bindings(p).unwrap();
}

#[test]
fn curve_two_plots_use_independent_columns_and_keep_defaults_styles_and_frames() {
    let mut app = fixture(false);
    let ids = [
        plot(&app, 0).binding.series[0].id,
        plot(&app, 1).binding.series[0].id,
    ];
    let defaults = (table(&app).x_binding, table(&app).series_bindings.clone());
    let frame = app.doc.canvases[0].objects[0].frame;
    {
        let s = &mut app.doc.canvases[0].objects[0]
            .plot_mut()
            .unwrap()
            .binding
            .series[0];
        s.label = Some("Authored label".into());
        s.set_primary_color(plotium_figure::Color::BLACK);
    }
    bind(&mut app, 0, ids[0], 0, 1, Some(2));
    bind(&mut app, 1, ids[1], 3, 4, None);
    assert!((table(&app).x_binding, table(&app).series_bindings.clone()) == defaults);
    assert_eq!(app.doc.datasets.len(), 1);
    assert_eq!(app.doc.canvases[0].objects[0].frame, frame);
    assert_eq!(plot(&app, 0).figure().series[0].points[1], [1., 2.]);
    assert_eq!(plot(&app, 1).figure().series[0].points[1], [0.2, 3.]);
    assert_eq!(
        plot(&app, 0).binding.series[0].label.as_deref(),
        Some("Authored label")
    );
    assert_eq!(
        plot(&app, 0).figure().series[0].color,
        plotium_figure::Color::BLACK
    );
    let right = plot(&app, 1).binding.clone();
    bind(&mut app, 0, ids[0], 7, 6, Some(8));
    assert_eq!(plot(&app, 0).figure().series[0].points[1], [1., 4.]);
    assert_eq!(plot(&app, 1).binding, right);
    assert!((plot(&app, 0).figure().error_bars[1].positive - 0.2).abs() < 1e-12);
}
#[test]
fn curve_materialize_then_reorder_delete_and_rebind_keep_identity_units_and_undo() {
    let mut app = fixture(true);
    let id = plot(&app, 0).binding.series[0].id;
    let before = plot(&app, 0).binding.clone();
    let rejected = CurveBindingEdit {
        target: target(&app, 0, id),
        source: source(&app, 0, 1, None),
    };
    assert!(
        app.plan_curve_bindings(DocumentRevision(0), &[rejected])
            .unwrap_err()
            .contains("Materialize")
    );
    let y = table(&app).series_bindings[0].value_column;
    let p = app
        .plan_materialize_curves(
            DocumentRevision(0),
            &[MaterializeCurveEdit {
                target: target(&app, 0, id),
                retain_y_column_id: y,
                table_revision: table(&app).typed_state.envelope.revision.id,
            }],
        )
        .unwrap();
    app.commit_curve_bindings(p).unwrap();
    let ids = plot(&app, 0)
        .binding
        .series
        .iter()
        .map(|s| s.id)
        .collect::<Vec<_>>();
    assert_eq!(ids[0], id);
    assert_ne!(ids[0], ids[1]);
    let units = plot(&app, 0).binding.coordinates.clone();
    {
        let mut axes = plot(&app, 0).axis_overrides.clone();
        axes.x_range = Some(AxisRange { min: 0., max: 3. });
        let o = app.doc.canvases[0].objects[0].id;
        app.set_axis_overrides_value(0, o, &axes);
    }
    bind(&mut app, 0, ids[1], 7, 6, Some(8));
    let canvas = app.doc.canvases[0].resource_id;
    let object = app.doc.canvases[0].objects[0].id;
    let mut reordered = plot(&app, 0).binding.clone();
    reordered.series.reverse();
    let p = app
        .plan_curve_series_order(
            DocumentRevision(app.doc.automation_revision),
            canvas,
            object,
            reordered,
        )
        .unwrap();
    app.commit_curve_bindings(p).unwrap();
    let mut removed = plot(&app, 0).binding.clone();
    removed.series.remove(1);
    let p = app
        .plan_curve_series_order(
            DocumentRevision(app.doc.automation_revision),
            canvas,
            object,
            removed,
        )
        .unwrap();
    app.commit_curve_bindings(p).unwrap();
    assert_eq!(plot(&app, 0).binding.series[0].id, ids[1]);
    assert_eq!(plot(&app, 0).binding.coordinates, units);
    assert_eq!(plot(&app, 0).figure().x.max, 3.);
    assert_eq!(plot(&app, 0).figure().series[0].points[1], [1., 4.]);
    app.undo();
    assert_eq!(plot(&app, 0).binding.series.len(), 2);
    app.redo();
    assert_eq!(plot(&app, 0).binding.series[0].id, ids[1]);
    app.undo();
    app.undo();
    app.undo();
    app.undo();
    assert_eq!(plot(&app, 0).binding, before);
    app.redo();
    assert_eq!(
        plot(&app, 0)
            .binding
            .series
            .iter()
            .map(|s| s.id)
            .collect::<Vec<_>>(),
        ids
    );
}
#[test]
fn curve_invalid_batches_stale_revisions_types_units_and_negative_errors_are_atomic() {
    let mut app = fixture(false);
    let id = plot(&app, 0).binding.series[0].id;
    let initial = plot(&app, 0).binding.clone();
    let valid = CurveBindingEdit {
        target: target(&app, 0, id),
        source: source(&app, 0, 1, None),
    };
    let mut invalid = CurveBindingEdit {
        target: target(&app, 1, id),
        source: source(&app, 3, 4, None),
    };
    invalid.source.y_column_id = ColumnId::new();
    assert!(
        app.plan_curve_bindings(DocumentRevision(0), &[valid.clone(), invalid])
            .is_err()
    );
    assert_eq!(plot(&app, 0).binding, initial);
    assert!(!app.can_undo());
    let bad = CurveBindingEdit {
        target: target(&app, 0, id),
        source: source(&app, 0, 1, Some(5)),
    };
    assert!(
        app.plan_curve_bindings(DocumentRevision(0), &[bad])
            .unwrap_err()
            .contains("non-negative")
    );
    let p = app
        .plan_curve_bindings(DocumentRevision(0), &[valid])
        .unwrap();
    app.commit_curve_bindings(p.clone()).unwrap();
    assert!(app.commit_curve_bindings(p).is_err());
    let unit = CurveBindingEdit {
        target: target(&app, 0, id),
        source: source(&app, 3, 4, None),
    };
    assert!(
        app.plan_curve_bindings(DocumentRevision(app.doc.automation_revision), &[unit])
            .unwrap_err()
            .contains("units")
    );
    let mut stale = CurveBindingEdit {
        target: target(&app, 0, id),
        source: source(&app, 0, 1, None),
    };
    stale.source.table_revision = RevisionId::new();
    assert!(
        app.plan_curve_bindings(DocumentRevision(app.doc.automation_revision), &[stale])
            .is_err()
    );
    let mut forged = target(&app, 0, id);
    forged.resource.parent_id = None;
    assert!(app.curve_target(&forged).is_err());
}
#[test]
fn curve_null_nonfinite_and_error_null_preserve_gaps_and_report_rows() {
    let make = |name: &str, values: Vec<Option<f64>>| {
        let mut c = ColumnSchema::new(name, LogicalType::Float64);
        c.nullable = true;
        (c, ColumnChunk::optional_f64(values))
    };
    let columns = vec![
        make("x", vec![Some(0.), Some(1.), Some(2.), Some(3.), Some(4.)]),
        make(
            "y",
            vec![Some(1.), None, Some(3.), Some(f64::INFINITY), Some(5.)],
        ),
        make("e", vec![Some(0.), Some(0.1), None, Some(0.2), Some(0.3)]),
    ];
    let ids = columns.iter().map(|c| c.0.id).collect::<Vec<_>>();
    let table = TableDataset::from_materialized(columns, vec![], None, vec![], "plotium.test.gaps")
        .unwrap();
    let c = TableCurveColumns {
        table_id: table.typed_state.envelope.revision.table_id,
        x_column_id: ids[0],
        y_column_id: ids[1],
        y_error_column_id: Some(ids[2]),
    };
    let data = extract_table_curve(&table, c, &CurveCoordinates::default()).unwrap();
    assert_eq!(data.runs.len(), 3);
    assert_eq!(data.diagnostics.total_rows, 5);
    assert_eq!(data.diagnostics.plotted_rows, 3);
    assert_eq!(data.diagnostics.missing_xy, 1);
    assert_eq!(data.diagnostics.non_finite_xy, 1);
    assert_eq!(data.diagnostics.missing_error, 1);
    assert_eq!(data.errors.len(), 2);
    assert_eq!(data.errors[0].positive, 0.);
}
#[test]
fn curve_v1_roundtrip_preserves_sources_ids_units_and_actual_figure() {
    let mut app = fixture(false);
    let id = plot(&app, 0).binding.series[0].id;
    bind(&mut app, 0, id, 0, 1, Some(2));
    bind(&mut app, 1, id, 3, 4, None);
    let path = std::env::temp_dir().join(format!("plotium-r4-{}.plotium", uuid::Uuid::new_v4()));
    crate::project::save_project(&app, &path, false).unwrap();
    let loaded = crate::project::load_project(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(plot(&loaded, 0).binding, plot(&app, 0).binding);
    assert_eq!(plot(&loaded, 1).binding, plot(&app, 1).binding);
    assert_eq!(
        plot(&loaded, 0).figure().series[0].points,
        plot(&app, 0).figure().series[0].points
    );
}
#[test]
fn curve_automation_inspects_explicit_sources_and_requires_fresh_plan() {
    let mut app = fixture(false);
    let id = plot(&app, 0).binding.series[0].id;
    let t = target(&app, 0, id);
    let req = ToolRequest {
        tool_id: CURVE_BIND.into(),
        tool_version: 1,
        parameters: json!({"edits":[{"target":t,"source":source(&app,0,1,Some(2))}]}),
        targets: TargetSelector::Explicit { ids: vec![] },
        expected_revision: DocumentRevision(0),
        caller: CallerType::Agent,
    };
    let p = plan_tool(&app, req).unwrap();
    assert!(p.binding_plan.is_some());
    assert!(execute_tool(&mut app, p.clone(), ExecutionAuthority::Read).is_err());
    execute_tool(&mut app, p, ExecutionAuthority::ReversibleModify).unwrap();
    let req = ToolRequest {
        tool_id: DOCUMENT_INSPECT.into(),
        tool_version: 1,
        parameters: json!({"target":t,"view":"self"}),
        targets: TargetSelector::Components { targets: vec![t] },
        expected_revision: DocumentRevision(app.doc.automation_revision),
        caller: CallerType::Agent,
    };
    let p = plan_tool(&app, req).unwrap();
    let r = execute_tool(&mut app, p, ExecutionAuthority::Read).unwrap();
    assert_eq!(r.value["node"]["source"]["binding_mode"], "table_explicit");
    assert!(r.value["node"]["source"].get("series_columns").is_none());
}
#[test]
#[ignore = "Writes a synthetic local acceptance project only when explicitly requested"]
fn curve_write_acceptance_fixture() {
    let mut app = fixture(false);
    let id = plot(&app, 0).binding.series[0].id;
    bind(&mut app, 0, id, 0, 1, Some(2));
    bind(&mut app, 1, id, 3, 4, None);
    let p = app.doc.canvases[0].objects[0].plot_mut().unwrap();
    if let SeriesEncoding::Line(line) = &mut p.binding.series[0].encoding {
        line.kind = SeriesKind::Line;
    }
    app.rebuild_canvas(0);
    app.clear_history();
    let defaults = fixture(true);
    crate::project::save_project(
        &defaults,
        std::path::Path::new("/tmp/plotium-r4-defaults.plotium"),
        false,
    )
    .unwrap();
    let path = std::path::Path::new("/tmp/plotium-r4-acceptance.plotium");
    crate::project::save_project(&app, path, false).unwrap();
}

#[path = "curve_edge_tests.rs"]
mod edge;
