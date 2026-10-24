use super::*;
use crate::automation::*;
use crate::properties::{
    AxisRangeValue, PropertyAddress, PropertyError, PropertyId, PropertyValue, line_display,
    right_axis,
};
use crate::state::*;
use plotium_data::{ColumnChunk, ColumnSchema, LogicalType, UnitRegistry};
use plotium_figure::{AxisScale, Series};

fn two_axis_figure() -> Figure {
    let mut figure = Figure::new("", Axis::new("x", 0.0, 10.0), Axis::new("y", 0.0, 1000.0));
    figure
        .series
        .push(Series::line("left", vec![[0.0, 1.0], [10.0, 2.0]]));
    let mut right = Series::line("right", vec![[0.0, 100.0], [10.0, 1000.0]]);
    right.appearance.y_axis = YAxisSide::Right;
    figure.series.push(right);
    figure
}

#[test]
fn each_y_axis_fits_its_own_curves() {
    let mut figure = two_axis_figure();
    split_y_axes(&mut figure);
    let left = AxisRange::from_axis(&figure.y);
    let right = AxisRange::from_axis(figure.y2.as_ref().expect("right axis"));
    assert!(left.contains(1.0) && left.contains(2.0) && left.max < 3.0);
    assert!(right.contains(100.0) && right.contains(1000.0) && right.min > 0.0);

    figure.series[1].appearance.y_axis = YAxisSide::Left;
    split_y_axes(&mut figure);
    assert!(figure.y2.is_none(), "no right curve, no right axis");
}

#[test]
fn right_overrides_apply_only_to_the_right_axis() {
    let mut figure = two_axis_figure();
    split_y_axes(&mut figure);
    let left = AxisRange::from_axis(&figure.y);
    AxisOverrides {
        y2_label: Some("Rate (Hz)".into()),
        y2_scale: Some(AxisScale::Log10),
        y2_show_tick_labels: Some(false),
        ..AxisOverrides::default()
    }
    .apply_to(&mut figure);
    let right = figure.y2.as_ref().unwrap();
    assert_eq!(right.label, "Rate (Hz)");
    assert_eq!(right.scale, AxisScale::Log10);
    assert!(!right.show_tick_labels);
    // The automatic log window is refitted to the right curve's decades.
    assert!(right.min > 50.0 && right.min < 100.0 && right.max > 1000.0);
    assert_eq!(AxisRange::from_axis(&figure.y), left);
    assert_eq!(figure.y.scale, AxisScale::Linear);

    let mut manual = two_axis_figure();
    split_y_axes(&mut manual);
    AxisOverrides {
        y2_range: Some(AxisRange::new(0.0, 5000.0)),
        ..AxisOverrides::default()
    }
    .apply_to(&mut manual);
    assert_eq!(
        AxisRange::from_axis(manual.y2.as_ref().unwrap()),
        AxisRange::new(0.0, 5000.0)
    );
}

#[test]
fn y_navigation_moves_both_axes_by_the_same_fraction() {
    let mut figure = two_axis_figure();
    split_y_axes(&mut figure);
    let mut viewport = CanvasViewport::from_figure(&figure);
    let full_y2 = viewport.full_y2.unwrap();
    let fraction = |range: AxisRange, full: AxisRange| {
        [
            (range.min - full.min) / full.span(),
            (range.max - full.min) / full.span(),
        ]
    };
    // Zoom the left axis to its lower half around its minimum.
    viewport.zoom_y(&figure, viewport.full_y.min, 0.5);
    let left = fraction(viewport.view_y, viewport.full_y);
    let right = fraction(viewport.view_y2.unwrap(), full_y2);
    assert!((left[0] - right[0]).abs() < 1e-9 && (left[1] - right[1]).abs() < 1e-9);
    assert!((left[1] - 0.5).abs() < 1e-9);

    viewport.pan(&figure, 0.0, viewport.view_y.span() * 0.25);
    let left = fraction(viewport.view_y, viewport.full_y);
    let right = fraction(viewport.view_y2.unwrap(), full_y2);
    assert!(left[0] > 0.0 && (left[0] - right[0]).abs() < 1e-9);

    viewport.reset_all();
    assert_eq!(viewport.view_y2, Some(full_y2));
    viewport.apply_to(&mut figure);
    assert_eq!(AxisRange::from_axis(figure.y2.as_ref().unwrap()), full_y2);
}

