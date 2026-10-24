//! Category-axis explicit charts: overlaid points, value labels, horizontal
//! bars, and box/violin summaries of raw samples.
use super::*;
use crate::automation::*;
use crate::properties::{Availability, PropertyAddress, chart_layout};
use plotium_data::{ColumnChunk, ColumnSchema, ColumnValues, LogicalType};
use plotium_figure::TextAnchor;
use serde_json::json;

const GROUP: usize = 0;
const Y: usize = 1;
const Y2: usize = 2;
const ERROR: usize = 3;

/// Three replicates in each of groups a and b, in long format.
fn fixture() -> PlotiumApp {
    let mut app = PlotiumApp::new();
    let numeric = |name: &str, v: [f64; 6]| {
        (
            ColumnSchema::new(name, LogicalType::Float64),
            ColumnChunk::optional_f64(v.map(Some)),
        )
    };
    let table = TableDataset::from_materialized(
        vec![
            (
                ColumnSchema::new("group", LogicalType::Utf8),
                ColumnChunk::all_valid(ColumnValues::Utf8(
                    ["a", "a", "a", "b", "b", "b"].map(String::from).to_vec(),
                )),
            ),
            numeric("y", [2.0, 3.0, 4.0, -1.0, -2.0, -3.0]),
            numeric("y2", [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]),
            numeric("error", [0.5; 6]),
        ],
        vec![],
        None,
        vec![],
        "plotium.test.category",
    )
    .unwrap();
    app.doc.datasets.push(Dataset::Table(Box::new(table)));
    app.clear_history();
    app
}

fn source(app: &PlotiumApp, x: usize, y: usize, error: Option<usize>) -> TableCurveSource {
    let dataset = &app.doc.datasets[0];
    let r = &dataset.as_table().unwrap().typed_state.envelope.revision;
    let c = &r.snapshot.schema.columns;
    TableCurveSource {
        dataset_id: dataset.resource_id(),
        field_id: dataset.default_field_id().unwrap(),
        table_id: r.table_id,
        table_revision: r.id,
        x_column_id: c[x].id,
        y_column_id: c[y].id,
        y_error_column_id: error.map(|i| c[i].id),
    }
}

fn curve(kind: CurveKind, source: TableCurveSource) -> NewCurve {
    NewCurve {
        kind,
        source,
        y_axis: plotium_figure::YAxisSide::Left,
    }
}

fn plan(app: &PlotiumApp, series: Vec<NewCurve>) -> Result<CurveEditPlan, String> {
    let params = PlotCreateParams {
        parent: document_root_target(),
        name: "Categories".into(),
        canvas: None,
        panel_name: None,
        panel_frame_pt: None,
        series,
    };
    app.plan_plot_creation(DocumentRevision(app.doc.automation_revision), &params)
}

/// Create a plot from curves built against the current table revision.
fn create(
    app: &mut PlotiumApp,
    build: impl FnOnce(&PlotiumApp) -> Vec<NewCurve>,
) -> PlotCreationSummary {
    let series = build(app);
    let planned = plan(app, series).unwrap();
    let summary = planned.creation.clone().unwrap();
    app.commit_curve_bindings(planned).unwrap();
    summary
}

fn figure(app: &PlotiumApp) -> &plotium_figure::Figure {
    app.doc.canvases[0].objects[0].plot().unwrap().figure()
}

fn center(polygon: &plotium_figure::Polygon) -> f64 {
    let xs = polygon.points.iter().map(|p| p[0]);
    (xs.clone().fold(f64::INFINITY, f64::min) + xs.fold(f64::NEG_INFINITY, f64::max)) / 2.0
}

fn set(app: &mut PlotiumApp, target: &TargetRef, key: &str, value: serde_json::Value) {
    let targets = match target.component {
        Some(_) => TargetSelector::Components {
            targets: vec![target.clone()],
        },
        None => TargetSelector::Explicit {
            ids: vec![target.resource.id.clone()],
        },
    };
    let request = ToolRequest {
        tool_id: "properties.set".into(),
        tool_version: 1,
        parameters: json!({"key": key, "value": value}),
        targets,
        expected_revision: DocumentRevision(app.doc.automation_revision),
        caller: CallerType::Agent,
    };
    let plan = plan_tool(app, request).unwrap();
    execute_tool(app, plan, ExecutionAuthority::ReversibleModify).unwrap();
}

