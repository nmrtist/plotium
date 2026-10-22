//! Explicit bar curves: numeric and text-category x, grouping, and bar width.
use super::*;
use crate::automation::*;
use crate::properties::{
    AggregateValue, Availability, PropertyAddress, PropertyValue, line_display,
};
use plotium_data::{ColumnChunk, ColumnSchema, ColumnValues, LogicalType};
use serde_json::json;

const GROUP: usize = 0;
const X: usize = 1;
const Y: usize = 2;
const Y2: usize = 3;
const ERROR: usize = 4;
const GROUP2: usize = 5;
const GROUP3: usize = 6;

fn fixture() -> PlotiumApp {
    let mut app = PlotiumApp::new();
    let group = ColumnSchema::new("group", LogicalType::Utf8);
    let numeric = |name| ColumnSchema::new(name, LogicalType::Float64);
    let values = |v: [f64; 4]| ColumnChunk::optional_f64(v.map(Some));
    let table = TableDataset::from_materialized(
        vec![
            (
                group,
                ColumnChunk::all_valid(ColumnValues::Utf8(
                    ["b", "a", "b", "c"].map(String::from).to_vec(),
                )),
            ),
            (numeric("x"), values([1., 2., 3., 4.])),
            (numeric("y"), values([2., 3., 1., 5.])),
            (numeric("y2"), values([1., 2., -1., 3.])),
            (numeric("error"), values([0.1, 0.2, 0.3, 0.4])),
            (
                ColumnSchema::new("group2", LogicalType::Utf8),
                ColumnChunk::all_valid(ColumnValues::Utf8(
                    ["c", "b", "c", "a"].map(String::from).to_vec(),
                )),
            ),
            (
                ColumnSchema::new("group3", LogicalType::Utf8),
                ColumnChunk::all_valid(ColumnValues::Utf8(
                    ["a", "d", "d", "e"].map(String::from).to_vec(),
                )),
            ),
        ],
        vec![],
        None,
        vec![],
        "plotium.test.bars",
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

fn params(series: Vec<NewCurve>) -> PlotCreateParams {
    PlotCreateParams {
        parent: document_root_target(),
        name: "Bars".into(),
        canvas: None,
        panel_name: None,
        panel_frame_pt: None,
        series,
    }
}

fn curve(kind: CurveKind, source: TableCurveSource) -> NewCurve {
    NewCurve { kind, source }
}

fn plan(app: &PlotiumApp, series: Vec<NewCurve>) -> Result<CurveEditPlan, String> {
    app.plan_plot_creation(
        DocumentRevision(app.doc.automation_revision),
        &params(series),
    )
}

fn create(app: &mut PlotiumApp, series: Vec<NewCurve>) -> PlotCreationSummary {
    let planned = plan(app, series).unwrap();
    let summary = planned.creation.clone().unwrap();
    app.commit_curve_bindings(planned).unwrap();
    summary
}

fn figure(app: &PlotiumApp) -> &plotium_figure::Figure {
    app.doc.canvases[0].objects[0].plot().unwrap().figure()
}

/// `[x0, x1, y0, y1]` of an axis-aligned bar rectangle.
fn rect(polygon: &plotium_figure::Polygon) -> [f64; 4] {
    let xs = polygon.points.iter().map(|p| p[0]);
    let ys = polygon.points.iter().map(|p| p[1]);
    [
        xs.clone().fold(f64::INFINITY, f64::min),
        xs.fold(f64::NEG_INFINITY, f64::max),
        ys.clone().fold(f64::INFINITY, f64::min),
        ys.fold(f64::NEG_INFINITY, f64::max),
    ]
}

fn close(a: [f64; 4], b: [f64; 4]) -> bool {
    a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-9)
}

#[test]
fn numeric_bar_with_errors_lowers_to_rectangles_and_whiskers() {
    let mut app = fixture();
    let s = source(&app, X, Y, Some(ERROR));
    let summary = create(&mut app, vec![curve(CurveKind::Bar, s)]);
    assert_eq!(summary.series_defaults[0].kind, CurveKind::Bar);
    let fig = figure(&app);
    assert!(
        fig.series.is_empty(),
        "bars never reach renderers as series"
    );
    assert_eq!(fig.polygons.len(), 4);
    // 80% of the unit gap between x positions, from the zero baseline.
    assert!(close(rect(&fig.polygons[0]), [0.6, 1.4, 0., 2.]));
    assert!(close(rect(&fig.polygons[3]), [3.6, 4.4, 0., 5.]));
    assert_eq!(fig.polygons[0].name, "y");
    assert_eq!(fig.error_bars.len(), 4);
    assert!(fig.error_bars.iter().all(|e| e.draw_over_data));
    assert_eq!(fig.error_bars[1].center, [2., 3.]);
    assert_eq!(fig.y.min, 0.0, "the baseline is the automatic floor");
    assert!(fig.y.max > 5.4);
    assert!(fig.x.categories.is_none());
    // Default journal look: a 0.5 pt black outline, whiskers in the same ink,
    // caps spanning 45% of the bar.
    let black = plotium_figure::Color::BLACK;
    assert!(fig.polygons.iter().all(|p| p.stroke == Some((black, 0.5))));
    for error in &fig.error_bars {
        assert_eq!((error.color, error.width), (black, 0.5));
        assert!((error.cap_data_width.unwrap() - 0.45 * 0.8).abs() < 1e-9);
    }
}

#[test]
fn text_x_bars_use_category_slots_in_first_row_order() {
    let mut app = fixture();
    let s = source(&app, GROUP, Y, None);
    create(&mut app, vec![curve(CurveKind::Bar, s)]);
    let fig = figure(&app);
    assert_eq!(
        fig.x.categories.as_deref(),
        Some(&["b".to_owned(), "a".to_owned(), "c".to_owned()][..])
    );
    assert_eq!(fig.x.label, "group");
    let slots: Vec<f64> = fig
        .polygons
        .iter()
        .map(|p| (rect(p)[0] + rect(p)[1]) / 2.)
        .collect();
    assert_eq!(slots, vec![0., 1., 0., 2.]);
    assert!(close(rect(&fig.polygons[1]), [0.6, 1.4, 0., 3.]));
}

#[test]
fn whiskers_follow_their_bars_across_differently_ordered_categories() {
    let mut app = fixture();
    let (a, b) = (
        source(&app, GROUP, Y, Some(ERROR)),
        source(&app, GROUP2, Y2, Some(ERROR)),
    );
    create(
        &mut app,
        vec![curve(CurveKind::Bar, a), curve(CurveKind::Bar, b)],
    );
    let fig = figure(&app);
    assert_eq!(fig.x.categories.as_ref().unwrap(), &["b", "a", "c"]);
    // Second curve rows c, b, c, a land on slots 2, 0, 2, 1 (right half).
    let centers: Vec<f64> = fig.polygons[4..]
        .iter()
        .map(|p| (rect(p)[0] + rect(p)[1]) / 2.)
        .collect();
    assert_eq!(centers, vec![2.25, 0.25, 2.25, 1.25]);
    let whiskers: Vec<f64> = fig.error_bars.iter().map(|e| e.center[0]).collect();
    let bars: Vec<f64> = fig
        .polygons
        .iter()
        .map(|p| (rect(p)[0] + rect(p)[1]) / 2.)
        .collect();
    assert_eq!(whiskers, bars);
}

#[test]
fn a_curve_alone_in_a_slot_keeps_the_whole_slot() {
    let mut app = fixture();
    let (a, b) = (
        source(&app, GROUP2, Y, Some(ERROR)),
        source(&app, GROUP3, Y2, Some(ERROR)),
    );
    create(
        &mut app,
        vec![curve(CurveKind::Bar, a), curve(CurveKind::Bar, b)],
    );
    let fig = figure(&app);
    assert_eq!(
        fig.x.categories.as_ref().unwrap(),
        &["c", "b", "a", "d", "e"]
    );
    let centers: Vec<f64> = fig
        .polygons
        .iter()
        .map(|p| (rect(p)[0] + rect(p)[1]) / 2.)
        .collect();
    // Only "a" holds both curves; every other slot has one full-width bar.
    assert_eq!(centers, vec![0., 1., 0., 1.75, 2.25, 3., 3., 4.]);
    assert!(close(rect(&fig.polygons[1]), [0.6, 1.4, 0., 3.]));
    assert!(close(rect(&fig.polygons[7]), [3.6, 4.4, 0., 3.]));
    let whiskers: Vec<f64> = fig.error_bars.iter().map(|e| e.center[0]).collect();
    assert_eq!(whiskers, centers);
}

#[test]
fn bar_series_in_one_plot_are_grouped_side_by_side() {
    let mut app = fixture();
    let (a, b) = (source(&app, X, Y, None), source(&app, X, Y2, Some(ERROR)));
    create(
        &mut app,
        vec![curve(CurveKind::Bar, a), curve(CurveKind::Bar, b)],
    );
    let fig = figure(&app);
    assert_eq!(fig.polygons.len(), 8);
    // Each curve takes half of the unit slot and fills 80% of its half.
    assert!(close(rect(&fig.polygons[0]), [0.55, 0.95, 0., 2.]));
    assert!(close(rect(&fig.polygons[4]), [1.05, 1.45, 0., 1.]));
    assert!(close(rect(&fig.polygons[6]), [3.05, 3.45, -1., 0.]));
    assert!((fig.error_bars[0].center[0] - 1.25).abs() < 1e-9);
    assert!(fig.y.min < -1.3, "negative bars and whiskers stay in view");
}

#[test]
fn bars_mix_with_lines_only_on_a_numeric_x() {
    let app = fixture();
    let mixed = vec![
        curve(CurveKind::Bar, source(&app, X, Y, None)),
        curve(CurveKind::Line, source(&app, X, Y2, None)),
    ];
    assert!(plan(&app, mixed).is_ok());
    for series in [
        vec![
            curve(CurveKind::Bar, source(&app, GROUP, Y, None)),
            curve(CurveKind::Line, source(&app, GROUP, Y2, None)),
        ],
        vec![curve(CurveKind::Scatter, source(&app, GROUP, Y, None))],
    ] {
        let error = plan(&app, series).unwrap_err();
        assert!(error.contains("text x column needs bar series"), "{error}");
    }
    let error = plan(
        &app,
        vec![
            curve(CurveKind::Bar, source(&app, GROUP, Y, None)),
            curve(CurveKind::Bar, source(&app, X, Y2, None)),
        ],
    )
    .unwrap_err();
    assert!(error.contains("text or a numeric x"), "{error}");
    assert!(app.doc.canvases.is_empty());
}

fn set(app: &mut PlotiumApp, target: &TargetRef, key: &str, value: serde_json::Value) {
    let request = ToolRequest {
        tool_id: "properties.set".into(),
        tool_version: 1,
        parameters: json!({"key": key, "value": value}),
        targets: TargetSelector::Components {
            targets: vec![target.clone()],
        },
        expected_revision: DocumentRevision(app.doc.automation_revision),
        caller: CallerType::Agent,
    };
    let plan = plan_tool(app, request).unwrap();
    execute_tool(app, plan, ExecutionAuthority::ReversibleModify).unwrap();
}

#[test]
fn bar_width_and_color_are_series_properties() {
    let mut app = fixture();
    let s = source(&app, X, Y, None);
    let summary = create(&mut app, vec![curve(CurveKind::Bar, s)]);
    let target = summary.series_defaults[0].target.clone();
    let read = |app: &PlotiumApp, key| {
        app.resolve_property(&PropertyAddress::new(target.clone(), key))
            .unwrap()
    };
    let width = read(&app, line_display::BAR_WIDTH);
    assert_eq!(width.availability, Availability::Editable);
    assert_eq!(
        width.value,
        AggregateValue::Uniform(PropertyValue::Float(0.8))
    );
    for key in [line_display::MARKER, line_display::STYLE] {
        assert!(matches!(
            read(&app, key).availability,
            Availability::Disabled(_)
        ));
    }
    set(&mut app, &target, "series.bar.width", json!(0.5));
    set(&mut app, &target, "series.line.color", json!("#0072B2"));
    set(&mut app, &target, "series.bar.edge_color", json!("#404040"));
    let grey = plotium_figure::Color::rgb(0x40, 0x40, 0x40);
    assert_eq!(figure(&app).polygons[0].stroke, Some((grey, 0.5)));
    set(&mut app, &target, "series.bar.edge_width", json!(0.0));
    assert!(figure(&app).polygons.iter().all(|p| p.stroke.is_none()));
    let fig = figure(&app);
    assert!(close(rect(&fig.polygons[0]), [0.75, 1.25, 0., 2.]));
    assert_eq!(
        fig.polygons[0].fill,
        plotium_figure::Color::rgb(0, 0x72, 0xb2)
    );
    let reset = app
        .plan_property_reset(line_display::BAR_WIDTH, std::slice::from_ref(&target))
        .unwrap();
    app.commit_property(reset);
    assert!(close(rect(&figure(&app).polygons[0]), [0.6, 1.4, 0., 2.]));
    // Log scales would misdraw the zero baseline.
    assert!(crate::state::log_scale_unavailable(figure(&app), false).is_some());
}

#[test]
fn bar_settings_are_not_offered_for_lines() {
    let mut app = fixture();
    let s = source(&app, X, Y, None);
    let summary = create(&mut app, vec![curve(CurveKind::Line, s)]);
    for key in [
        line_display::BAR_WIDTH,
        line_display::BAR_EDGE_COLOR,
        line_display::BAR_EDGE_WIDTH,
    ] {
        let resolved = app
            .resolve_property(&PropertyAddress::new(
                summary.series_defaults[0].target.clone(),
                key,
            ))
            .unwrap();
        assert!(matches!(resolved.availability, Availability::Disabled(_)));
    }
}
