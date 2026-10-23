use super::*;
use crate::Margins;
use plotium_figure::{ArrowAnnotation, ArrowHead, Axis, Color, LineStyle, Series};

fn figure(series: Vec<Series>) -> Figure {
    let mut f = Figure::new("", Axis::new("x", 0.0, 10.0), Axis::new("y", 0.0, 10.0));
    f.series = series;
    f
}

fn text(at: [f64; 2], label: &str) -> Annotation {
    Annotation {
        text: label.into(),
        at,
        color: Color::BLACK,
        size: 8.0,
        anchor: plotium_figure::TextAnchor::Point,
        offset_pt: None,
    }
}

fn reference(orientation: ReferenceOrientation, value: f64, label: &str) -> ReferenceLine {
    ReferenceLine {
        orientation,
        value,
        color: Color::BLACK,
        width: 1.0,
        line_style: LineStyle::Dashed,
        label: label.into(),
        label_size: None,
        label_position: None,
    }
}

fn flat(y: f64) -> Series {
    Series::line("flat", (0..=100).map(|i| [i as f64 / 10.0, y]).collect())
}

fn plot_of(fig: &Figure) -> Rect {
    let outer = Rect::new(0.0, 0.0, fig.width, fig.height);
    crate::Projector::new(fig, outer, &Margins::for_figure(fig)).plot
}

fn text_label(fig: &Figure, index: usize) -> TextLabelBox {
    label_layout(fig, plot_of(fig), 1.0).texts[index].expect("placed text")
}

fn data_hits(fig: &Figure, bounds: Rect) -> usize {
    let mut hits = 0;
    for_each_data_segment(fig, plot_of(fig), |a, b| {
        hits += usize::from(clip(a, b, bounds).is_some());
    });
    hits
}

#[test]
fn clear_labels_keep_their_default_place() {
    let mut f = figure(vec![flat(1.0)]);
    f.author_annotations.push(text([5.0, 6.0], "peak"));
    f.reference_lines = vec![
        reference(ReferenceOrientation::Vertical, 2.5, "onset"),
        reference(ReferenceOrientation::Horizontal, 8.0, "limit"),
    ];
    let plot = plot_of(&f);
    let layout = label_layout(&f, plot, 1.0);
    let label = layout.texts[0].expect("placed text");
    assert_eq!(label.offset_pt, [0.0, 0.0]);
    let vertical = layout.reference[0].expect("vertical label");
    assert_eq!(vertical.top, plot.top + REFERENCE_GAP_PT);
    assert_eq!(vertical.anchor, TextAnchor::Left);
    let horizontal = layout.reference[1].expect("horizontal label");
    assert_eq!(horizontal.anchor, TextAnchor::Right);
    assert_eq!(horizontal.x, plot.right() - REFERENCE_GAP_PT);
    assert!(horizontal.bounds.bottom() < project(&f, plot, [0.0, 8.0], YAxisSide::Left).1);
}

#[test]
fn a_covered_text_moves_to_the_nearest_clear_spot() {
    let mut f = figure(vec![flat(5.0)]);
    f.author_annotations.push(text([5.0, 5.0], "peak"));
    let label = text_label(&f, 0);
    assert_ne!(label.offset_pt, [0.0, 0.0]);
    assert_eq!(data_hits(&f, label.bounds), 0);
    let reach = 3.0 * f.author_annotations[0].size;
    assert!(label.offset_pt[0].hypot(label.offset_pt[1]) <= reach + 1e-3);
    // The nearest clear spot is straight up, just clearing the line.
    assert_eq!(label.offset_pt[0], 0.0);
    assert!(label.offset_pt[1] > 0.0 && label.offset_pt[1] <= 4.0);
}