#[test]
fn automatic_y_refits_the_right_axis_to_the_visible_x_window() {
    let mut figure = two_axis_figure();
    split_y_axes(&mut figure);
    let mut viewport = CanvasViewport::from_figure(&figure);
    viewport.select(&figure, Some(AxisRange::new(0.0, 1.0)), None);
    let right = viewport.view_y2.unwrap();
    assert!(right.contains(100.0) && right.max < 300.0, "{right:?}");

    // A manual right range is not replaced by automatic y.
    viewport.auto_y2 = false;
    viewport.reset_y(&figure);
    assert_eq!(viewport.view_y2, viewport.full_y2);
}

#[test]
fn legend_room_widens_the_right_axis_with_the_left_one() {
    let mut figure = Figure::new("", Axis::new("x", 0.0, 10.0), Axis::new("y", 0.0, 10.0));
    figure.width = 320.0;
    figure.height = 220.0;
    figure.guide_visibility = plotium_figure::GuideVisibility::Show;
    figure.series = (0..=20)
        .map(|i| {
            let y = f64::from(i) * 0.5;
            let name = if i < 2 {
                format!("S{i}")
            } else {
                String::new()
            };
            let mut series = Series::line(name, vec![[0.0, y], [10.0, y]]);
            if i % 2 == 1 {
                series.appearance.y_axis = YAxisSide::Right;
                series.points.iter_mut().for_each(|p| p[1] *= 100.0);
            }
            series
        })
        .collect();
    split_y_axes(&mut figure);
    let before = (
        AxisRange::from_axis(&figure.y),
        AxisRange::from_axis(figure.y2.as_ref().unwrap()),
    );
    assert!(plotium_render::make_legend_room(
        &mut figure,
        [320.0, 220.0],
        false,
        true,
        true
    ));
    let grown = |a: AxisRange, b: AxisRange| (b.span() - a.span()) / a.span();
    let left = grown(before.0, AxisRange::from_axis(&figure.y));
    let right = grown(before.1, AxisRange::from_axis(figure.y2.as_ref().unwrap()));
    assert!(
        left > 0.0 && (left - right).abs() < 1e-6,
        "{left} vs {right}"
    );
}

/// A table whose two y columns carry incompatible units, `%` and `Hz`.
fn table_app() -> PlotiumApp {
    let mut app = PlotiumApp::new();
    let units = UnitRegistry::plotium_v1();
    let column = |name: &str, unit: &str| {
        let mut schema = ColumnSchema::new(name, LogicalType::Float64);
        schema.unit = Some(units.resolve(unit).unwrap());
        schema
    };
    let values = |v: [f64; 3]| ColumnChunk::optional_f64(v.map(Some));
    let table = TableDataset::from_materialized(
        vec![
            (column("time", "s"), values([0., 1., 2.])),
            (column("yield", "%"), values([10., 20., 40.])),
            (column("rate", "Hz"), values([1000., 3000., 2000.])),
            (column("bars", "%"), values([1., 2., 3.])),
        ],
        vec![],
        None,
        vec![],
        "plotium.test.right_axis",
    )
    .unwrap();
    app.doc.datasets.push(Dataset::Table(Box::new(table)));
    app.clear_history();
    app
}

fn curve(app: &PlotiumApp, kind: CurveKind, y: usize, side: YAxisSide) -> NewCurve {
    let dataset = &app.doc.datasets[0];
    let revision = &dataset.as_table().unwrap().typed_state.envelope.revision;
    let columns = &revision.snapshot.schema.columns;
    NewCurve {
        kind,
        source: TableCurveSource {
            dataset_id: dataset.resource_id(),
            field_id: dataset.default_field_id().unwrap(),
            table_id: revision.table_id,
            table_revision: revision.id,
            x_column_id: columns[0].id,
            y_column_id: columns[y].id,
            y_error_column_id: None,
        },
        y_axis: side,
    }
}

