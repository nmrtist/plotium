use super::*;
use crate::{Margins, Projector};
use plotium_figure::{Axis, Color, ReferenceLine, Series};

fn line(orientation: ReferenceOrientation, value: f64, label: &str) -> ReferenceLine {
    ReferenceLine {
        orientation,
        value,
        color: Color::rgb(0xcc, 0x22, 0x11),
        width: 1.5,
        line_style: LineStyle::Dashed,
        label: label.into(),
        label_size: None,
        label_position: None,
    }
}

fn figure(lines: Vec<ReferenceLine>) -> Figure {
    let mut fig = Figure::new("", Axis::new("x", 0.0, 10.0), Axis::new("y", 0.0, 1.0))
        .with_series(Series::line("trace", vec![[0.0, 0.0], [10.0, 1.0]]));
    fig.reference_lines = lines;
    fig
}

/// Start, end and optional (label x, anchor) of one laid-out line.
type Laid = ((f32, f32), (f32, f32), Option<(f32, TextAnchor)>);

fn layouts(fig: &Figure) -> (Rect, Vec<Laid>) {
    let outer = Rect::new(0.0, 0.0, fig.width, fig.height);
    let margins = Margins::for_figure(fig);
    let proj = Projector::new(fig, outer, &margins);
    let plot = proj.plot;
    let labels = crate::label_layout(fig, plot, 1.0);
    let items = layout(fig, plot, &labels)
        .into_iter()
        .map(|l| (l.start, l.end, l.label.map(|label| (label.x, label.anchor))))
        .collect();
    (plot, items)
}

#[test]
fn lines_span_the_plot_at_their_data_value() {
    let fig = figure(vec![
        line(ReferenceOrientation::Vertical, 2.5, "onset"),
        line(ReferenceOrientation::Horizontal, 0.5, ""),
    ]);
    let (plot, items) = layouts(&fig);
    assert_eq!(items.len(), 2);
    let (start, end, label) = items[0];
    let x = plot.left + 0.25 * plot.width;
    assert!((start.0 - x).abs() < 1e-3 && (end.0 - x).abs() < 1e-3);
    assert_eq!((start.1, end.1), (plot.bottom(), plot.top));
    // Left half: the label sits to the right of the line.
    assert!(matches!(label, Some((lx, TextAnchor::Left)) if lx > x));
    let (start, end, label) = items[1];
    let y = plot.top + 0.5 * plot.height;
    assert!((start.1 - y).abs() < 1e-3 && (end.1 - y).abs() < 1e-3);
    assert_eq!((start.0, end.0), (plot.left, plot.right()));
    assert!(label.is_none());
}

#[test]
fn label_moves_left_of_a_line_in_the_right_half() {
    let fig = figure(vec![line(ReferenceOrientation::Vertical, 9.0, "late")]);
    let (_, items) = layouts(&fig);
    let (start, _, label) = items[0];
    assert!(matches!(label, Some((lx, TextAnchor::Right)) if lx < start.0));
}

#[test]
fn values_outside_the_visible_range_are_clipped_away() {
    let fig = figure(vec![
        line(ReferenceOrientation::Vertical, 11.0, "off"),
        line(ReferenceOrientation::Horizontal, -0.5, "off"),
        line(ReferenceOrientation::Vertical, f64::NAN, "nan"),
    ]);
    assert!(layouts(&fig).1.is_empty());
    let svg = crate::svg::export(&fig);
    assert!(!svg.contains("reference-line"));
}

#[test]
fn svg_draws_dashed_and_solid_lines_with_labels() {
    let mut solid = line(ReferenceOrientation::Horizontal, 0.25, "baseline & noise");
    solid.line_style = LineStyle::Solid;
    let fig = figure(vec![
        line(ReferenceOrientation::Vertical, 5.0, "onset"),
        solid,
    ]);
    let svg = crate::svg::export(&fig);
    assert_eq!(svg.matches(r#"<line class="reference-line""#).count(), 2);
    assert_eq!(svg.matches("stroke-dasharray=\"9,4.5\"").count(), 1);
    assert!(svg.contains(r##"stroke="#cc2211""##));
    assert!(svg.contains("onset"));
    assert!(svg.contains("baseline &amp; noise"));
    // Lines sit inside the plot clip group with the other plot content.
    let clip_start = svg.find(r#"clip-path="url("#).expect("plot clip group");
    let clip_end = clip_start + svg[clip_start..].find("</g>").expect("clip group end");
    let at = svg.find("reference-line").expect("reference line");
    assert!(clip_start < at && at < clip_end);
}

#[cfg(feature = "screen")]
#[test]
fn screen_paints_clipped_line_segments() {
    let fig = figure(vec![line(ReferenceOrientation::Vertical, 5.0, "")]);
    let mut solid = fig.clone();
    solid.reference_lines[0].line_style = LineStyle::Solid;
    let count = |fig: &Figure| {
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let outer = Rect::new(0.0, 0.0, fig.width, fig.height);
            crate::screen::paint(ui.painter(), outer, fig, 1.0);
        });
        let red = egui::Color32::from_rgb(0xcc, 0x22, 0x11);
        output
            .shapes
            .iter()
            .filter(|s| matches!(&s.shape, egui::Shape::LineSegment { stroke, .. } if stroke.color == red))
            .count()
    };
    assert_eq!(count(&solid), 1);
    assert!(count(&fig) > 1);
}