#[test]
fn labels_avoid_each_other_and_the_legend() {
    let mut f = figure(vec![Series::line("trace", vec![[0.0, 0.0], [10.0, 1.0]])]);
    f.guide_visibility = plotium_figure::GuideVisibility::Show;
    f.guide_placement = plotium_figure::GuidePlacement::Inside;
    let plot = plot_of(&f);
    let legend = crate::legend_rect(&f, plot, 1.0).expect("legend");
    let legend_centre = [
        f.x.min
            + f.x.span() * f64::from((legend.left + legend.width * 0.5 - plot.left) / plot.width),
        f.y.max
            - f.y.span() * f64::from((legend.top + legend.height * 0.5 - plot.top) / plot.height),
    ];
    f.author_annotations = vec![
        text([3.0, 6.0], "first"),
        text([3.0, 6.0], "second"),
        text(legend_centre, "legend"),
    ];
    let layout = label_layout(&f, plot, 1.0);
    let boxes: Vec<Rect> = layout
        .texts
        .iter()
        .map(|l| l.expect("placed").bounds)
        .collect();
    assert!(!overlaps(boxes[0], boxes[1]));
    assert!(boxes.iter().all(|b| !overlaps(*b, legend)));
}

#[test]
fn a_pinned_offset_follows_its_anchor_through_range_changes() {
    let mut f = figure(vec![flat(5.0)]);
    let mut pinned = text([4.0, 5.0], "pinned");
    pinned.offset_pt = Some([6.0, -3.0]);
    f.author_annotations.push(pinned);
    for range in [(0.0, 10.0), (2.0, 6.0)] {
        f.x.min = range.0;
        f.x.max = range.1;
        let plot = plot_of(&f);
        let label = text_label(&f, 0);
        let anchor = project(&f, plot, [4.0, 5.0], YAxisSide::Left);
        assert_eq!(label.offset_pt, [6.0, -3.0]);
        assert!((label.x - (anchor.0 + 6.0)).abs() < 1e-3);
        assert!((label.y - (anchor.1 + 3.0)).abs() < 1e-3);
    }
}

#[test]
fn a_pinned_reference_label_stays_on_its_line() {
    let mut f = figure(vec![]);
    let mut line = reference(ReferenceOrientation::Vertical, 5.0, "mid");
    line.label_position = Some(ReferenceLabelPosition {
        along: 0.5,
        side: ReferenceLabelSide::Before,
    });
    f.reference_lines.push(line);
    for range in [(0.0, 10.0), (4.0, 8.0)] {
        f.x.min = range.0;
        f.x.max = range.1;
        let plot = plot_of(&f);
        let label = label_layout(&f, plot, 1.0).reference[0].expect("label");
        let x = project(&f, plot, [5.0, 0.0], YAxisSide::Left).0;
        assert_eq!(label.anchor, TextAnchor::Right);
        assert!((label.x - (x - REFERENCE_GAP_PT)).abs() < 1e-3);
        let centre = label.top + 0.5 * label.size;
        assert!((centre - (plot.top + 0.5 * plot.height)).abs() < 1e-3);
    }
}

#[test]
fn a_reference_label_leaves_a_curve_at_the_top() {
    let mut f = figure(vec![flat(9.8)]);
    f.reference_lines
        .push(reference(ReferenceOrientation::Vertical, 3.0, "onset"));
    let plot = plot_of(&f);
    let label = label_layout(&f, plot, 1.0).reference[0].expect("label");
    assert!(label.position.along < 1.0);
    assert_eq!(data_hits(&f, label.bounds), 0);
}

#[test]
fn an_arrow_pointing_at_a_text_does_not_push_it_away() {
    let mut f = figure(vec![]);
    f.author_annotations.push(text([5.0, 5.0], "here"));
    f.author_arrows.push(ArrowAnnotation {
        from: [5.0, 5.0],
        to: [8.0, 2.0],
        color: Color::BLACK,
        width: 1.0,
        head: ArrowHead::Filled,
    });
    assert_eq!(text_label(&f, 0).offset_pt, [0.0, 0.0]);
}