fn create(app: &mut PlotiumApp, series: Vec<NewCurve>) -> Result<(), String> {
    let params = PlotCreateParams {
        parent: document_root_target(),
        name: "Two axes".into(),
        canvas: None,
        panel_name: None,
        panel_frame_pt: None,
        series,
    };
    let plan = app.plan_plot_creation(DocumentRevision(app.doc.automation_revision), &params)?;
    app.commit_curve_bindings(plan)
}

fn plot(app: &PlotiumApp) -> &PlotObject {
    app.doc.canvases[0].objects[0].plot().unwrap()
}

fn write(app: &mut PlotiumApp, target: TargetRef, id: PropertyId, value: PropertyValue) {
    let commit = app.plan_property_write(id, &[target], &value).unwrap();
    app.commit_property(commit);
}

#[test]
fn a_right_curve_keeps_its_own_unit_title_and_window() {
    let mut app = table_app();
    let curves = vec![
        curve(&app, CurveKind::Line, 1, YAxisSide::Left),
        curve(&app, CurveKind::Scatter, 2, YAxisSide::Right),
    ];
    create(&mut app, curves).unwrap();
    let plot = plot(&app);
    let coordinates = plot.binding.coordinates.as_ref().unwrap();
    assert_eq!(coordinates.y.as_ref().unwrap().display_unit, "%");
    assert_eq!(coordinates.y2.as_ref().unwrap().display_unit, "Hz");
    let figure = plot.figure();
    let right = figure.y2.as_ref().expect("right axis");
    assert_eq!(figure.y.label, "yield (%)");
    assert_eq!(right.label, "rate (Hz)");
    assert!(AxisRange::from_axis(right).contains(3000.0));
    assert!(AxisRange::from_axis(&figure.y).max < 100.0);
}

#[test]
fn bars_cannot_use_the_right_axis() {
    let mut app = table_app();
    let curves = vec![curve(&app, CurveKind::Bar, 3, YAxisSide::Right)];
    let error = create(&mut app, curves).unwrap_err();
    assert!(error.contains("bars"), "{error}");

    let curves = vec![
        curve(&app, CurveKind::Line, 1, YAxisSide::Left),
        curve(&app, CurveKind::Bar, 3, YAxisSide::Left),
    ];
    create(&mut app, curves).unwrap();
    let bar = app.series_targets(0, app.doc.canvases[0].objects[0].id)[1].clone();
    let planned = app.plan_property_write(
        line_display::Y_AXIS,
        &[bar],
        &PropertyValue::Enum(line_display::RIGHT),
    );
    assert!(planned.is_err() || planned.unwrap().applied.is_empty());
}

