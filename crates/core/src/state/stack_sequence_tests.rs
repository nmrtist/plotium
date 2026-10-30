use crate::actions::Action;
use crate::state::{
    CanvasDocument, ChartSpec, Dataset, FloatSeries, ObjectFrame, PlotiumApp, StackMode, StackSpec,
    materialized_float_series_table,
};
use plotium_figure::{ColormapId, ErrorBar, Figure, GuideVisibility, Series, SeriesKind};

fn table_app(names: &[&str]) -> PlotiumApp {
    let series = names
        .iter()
        .enumerate()
        .map(|(column, name)| FloatSeries {
            name: (*name).to_owned(),
            unit: String::new(),
            values: vec![Some(1.0), Some(2.0 + column as f64), Some(1.0)],
            uncertainty: None,
            fit: None,
        })
        .collect();
    let table = materialized_float_series_table(
        (
            "2θ".into(),
            "°".into(),
            vec![Some(20.0), Some(21.0), Some(22.0)],
        ),
        series,
        "plotium.test.waterfall-table.v1",
    )
    .unwrap();
    let mut app = PlotiumApp::new();
    app.doc.datasets.push(Dataset::Table(Box::new(table)));
    let mut canvas = CanvasDocument::new("table".to_owned(), [120.0, 80.0]);
    let [w, h] = canvas.size_pt();
    let id = canvas.allocate_object_id();
    let object =
        app.build_plot_object(0, ObjectFrame::new(0.0, 0.0, w, h), id, "Plot 1".to_owned());
    canvas.objects.push(object);
    app.doc.canvases.push(canvas);
    app.focus_single(0);
    app.session.active_canvas = Some(0);
    app
}

fn stacked(app: &mut PlotiumApp, after: StackSpec) -> Figure {
    let object = &app.doc.canvases[0].objects[0];
    let (id, before) = (object.id, object.plot().unwrap().stack);
    app.execute_action(Action::set_stack_spec(0, id, before, after));
    app.doc.canvases[0].objects[0]
        .plot()
        .unwrap()
        .figure()
        .clone()
}

fn waterfall() -> StackSpec {
    StackSpec {
        mode: StackMode::Offset,
        spacing_y: 0.5,
        occlude: true,
        gradient: Some(ColormapId::Viridis),
        sequence_scale: true,
        ..StackSpec::default()
    }
}

#[test]
fn an_offset_table_stacks_its_columns_as_traces() {
    let mut app = table_app(&["0 min", "1 min", "4 min"]);
    let figure = stacked(&mut app, waterfall());
    assert_eq!(figure.series.len(), 3);
    let peak = 4.0;
    let baselines: Vec<f64> = figure.series.iter().map(|s| s.display_baseline).collect();
    assert_eq!(baselines, [0.0, 0.5 * peak, peak]);
    assert_eq!(figure.series[1].points[1][1], 3.0 + 0.5 * peak);
    assert!(
        figure
            .series
            .iter()
            .all(|series| { series.occludes && series.kind == SeriesKind::Line })
    );
    assert_eq!(figure.series_in_paint_order().next().unwrap().0, 2);
}

#[test]
fn a_superimposed_table_keeps_its_plain_line_chart() {
    let mut app = table_app(&["0 min", "1 min"]);
    let figure = stacked(
        &mut app,
        StackSpec {
            mode: StackMode::Superimposed,
            ..waterfall()
        },
    );
    assert!(
        figure
            .series
            .iter()
            .all(|series| { series.display_baseline == 0.0 && !series.occludes })
    );
    assert!(figure.sequence_scale.is_none());
}

#[test]
fn the_gradient_follows_the_sequence_value_not_the_trace_count() {
    let mut app = table_app(&["0 min", "1 min", "4 min"]);
    let figure = stacked(&mut app, waterfall());
    let map = ColormapId::Viridis;
    assert_eq!(figure.series[0].color, map.sample(0.0));
    assert_eq!(figure.series[1].color, map.sample(0.25));
    assert_eq!(figure.series[2].color, map.sample(1.0));
}

#[test]
fn the_side_scale_names_traces_at_their_baselines_in_place_of_a_legend() {
    let mut app = table_app(&["300 K", "350 K"]);
    let figure = stacked(&mut app, waterfall());
    let scale = figure.sequence_scale.as_ref().unwrap();
    assert_eq!(scale.axis.label, "Temperature (K)");
    assert_eq!(
        scale.ticks,
        [(0.0, "300".to_owned()), (1.5, "350".to_owned())]
    );
    assert_eq!(figure.guide_visibility, GuideVisibility::Hide);
}

