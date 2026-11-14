use super::*;
use crate::Margins;
use plotium_figure::{Axis, AxisScale, Color, Figure, Polygon, Series};

fn span(axis: SpanAxis, range: [f64; 2], label: &str) -> SpanAnnotation {
    SpanAnnotation {
        axis,
        range,
        color: Color::rgb(0x33, 0x66, 0xcc),
        opacity: 0.25,
        label: label.into(),
        label_size: None,
    }
}

fn figure(spans: Vec<SpanAnnotation>) -> Figure {
    let mut fig = Figure::new("", Axis::new("x", 0.0, 10.0), Axis::new("y", 0.0, 1.0))
        .with_series(Series::line("trace", vec![[0.0, 0.0], [10.0, 1.0]]));
    fig.spans = spans;
    fig
}

fn rects(fig: &Figure) -> (Rect, Vec<(Rect, Option<TextAnchor>)>) {
    let outer = Rect::new(0.0, 0.0, fig.width, fig.height);
    let proj = Projector::new(fig, outer, &Margins::for_figure(fig));
    let items = layout(&proj, 1.0)
        .into_iter()
        .map(|item| (item.rect, item.label.map(|label| label.anchor)))
        .collect();
    (proj.plot, items)
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn spans_cover_their_data_range_across_the_plot() {
    let fig = figure(vec![
        span(SpanAxis::X, [7.5, 2.5], "window"),
        span(SpanAxis::Y, [0.25, 0.5], ""),
    ]);
    let (plot, items) = rects(&fig);
    assert_eq!(items.len(), 2);
    let (x, label) = items[0];
    assert!(close(x.left, plot.left + 0.25 * plot.width));
    assert!(close(x.right(), plot.left + 0.75 * plot.width));
    assert_eq!((x.top, x.height), (plot.top, plot.height));
    assert_eq!(label, Some(TextAnchor::Left));
    let (y, label) = items[1];
    assert!(close(y.top, plot.top + 0.5 * plot.height));
    assert!(close(y.bottom(), plot.top + 0.75 * plot.height));
    assert_eq!((y.left, y.width), (plot.left, plot.width));
    assert!(label.is_none());
}

#[test]
fn spans_clip_to_the_plot_and_drop_when_off_axis() {
    let mut fig = figure(vec![
        span(SpanAxis::X, [-5.0, 5.0], ""),
        span(SpanAxis::X, [11.0, 12.0], ""),
        span(SpanAxis::Y, [-1.0, 0.5], ""),
    ]);
    let (plot, items) = rects(&fig);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].0.left, plot.left);
    assert_eq!(items[1].0.bottom(), plot.bottom());

    // An end at or below zero on a log axis reaches the plot edge.
    fig.x = Axis::new("x", 1.0, 100.0);
    fig.x.scale = AxisScale::Log10;
    fig.spans = vec![span(SpanAxis::X, [0.0, 10.0], "")];
    let (plot, items) = rects(&fig);
    assert_eq!(items[0].0.left, plot.left);
    assert!(close(items[0].0.right(), plot.left + 0.5 * plot.width));
}

#[test]
fn svg_paints_spans_behind_bars_and_series() {
    let mut fig = figure(vec![span(SpanAxis::X, [2.0, 4.0], "OER")]);
    fig.polygons.push(Polygon::new(
        "bar",
        vec![[1.0, 0.0], [3.0, 0.0], [3.0, 0.8], [1.0, 0.8]],
        Color::rgb(0xcc, 0x33, 0x33),
    ));
    let svg = crate::svg::export(&fig);
    let span = svg.find(r#"class="span""#).expect("span rect");
    let label = svg.find(r#"class="span-label""#).expect("span label");
    let bar = svg.find("<polygon").expect("bar polygon");
    let series = svg.find("<polyline").expect("series line");
    assert!(span < bar && bar < series && series < label);
    assert!(svg.contains(r##"fill="#3366cc" fill-opacity="0.250""##));
}