#[test]
fn points_on_a_category_axis_line_up_with_their_bars() {
    let mut app = fixture();
    create(&mut app, |app| {
        vec![
            curve(CurveKind::Bar, source(app, GROUP, Y, None)),
            curve(CurveKind::Bar, source(app, GROUP, Y2, None)),
            curve(CurveKind::Scatter, source(app, GROUP, Y2, None)),
        ]
    });
    let fig = figure(&app);
    // The scatter is the first overlay, so it sits on the first bar curve.
    let first_bar = center(&fig.polygons[0]);
    let points = &fig.series[0].points;
    assert!(points[..3].iter().all(|p| (p[0] - first_bar).abs() < 1e-9));
    assert!((first_bar + 0.25).abs() < 1e-9, "left half of slot a");
}

#[test]
fn replicate_spread_is_deterministic_and_reset_to_a_line() {
    let mut app = fixture();
    let summary = create(&mut app, |app| {
        vec![
            curve(CurveKind::Bar, source(app, GROUP, Y2, None)),
            curve(CurveKind::Scatter, source(app, GROUP, Y2, None)),
        ]
    });
    let points = summary.series_defaults[1].target.clone();
    set(&mut app, &points, "series.points.spread", json!(0.5));
    // Three replicates across half of the 0.8-wide bar, in row order.
    let xs: Vec<f64> = figure(&app).series[0].points[..3]
        .iter()
        .map(|p| p[0])
        .collect();
    for (x, expected) in xs.iter().zip([-0.2, 0.0, 0.2]) {
        assert!((x - expected).abs() < 1e-9, "{xs:?}");
    }
    set(&mut app, &points, "series.points.spread", json!(0.0));
    assert!(
        figure(&app).series[0].points[..3]
            .iter()
            .all(|p| p[0].abs() < 1e-9)
    );
}

#[test]
fn value_labels_sit_past_the_bar_and_its_whisker() {
    let mut app = fixture();
    let summary = create(&mut app, |app| {
        vec![curve(CurveKind::Bar, source(app, GROUP, Y, Some(ERROR)))]
    });
    set(
        &mut app,
        &summary.series_defaults[0].target,
        "series.value_labels",
        json!(true),
    );
    let labels = &figure(&app).annotations;
    assert_eq!(labels.len(), 6);
    assert_eq!(
        (labels[0].text.as_str(), labels[0].anchor),
        ("2", TextAnchor::Above)
    );
    assert!((labels[0].at[1] - 2.5).abs() < 1e-9, "past the 0.5 whisker");
    assert_eq!(
        (labels[3].text.as_str(), labels[3].anchor),
        ("-1", TextAnchor::Below)
    );
    assert!((labels[3].at[1] + 1.5).abs() < 1e-9);
    assert_eq!(super::curve_layout::format_value(10.43), "10.4");
    assert_eq!(super::curve_layout::format_value(0.0507), "0.0507");
    assert_eq!(super::curve_layout::format_value(45.75108), "45.8");
    assert_eq!(super::curve_layout::format_value(123456.0), "1.23e5");
    assert_eq!(super::curve_layout::format_value(12345.0), "12300");
    assert_eq!(super::curve_layout::format_value(1234.5), "1230");
    assert_eq!(super::curve_layout::format_value(0.0004), "4e-4");
}

#[test]
fn horizontal_orientation_swaps_axes_whiskers_and_labels() {
    let mut app = fixture();
    let summary = create(&mut app, |app| {
        vec![curve(CurveKind::Bar, source(app, GROUP, Y2, Some(ERROR)))]
    });
    let bar = summary.series_defaults[0].target.clone();
    set(&mut app, &bar, "series.value_labels", json!(true));
    set(
        &mut app,
        &summary.plot,
        "object.chart.orientation",
        json!("horizontal"),
    );
    let fig = figure(&app);
    assert_eq!(
        fig.y.categories.as_deref(),
        Some(&["a".to_owned(), "b".to_owned()][..])
    );
    assert!(fig.y.reversed, "the first category reads at the top");
    assert!(fig.x.categories.is_none());
    // Bars run along x from zero; their slot is on y.
    let ys: Vec<f64> = fig.polygons[0].points.iter().map(|p| p[1]).collect();
    assert!(ys.iter().all(|y| (-0.41..0.41).contains(y)), "{ys:?}");
    assert!(fig.error_bars.iter().all(|e| e.horizontal));
    assert!(
        fig.annotations
            .iter()
            .all(|a| a.anchor == TextAnchor::Right)
    );
    // The view follows the new value axis rather than the old category range.
    assert!(fig.x.max > 6.5, "x max {}", fig.x.max);
    set(
        &mut app,
        &summary.plot,
        "object.chart.orientation",
        json!("vertical"),
    );
    assert!(figure(&app).x.categories.is_some());
}