#[test]
fn moving_a_curve_right_creates_the_axis_and_its_properties() {
    let mut app = table_app();
    let curves = vec![
        curve(&app, CurveKind::Line, 1, YAxisSide::Left),
        curve(&app, CurveKind::Line, 2, YAxisSide::Left),
    ];
    // Two incompatible curves cannot share the left axis.
    assert!(create(&mut app, curves).is_err());
    let curves = vec![curve(&app, CurveKind::Line, 1, YAxisSide::Left)];
    create(&mut app, curves).unwrap();
    let object_id = app.doc.canvases[0].objects[0].id;
    let object = app.object_target(0, object_id).unwrap();
    let label = PropertyAddress::new(object.clone(), right_axis::Y2_LABEL);
    assert!(matches!(
        app.resolve_property(&label),
        Err(PropertyError::NotApplicable(_))
    ));

    let params = CurveAppendParams {
        plot: object.clone(),
        series: vec![curve(&app, CurveKind::Line, 2, YAxisSide::Right)],
    };
    let plan = app
        .plan_append_curves(DocumentRevision(app.doc.automation_revision), &params)
        .unwrap();
    app.commit_curve_bindings(plan).unwrap();
    let units = plot(&app).binding.coordinates.as_ref().unwrap();
    assert_eq!(units.y2.as_ref().unwrap().display_unit, "Hz");
    let resolved = app.resolve_property(&label).unwrap();
    assert_eq!(
        resolved.default_value,
        Some(PropertyValue::Text("rate (Hz)".into()))
    );

    write(
        &mut app,
        object.clone(),
        right_axis::Y2_UNIT,
        PropertyValue::Text("kHz".into()),
    );
    write(
        &mut app,
        object.clone(),
        right_axis::Y2_RANGE,
        PropertyValue::AxisRange(AxisRangeValue::Manual {
            min: 0.0,
            max: 5.0,
            unit: Some("kHz".into()),
        }),
    );
    let figure = plot(&app).figure();
    let right = figure.y2.as_ref().unwrap();
    assert_eq!(right.label, "rate (kHz)");
    assert_eq!(AxisRange::from_axis(right), AxisRange::new(0.0, 5000.0));
    assert!(AxisRange::from_axis(&figure.y).max < 100.0);

    // Back on the left the Hz curve cannot share the % axis.
    let rate = app.series_targets(0, object_id)[1].clone();
    let refused = app.plan_property_write(
        line_display::Y_AXIS,
        &[rate],
        &PropertyValue::Enum(line_display::LEFT),
    );
    assert!(refused.is_err() || refused.unwrap().applied.is_empty());
    assert!(plot(&app).figure().y2.is_some());
}

#[test]
fn moving_a_unitless_curve_right_fixes_the_right_axis_unit_once() {
    let mut app = table_app();
    let curves = vec![
        curve(&app, CurveKind::Line, 1, YAxisSide::Left),
        curve(&app, CurveKind::Line, 3, YAxisSide::Left),
    ];
    create(&mut app, curves).unwrap();
    let object_id = app.doc.canvases[0].objects[0].id;
    let original = plot(&app).figure().clone();
    let amplified = app.doc.canvases[0].objects[0].plot_mut().unwrap();
    amplified.viewport.auto_y = false;
    amplified.viewport.display_gain = 2.;
    amplified.apply_viewport();
    assert_eq!(plot(&app).figure().display_gain, 2.);
    assert_ne!(
        plot(&app).figure().series[0].points,
        original.series[0].points
    );
    let second = app.series_targets(0, object_id)[1].clone();
    write(
        &mut app,
        second,
        line_display::Y_AXIS,
        PropertyValue::Enum(line_display::RIGHT),
    );
    let units = plot(&app).binding.coordinates.as_ref().unwrap();
    assert_eq!(units.y2.as_ref().unwrap().display_unit, "%");
    let figure = plot(&app).figure();
    assert_eq!(plot(&app).viewport.display_gain, 1.);
    assert_eq!(figure.display_gain, 1.);
    assert!(!figure.supports_display_gain());
    for (actual, expected) in figure.series.iter().zip(&original.series) {
        assert_eq!(actual.points, expected.points);
    }
    assert_eq!(figure.y2.as_ref().unwrap().label, "bars (%)");
    assert!(figure.series[1].appearance.y_axis.is_right());

    let second = app.series_targets(0, object_id)[1].clone();
    write(
        &mut app,
        second,
        line_display::Y_AXIS,
        PropertyValue::Enum(line_display::LEFT),
    );
    assert!(plot(&app).figure().y2.is_none(), "the right axis goes away");
    assert_eq!(plot(&app).viewport.display_gain, 1.);
    assert_eq!(plot(&app).figure().display_gain, 1.);
    app.undo();
    assert!(plot(&app).figure().y2.is_some());
    assert_eq!(plot(&app).viewport.display_gain, 1.);
    app.redo();
    assert_eq!(plot(&app).viewport.display_gain, 1.);
}