#[test]
fn placement_does_not_depend_on_zoom() {
    let mut f = figure(vec![flat(5.0)]);
    f.author_annotations.push(text([5.0, 5.0], "peak"));
    f.reference_lines
        .push(reference(ReferenceOrientation::Horizontal, 5.0, "baseline"));
    let plot = Rect::new(40.0, 10.0, 300.0, 200.0);
    let zoomed = Rect::new(80.0, 20.0, 600.0, 400.0);
    let base = label_layout(&f, plot, 1.0);
    let big = label_layout(&f, zoomed, 2.0);
    assert_eq!(
        base.texts[0].map(|l| l.offset_pt),
        big.texts[0].map(|l| l.offset_pt)
    );
    assert_eq!(
        base.reference[0].map(|l| l.position),
        big.reference[0].map(|l| l.position)
    );
}

/// Position of the SVG `<text>` holding `content`.
fn svg_text_position(svg: &str, content: &str) -> (f32, f32) {
    let end = svg.find(&format!(">{content}</text>")).expect("svg text");
    let start = svg[..end].rfind("<text ").expect("text element");
    let attr = |name: &str| -> f32 {
        let tag = &svg[start..end];
        let from = tag.find(&format!(" {name}=\"")).expect("attribute") + name.len() + 3;
        tag[from..from + tag[from..].find('"').expect("closing quote")]
            .parse()
            .expect("number")
    };
    (attr("x"), attr("y"))
}

#[test]
fn svg_moves_a_covered_text_and_screen_agrees() {
    let mut f = figure(vec![flat(5.0)]);
    let clear = {
        let mut f = f.clone();
        f.series.clear();
        f.author_annotations.push(text([5.0, 5.0], "peak"));
        svg_text_position(&crate::svg::export(&f), "peak")
    };
    f.author_annotations.push(text([5.0, 5.0], "peak"));
    let moved = svg_text_position(&crate::svg::export(&f), "peak");
    assert_eq!(moved.0, clear.0);
    assert!(moved.1 < clear.1);
    #[cfg(feature = "screen")]
    {
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let outer = Rect::new(0.0, 0.0, f.width, f.height);
            crate::screen::paint(ui.painter(), outer, &f, 1.0);
        });
        let drawn = output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == "peak" => Some(t.visual_bounding_rect()),
                _ => None,
            })
            .expect("painted label");
        assert!((drawn.center().x - moved.0).abs() < 1.0);
        assert!((drawn.bottom() - moved.1).abs() < 3.0);
    }
}

#[test]
fn dense_lines_do_not_make_placement_depend_on_zoom() {
    // A step pooled into columns whose width follows the zoom.
    let step = Series::line(
        "step",
        (0..1000)
            .map(|i| {
                let x = i as f64 / 100.0;
                [x, if x < 5.0 { 1.0 } else { 9.0 }]
            })
            .collect(),
    );
    let mut f = figure(vec![step]);
    f.author_annotations.push(text([5.0, 5.0], "riser"));
    let plot = Rect::new(40.0, 10.0, 300.0, 200.0);
    let offsets: Vec<_> = [1.0, 2.0, 3.5]
        .into_iter()
        .map(|s| {
            let zoomed = Rect::new(plot.left * s, plot.top * s, plot.width * s, plot.height * s);
            label_layout(&f, zoomed, s).texts[0].map(|l| l.offset_pt)
        })
        .collect();
    assert!(offsets.windows(2).all(|w| w[0] == w[1]), "{offsets:?}");
}

#[test]
fn arrows_push_reference_labels_along_their_line() {
    let mut f = figure(vec![]);
    f.reference_lines
        .push(reference(ReferenceOrientation::Vertical, 3.0, "onset"));
    let plot = plot_of(&f);
    let clear = label_layout(&f, plot, 1.0).reference[0].expect("label");
    let y =
        f.y.max - f.y.span() * f64::from((clear.top + 0.5 * clear.size - plot.top) / plot.height);
    f.author_arrows.push(ArrowAnnotation {
        from: [0.5, y],
        to: [9.5, y],
        color: Color::BLACK,
        width: 1.0,
        head: ArrowHead::Filled,
    });
    let label = label_layout(&f, plot, 1.0).reference[0].expect("label");
    assert_ne!(label.position, clear.position);
}