#[test]
fn traces_without_a_sequence_count_from_one() {
    let mut app = table_app(&["Fe", "Co", "Ni"]);
    let figure = stacked(&mut app, waterfall());
    let scale = figure.sequence_scale.as_ref().unwrap();
    assert_eq!(scale.axis.label, "Trace");
    let labels: Vec<&str> = scale.ticks.iter().map(|(_, l)| l.as_str()).collect();
    assert_eq!(labels, ["1", "2", "3"]);
    assert_eq!(figure.series[1].color, ColormapId::Viridis.sample(0.5));
}

#[test]
fn other_chart_types_of_a_table_are_not_stacked() {
    let mut app = table_app(&["0 min", "1 min"]);
    stacked(&mut app, waterfall());
    let object = &app.doc.canvases[0].objects[0];
    let (id, before) = (object.id, object.plot().unwrap().chart.clone());
    let heatmap = ChartSpec {
        type_id: "table_heatmap".to_owned(),
        ..ChartSpec::default()
    };
    app.execute_action(Action::set_chart_type(0, id, before, heatmap));
    let plot = app.doc.canvases[0].objects[0].plot().unwrap();
    assert!(plot.figure().heatmap.is_some());
    assert!(plot.figure().sequence_scale.is_none());
}

#[test]
fn each_column_keeps_its_own_whiskers_and_fit() {
    let columns = |name, color| {
        let mut series = Series::points(name, vec![[0.0, 1.0], [1.0, 2.0]]);
        series.color = color;
        series
    };
    let (red, blue) = (
        plotium_figure::Color::rgb(255, 0, 0),
        plotium_figure::Color::rgb(0, 0, 255),
    );
    let whiskers = vec![
        ErrorBar::symmetric([0.0, 1.0], 0.1),
        ErrorBar::symmetric([1.0, 2.0], 0.1),
        ErrorBar::symmetric([0.0, 1.0], 0.1),
    ];
    let fit = Series::line("b", vec![[0.0, 1.0]]);
    let traces = super::split_columns(
        vec![columns("a", red), columns("b", blue), fit],
        vec!["a".to_owned(), "b".to_owned(), "b".to_owned()],
        whiskers,
        0.5,
    );
    assert_eq!(traces[0].2.len(), 2);
    assert_eq!(
        traces[1].2.len(),
        1,
        "same observations stay in column order"
    );
    assert_eq!(traces[0].1[0].kind, SeriesKind::Line);
    assert_eq!(traces[1].1.len(), 2, "the fit stays with its column");
}

#[test]
fn a_column_of_isolated_points_stays_points() {
    let sparse = Series::points("a", vec![[0.0, 1.0], [1.0, f64::NAN], [2.0, 1.0]]);
    let traces = super::split_columns(vec![sparse], vec!["a".to_owned()], Vec::new(), 0.5);
    assert_eq!(traces[0].1[0].kind, SeriesKind::Points);
}

#[test]
fn a_legend_label_leaves_the_sequence_in_the_column_names() {
    let mut app = table_app(&["300 K", "350 K"]);
    app.doc.canvases[0].objects[0]
        .plot_mut()
        .unwrap()
        .binding
        .series[0]
        .label = Some("Scan".to_owned());
    let figure = stacked(&mut app, waterfall());
    let scale = figure.sequence_scale.as_ref().unwrap();
    assert_eq!(scale.axis.label, "Temperature (K)");
}

#[test]
fn normalized_traces_fill_the_window() {
    let mut app = table_app(&["0 min", "1 min"]);
    let figure = stacked(
        &mut app,
        StackSpec {
            normalize: true,
            ..waterfall()
        },
    );
    assert_eq!(figure.y.max, 1.5);
}

#[test]
fn stack_options_apply_to_any_trace_list() {
    let mut figure = Figure::new(
        "",
        plotium_figure::Axis::new("x", 0.0, 1.0),
        plotium_figure::Axis::new("y", 0.0, 1.0),
    );
    figure.series.push(Series::line("a", vec![[0.0, 0.0]]));
    let traces = [super::StackTrace {
        name: "a".to_owned(),
        baseline: 0.0,
        series: 0..1,
    }];
    super::apply_stack_options(&mut figure, &waterfall(), &traces);
    assert!(figure.series[0].occludes);
    assert_eq!(figure.series[0].color, ColormapId::Viridis.sample(0.0));
    assert!(figure.sequence_scale.is_some());
}