#[test]
fn applying_a_stale_gain_to_a_two_axis_figure_restores_unamplified_geometry() {
    let mut figure = two_axis_figure();
    let original = figure.clone();
    let mut viewport = CanvasViewport::from_figure(&figure);
    viewport.display_gain = 2.;
    viewport.apply_to(&mut figure);
    assert_eq!(figure.display_gain, 2.);
    split_y_axes(&mut figure);
    viewport.apply_to(&mut figure);
    assert_eq!(figure.display_gain, 1.);
    for (actual, expected) in figure.series.iter().zip(&original.series) {
        assert_eq!(actual.points, expected.points);
    }
    viewport.sync_full_from(&figure);
    assert_eq!(viewport.display_gain, 1.);
}

#[test]
fn right_axis_overrides_survive_a_project_round_trip() {
    let mut app = table_app();
    let curves = vec![
        curve(&app, CurveKind::Line, 1, YAxisSide::Left),
        curve(&app, CurveKind::Line, 2, YAxisSide::Right),
    ];
    create(&mut app, curves).unwrap();
    let object = app
        .object_target(0, app.doc.canvases[0].objects[0].id)
        .unwrap();
    write(
        &mut app,
        object,
        right_axis::Y2_LABEL,
        PropertyValue::Text("Turnover".into()),
    );
    let path = std::env::temp_dir().join(format!("plotium-y2-{}.plotium", uuid::Uuid::new_v4()));
    crate::project::save_project(&app, &path, false).unwrap();
    let loaded = crate::project::load_project(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    let restored = loaded.doc.canvases[0].objects[0].plot().unwrap();
    assert_eq!(restored.binding, plot(&app).binding);
    let figure = restored.figure();
    assert_eq!(figure.y2.as_ref().unwrap().label, "Turnover");
    assert!(figure.series[1].appearance.y_axis.is_right());
    assert_eq!(restored.viewport.full_y2, plot(&app).viewport.full_y2);
}

fn diagnosed_codes(app: &PlotiumApp) -> Vec<String> {
    let canvas = &app.doc.canvases[0];
    let params = FigureDiagnosticParams {
        canvas: TargetRef::resource(canvas.resource_id.into()),
        output: DiagnosticOutput {
            size_mm: canvas.size_mm,
        },
        checks: all_checks(),
        targets: None,
        overlap_targets: vec![],
        view: DiagnosticView::Issues,
        detail: DiagnosticDetail::Compact,
        limit: 200,
    };
    let revision = DocumentRevision(app.doc.automation_revision);
    let mut scan = FigureDiagnosticScan::capture(app, revision, params).unwrap();
    let mut codes = Vec::new();
    loop {
        let page = scan.next_page(|| false).unwrap();
        codes.extend(
            page["items"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|row| row["code"].as_str().map(str::to_owned)),
        );
        if scan.is_complete() {
            return codes;
        }
    }
}

#[test]
fn diagnosis_flags_a_right_axis_without_a_visible_title() {
    let mut app = table_app();
    let curves = vec![
        curve(&app, CurveKind::Line, 1, YAxisSide::Left),
        curve(&app, CurveKind::Line, 2, YAxisSide::Right),
    ];
    create(&mut app, curves).unwrap();
    assert!(!diagnosed_codes(&app).contains(&"right_axis_untitled".to_owned()));
    let object = app
        .object_target(0, app.doc.canvases[0].objects[0].id)
        .unwrap();
    write(
        &mut app,
        object,
        right_axis::Y2_SHOW_LABEL,
        PropertyValue::Bool(false),
    );
    assert!(diagnosed_codes(&app).contains(&"right_axis_untitled".to_owned()));
}

#[test]
fn every_right_axis_property_reports_its_default_on_a_two_axis_plot() {
    let mut app = table_app();
    let curves = vec![
        curve(&app, CurveKind::Line, 1, YAxisSide::Left),
        curve(&app, CurveKind::Line, 2, YAxisSide::Right),
    ];
    create(&mut app, curves).unwrap();
    let object = app
        .object_target(0, app.doc.canvases[0].objects[0].id)
        .unwrap();
    for id in [
        right_axis::Y2_LABEL,
        right_axis::Y2_SHOW_TICK_LABELS,
        right_axis::Y2_SHOW_LABEL,
        right_axis::Y2_SCALE,
        right_axis::Y2_RANGE,
        right_axis::Y2_UNIT,
    ] {
        let resolved = app
            .resolve_property(&PropertyAddress::new(object.clone(), id))
            .unwrap_or_else(|error| panic!("{id} did not resolve: {error}"));
        assert!(resolved.default_value.is_some(), "{id} has no default");
    }
}

#[test]
fn dormant_right_overrides_never_block_edits_or_reopening() {
    let mut app = table_app();
    let curves = vec![
        curve(&app, CurveKind::Line, 1, YAxisSide::Left),
        curve(&app, CurveKind::Line, 3, YAxisSide::Right),
    ];
    create(&mut app, curves).unwrap();
    let object_id = app.doc.canvases[0].objects[0].id;
    let object = app.object_target(0, object_id).unwrap();
    write(
        &mut app,
        object.clone(),
        right_axis::Y2_RANGE,
        PropertyValue::AxisRange(AxisRangeValue::Manual {
            min: 0.0,
            max: 5.0,
            unit: Some("%".into()),
        }),
    );
    // The % curve leaves the right axis; its manual range stays dormant.
    let second = app.series_targets(0, object_id)[1].clone();
    write(
        &mut app,
        second,
        line_display::Y_AXIS,
        PropertyValue::Enum(line_display::LEFT),
    );
    assert!(plot(&app).figure().y2.is_none());
    write(
        &mut app,
        object.clone(),
        crate::properties::plot_display::Y_RANGE,
        PropertyValue::AxisRange(AxisRangeValue::Manual {
            min: 0.0,
            max: 50.0,
            unit: Some("%".into()),
        }),
    );
    let path = std::env::temp_dir().join(format!(
        "plotium-y2-dormant-{}.plotium",
        uuid::Uuid::new_v4()
    ));
    crate::project::save_project(&app, &path, false).unwrap();
    let loaded = crate::project::load_project(&path);
    std::fs::remove_file(path).unwrap();
    assert!(loaded.is_ok());

    // A Hz curve now opens the right axis with a new unit: the % range goes.
    let params = CurveAppendParams {
        plot: object,
        series: vec![curve(&app, CurveKind::Line, 2, YAxisSide::Right)],
    };
    let plan = app
        .plan_append_curves(DocumentRevision(app.doc.automation_revision), &params)
        .unwrap();
    app.commit_curve_bindings(plan).unwrap();
    assert_eq!(plot(&app).axis_overrides.y2_range, None);
    let right = plot(&app).figure().y2.clone().unwrap();
    assert!(AxisRange::from_axis(&right).contains(3000.0));
}

#[test]
fn bars_on_the_left_leave_the_right_axis_free_to_go_logarithmic() {
    let mut app = table_app();
    let curves = vec![
        curve(&app, CurveKind::Bar, 1, YAxisSide::Left),
        curve(&app, CurveKind::Line, 2, YAxisSide::Right),
    ];
    create(&mut app, curves).unwrap();
    let object = app
        .object_target(0, app.doc.canvases[0].objects[0].id)
        .unwrap();
    write(
        &mut app,
        object,
        right_axis::Y2_SCALE,
        PropertyValue::Enum(crate::properties::axis::LOG),
    );
    assert_eq!(
        plot(&app).figure().y2.as_ref().unwrap().scale,
        AxisScale::Log10
    );
}

#[test]
fn a_dormant_right_display_unit_passes_display_validation() {
    let units = UnitRegistry::plotium_v1();
    let overrides = AxisOverrides {
        y2_display_unit: Some(Box::new(units.resolve("kHz").unwrap())),
        ..AxisOverrides::default()
    };
    let figure = Figure::new("", Axis::new("x", 0.0, 1.0), Axis::new("y", 0.0, 1.0));
    let coordinates = CurveCoordinates::default();
    assert_eq!(
        super::super::axis_display::validate_display(&overrides, Some(&coordinates), &figure),
        Ok(())
    );
}