#[test]
fn box_and_violin_summarise_raw_samples_per_category() {
    let mut app = fixture();
    create(&mut app, |app| {
        vec![curve(CurveKind::Box, source(app, GROUP, Y2, None))]
    });
    let fig = figure(&app);
    // One box per category, named for the legend, centred on its slot.
    assert_eq!(fig.polygons.len(), 2);
    assert_eq!(fig.polygons[0].name, "y2");
    assert!((center(&fig.polygons[1]) - 1.0).abs() < 1e-9);

    let mut app = fixture();
    create(&mut app, |app| {
        vec![curve(CurveKind::Violin, source(app, GROUP, Y2, None))]
    });
    assert!(figure(&app).polygons.len() > 2);

    let app = fixture();
    let error = plan(
        &app,
        vec![curve(CurveKind::Box, source(&app, GROUP, Y2, Some(ERROR)))],
    )
    .unwrap_err();
    assert!(error.contains("no y error column"), "{error}");
}

#[test]
fn layout_properties_apply_only_where_they_draw() {
    let mut app = fixture();
    let summary = create(&mut app, |app| {
        vec![
            curve(CurveKind::Bar, source(app, GROUP, Y, None)),
            curve(CurveKind::Box, source(app, GROUP, Y2, None)),
        ]
    });
    let read = |key, target: &TargetRef| {
        app.resolve_property(&PropertyAddress::new(target.clone(), key))
            .unwrap()
            .availability
    };
    let (bar, boxes) = (
        &summary.series_defaults[0].target,
        &summary.series_defaults[1].target,
    );
    assert!(matches!(
        read(chart_layout::SPREAD, bar),
        Availability::Disabled(_)
    ));
    assert_eq!(
        read(chart_layout::VALUE_LABELS, bar),
        Availability::Editable
    );
    assert!(matches!(
        read(chart_layout::VALUE_LABELS, boxes),
        Availability::Disabled(_)
    ));
    assert_eq!(
        read(chart_layout::ORIENTATION, &summary.plot),
        Availability::Editable
    );
}

#[test]
fn a_horizontal_plot_keeps_one_value_axis() {
    let mut app = fixture();
    let summary = create(&mut app, |app| {
        vec![
            curve(CurveKind::Bar, source(app, GROUP, Y, None)),
            curve(CurveKind::Scatter, source(app, GROUP, Y2, None)),
        ]
    });
    let points = summary.series_defaults[1].target.clone();
    let read = |app: &PlotiumApp, key, target: &TargetRef| {
        app.resolve_property(&PropertyAddress::new(target.clone(), key))
            .unwrap()
            .availability
    };
    set(&mut app, &points, "series.y_axis", json!("right"));
    assert!(matches!(
        read(&app, chart_layout::ORIENTATION, &summary.plot),
        Availability::Disabled(_)
    ));
    set(&mut app, &points, "series.y_axis", json!("left"));
    set(
        &mut app,
        &summary.plot,
        "object.chart.orientation",
        json!("horizontal"),
    );
    assert!(matches!(
        read(&app, crate::properties::line_display::Y_AXIS, &points),
        Availability::Disabled(_)
    ));
    assert!(figure(&app).y2.is_none());
}

