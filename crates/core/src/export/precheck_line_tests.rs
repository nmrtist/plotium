//! Thinnest-line coverage and per-preset line minimums.
use super::*;
use crate::state::{
    AxisOverrides, AxisProjections, CanvasObject, CanvasViewport, ChartSpec, DataBinding,
    ObjectFrame, ObjectId, PlotObject, StackSpec,
};
use plotium_figure::{Axis, Color, ErrorBar, Figure, Polygon, Series};

/// A figure whose only line is its axis frame, at `axis_pt`.
fn figure(axis_pt: f32) -> Figure {
    let mut figure = Figure::new("", Axis::new("x", 0.0, 2.0), Axis::new("y", 0.0, 2.0));
    figure.typography.axis_line_pt = axis_pt;
    figure
}

fn bar(stroke: Option<(Color, f32)>) -> Polygon {
    let rect = Polygon::rect("bar", 0.5, 1.5, 0.0, 1.0, Color::rgb(0x93, 0xb5, 0xdc));
    match stroke {
        Some((color, width)) => rect.with_stroke(color, width),
        None => rect,
    }
}

fn page(figure: Figure, size_mm: [f32; 2], preset: Option<&str>) -> CanvasDocument {
    page_with(figure, size_mm, preset, true)
}

/// `explicit` plots come from column bindings (curves and bars); others are
/// chart-builder figures such as the table matrix chart.
fn page_with(
    figure: Figure,
    size_mm: [f32; 2],
    preset: Option<&str>,
    explicit: bool,
) -> CanvasDocument {
    let viewport = CanvasViewport::from_figure(&figure);
    let mut canvas = CanvasDocument::new("Page".to_owned(), size_mm);
    canvas.size_preset_id = preset.map(str::to_owned);
    canvas.objects.push(CanvasObject {
        id: ObjectId::new(1),
        name: "Plot".to_owned(),
        frame: ObjectFrame::new(0.0, 0.0, 100.0, 100.0),
        locked: false,
        visible: true,
        kind: CanvasObjectKind::Plot(Box::new(PlotObject::new(
            None,
            crate::state::SeriesId::new(1),
            DataBinding {
                coordinates: explicit
                    .then_some(crate::state::CurveCoordinates { x: None, y: None }),
                series: Vec::new(),
            },
            ChartSpec::default(),
            StackSpec::default(),
            AxisProjections::default(),
            AxisOverrides::default(),
            figure,
            viewport,
        ))),
    });
    canvas
}

fn nature(figure: Figure) -> CanvasDocument {
    page(figure, [89.0, 60.0], Some("nature-1col"))
}

fn plain(figure: Figure) -> CanvasDocument {
    page(figure, [89.0, 60.0], None)
}

/// The "Thinnest line" item for one page exported at its own size.
fn line_item(canvas: &CanvasDocument) -> ComplianceItem {
    precheck_report(&[page_metrics(canvas)], None, ExportFormat::Svg, 300).items[1].clone()
}

#[test]
fn nature_pages_accept_lines_down_to_a_quarter_point() {
    let item = line_item(&nature(figure(0.3)));
    assert_eq!(item.status, ComplianceStatus::Pass, "{}", item.detail);
    assert!(item.detail.contains("min 0.25 pt"), "{}", item.detail);
    assert_eq!(page_metrics(&nature(figure(0.3))).required_line_pt, 0.25);
}

#[test]
fn pages_without_a_journal_preset_keep_the_half_point_minimum() {
    let item = line_item(&plain(figure(0.3)));
    assert_eq!(item.status, ComplianceStatus::Fail);
    assert!(item.detail.contains("min 0.5 pt"), "{}", item.detail);
    // A page resized away from its Nature preset no longer follows it.
    let resized = page(figure(0.3), [100.0, 60.0], Some("nature-1col"));
    assert_eq!(page_metrics(&resized).required_line_pt, 0.5);
}

#[test]
fn the_axis_frame_and_ticks_count_as_lines() {
    let mut thin_axis = figure(0.3);
    thin_axis
        .series
        .push(Series::line("trace", vec![[0.0, 0.0], [1.0, 1.0]]));
    assert_eq!(
        page_metrics(&plain(thin_axis.clone())).min_line_pt,
        Some(0.3)
    );
    // Hidden axes draw no frame.
    thin_axis.axis_frame = AxisFrame::Hidden;
    assert_eq!(
        page_metrics(&plain(thin_axis)).min_line_pt,
        Some(plotium_figure::DEFAULT_DATA_LINE_WIDTH_PT)
    );
}

#[test]
fn bar_outlines_are_checked_against_the_page_minimum() {
    let mut outlined = figure(1.0);
    outlined.polygons.push(bar(Some((Color::BLACK, 0.4))));
    assert_eq!(
        page_metrics(&plain(outlined.clone())).min_line_pt,
        Some(0.4)
    );
    assert_eq!(
        line_item(&plain(outlined.clone())).status,
        ComplianceStatus::Fail
    );
    assert_eq!(line_item(&nature(outlined)).status, ComplianceStatus::Pass);
}

#[test]
fn zero_width_and_background_strokes_are_not_lines() {
    let mut figure = figure(1.0);
    let background = figure.background;
    figure.polygons.push(bar(None));
    figure.polygons.push(bar(Some((Color::BLACK, 0.0))));
    // Stacked segments are separated in the background color.
    figure.polygons.push(bar(Some((background, 0.2))));
    assert_eq!(page_metrics(&plain(figure)).min_line_pt, Some(1.0));
}

#[test]
fn chart_builder_shape_edges_are_not_authored_lines() {
    let mut facets = figure(1.0);
    facets.polygons.push(bar(Some((Color::BLACK, 0.3))));
    let chart = page_with(facets, [89.0, 60.0], None, false);
    assert_eq!(page_metrics(&chart).min_line_pt, Some(1.0));
}

#[test]
fn error_bars_and_marker_edges_count_as_lines() {
    let mut whiskers = figure(1.0);
    let mut error = ErrorBar::symmetric([1.0, 1.0], 0.2);
    error.width = 0.35;
    whiskers.error_bars.push(error);
    assert_eq!(page_metrics(&plain(whiskers)).min_line_pt, Some(0.35));

    let mut markers = figure(1.0);
    let mut points = Series::points("points", vec![[1.0, 1.0]]);
    points.width = 3.0;
    points.appearance.marker_edge_width = plotium_figure::PositiveFiniteF32::new(0.3);
    markers.series.push(points);
    assert_eq!(page_metrics(&plain(markers)).min_line_pt, Some(0.3));
}

#[test]
fn marker_radii_and_undrawn_edges_are_not_lines() {
    // A point series' width is its marker radius.
    let mut dots = figure(1.0);
    let mut points = Series::points("dots", vec![[1.0, 1.0]]);
    points.width = 0.4;
    dots.series.push(points.clone());
    assert_eq!(page_metrics(&plain(dots)).min_line_pt, Some(1.0));

    // Plus and cross markers are strokes of that width, with no edge drawn.
    let mut crosses = figure(1.0);
    points.appearance.marker = Some(plotium_figure::Marker::Cross);
    points.appearance.marker_edge_width = plotium_figure::PositiveFiniteF32::new(0.2);
    crosses.series.push(points);
    assert_eq!(page_metrics(&plain(crosses)).min_line_pt, Some(0.4));
}