#[test]
fn turning_a_plot_carries_author_titles_ranges_and_annotations() {
    let mut app = fixture();
    let summary = create(&mut app, |app| {
        vec![curve(CurveKind::Bar, source(app, GROUP, Y2, None))]
    });
    set(
        &mut app,
        &summary.plot,
        "object.axes.x_label",
        json!("Group"),
    );
    set(
        &mut app,
        &summary.plot,
        "object.axes.y_range",
        json!({"mode":"manual","min":0.0,"max":10.0,"unit":null}),
    );
    set(
        &mut app,
        &summary.plot,
        "object.chart.orientation",
        json!("horizontal"),
    );
    let plot = app.doc.canvases[0].objects[0].plot().unwrap();
    assert_eq!(plot.axis_overrides.y_label.as_deref(), Some("Group"));
    assert_eq!(
        plot.axis_overrides.x_range.map(|r| (r.min, r.max)),
        Some((0.0, 10.0))
    );
    assert_eq!((plot.figure().x.min, plot.figure().x.max), (0.0, 10.0));
    app.undo();
    let plot = app.doc.canvases[0].objects[0].plot().unwrap();
    assert_eq!(plot.axis_overrides.x_label.as_deref(), Some("Group"));
    assert!(plot.figure().x.categories.is_some());
}

#[test]
fn category_chart_settings_survive_save_and_load() {
    let mut app = fixture();
    let summary = create(&mut app, |app| {
        vec![
            curve(CurveKind::Violin, source(app, GROUP, Y2, None)),
            curve(CurveKind::Box, source(app, GROUP, Y, None)),
            curve(CurveKind::Scatter, source(app, GROUP, Y2, None)),
        ]
    });
    let points = summary.series_defaults[2].target.clone();
    set(&mut app, &points, "series.points.spread", json!(0.4));
    set(&mut app, &points, "series.value_labels", json!(true));
    set(
        &mut app,
        &summary.plot,
        "object.chart.orientation",
        json!("horizontal"),
    );
    let path =
        std::env::temp_dir().join(format!("plotium-category-{}.plotium", uuid::Uuid::new_v4()));
    crate::project::save_project(&app, &path, false).unwrap();
    let loaded = crate::project::load_project(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    let before = &app.doc.canvases[0].objects[0].plot().unwrap().binding;
    let after = &loaded.doc.canvases[0].objects[0].plot().unwrap().binding;
    assert!(after.coordinates.as_ref().unwrap().transposed);
    let encodings = |b: &DataBinding| {
        b.series
            .iter()
            .map(|s| s.encoding.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(encodings(after), encodings(before));
}

#[test]
fn offset_numeric_value_labels_stay_on_their_traces_after_gain_and_project_restore() {
    let mut app = fixture();
    let summary = create(&mut app, |app| {
        vec![
            curve(CurveKind::Line, source(app, Y2, Y, Some(ERROR))),
            curve(CurveKind::Scatter, source(app, Y2, Y, Some(ERROR))),
        ]
    });
    for series in summary.series_defaults {
        set(&mut app, &series.target, "series.value_labels", json!(true));
    }
    let plot = app.doc.canvases[0].objects[0].plot_mut().unwrap();
    plot.stack.mode = StackMode::Offset;
    plot.stack.spacing_y = 1.;
    app.rebuild_canvas(0);
    let original = figure(&app).clone();
    assert!(original.supports_display_gain());
    assert!(original.series[1].display_baseline > 0.);
    let check_labels = |figure: &plotium_figure::Figure| {
        let mut labels = figure.annotations.iter();
        for series in &figure.series {
            for point in &series.points {
                let label = labels.next().unwrap();
                assert_eq!(label.display_baseline, series.display_baseline);
                assert_eq!(label.at, *point);
            }
        }
        assert!(labels.next().is_none());
    };
    check_labels(&original);
    let plot = app.doc.canvases[0].objects[0].plot_mut().unwrap();
    plot.viewport.auto_y = false;
    plot.viewport.display_gain = 2.;
    plot.apply_viewport();
    check_labels(figure(&app));
    for snapshots in [false, true] {
        let path = std::env::temp_dir().join(format!(
            "plotium-stacked-labels-{}.plotium",
            uuid::Uuid::new_v4()
        ));
        crate::project::save_project(&app, &path, snapshots).unwrap();
        let mut loaded = crate::project::load_project(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(figure(&loaded).display_gain, 2.);
        check_labels(figure(&loaded));
        let plot = loaded.doc.canvases[0].objects[0].plot_mut().unwrap();
        plot.viewport.display_gain = 1.;
        plot.apply_viewport();
        check_labels(figure(&loaded));
        for (actual, expected) in figure(&loaded)
            .annotations
            .iter()
            .zip(&original.annotations)
        {
            assert_eq!(actual.at, expected.at);
            assert_eq!(actual.text, expected.text);
        }
    }
}
