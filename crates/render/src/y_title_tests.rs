use super::*;
use crate::diagnostics::{TextBox, figure_text_boxes, figure_text_boxes_placed};
use crate::{AXIS_LABEL_GAP, StackPlacement, axis_layout, rich_text::TOP_TO_BASELINE, svg};
use plotium_figure::Axis;

fn bars(categories: &[&str]) -> Figure {
    let mut fig = Figure::new(
        "",
        Axis::categorical(
            "Membrane",
            categories.iter().map(|name| (*name).to_owned()).collect(),
        ),
        Axis::new("Ionic transportation E_{a} (kJ mol^{-1})", 0.0, 50.0),
    );
    fig.width = 252.0;
    fig.height = 170.0;
    fig
}

fn boxes<'a>(all: &'a [TextBox], prefix: &str) -> impl Iterator<Item = Rect> + 'a {
    let prefix = prefix.to_owned();
    all.iter()
        .filter(move |text| text.component.starts_with(&prefix))
        .map(|text| text.bounds)
}

fn title_gaps(all: &[TextBox]) -> (f32, f32) {
    let y_title = boxes(all, "axis.y.label").next().expect("y title");
    let x_title = boxes(all, "axis.x.label").next().expect("x title");
    let y_ticks_left = boxes(all, "axis.y.tick:")
        .map(|bounds| bounds.left)
        .fold(f32::INFINITY, f32::min);
    let x_ticks_bottom = boxes(all, "axis.x.tick:")
        .map(|bounds| bounds.bottom())
        .fold(f32::NEG_INFINITY, f32::max);
    (y_ticks_left - y_title.right(), x_title.top - x_ticks_bottom)
}

fn svg_title_baseline(svg: &str) -> f32 {
    let end = svg.find(") rotate(-90)").expect("rotated y title");
    let start = svg[..end].rfind("translate(").expect("title translate") + "translate(".len();
    svg[start..end]
        .split(',')
        .next()
        .and_then(|x| x.parse().ok())
        .expect("title x")
}

#[test]
fn y_title_keeps_the_x_title_gap_from_its_tick_labels() {
    // Wide category names used to push the plot right and leave the extra
    // room between the y title and its tick labels.
    for fig in [
        bars(&["PorC1", "PorC4", "Fumasep BPM"]),
        bars(&["50", "100", "200", "300", "400", "500"]),
    ] {
        let all = figure_text_boxes(&fig, [fig.width, fig.height]);
        let (y_gap, x_gap) = title_gaps(&all);
        assert!((y_gap - AXIS_LABEL_GAP).abs() < 1e-3, "y gap {y_gap}");
        assert!((x_gap - AXIS_LABEL_GAP).abs() < 1e-3, "x gap {x_gap}");

        // SVG draws the baseline where the diagnostics box puts the line.
        let title = boxes(&all, "axis.y.label").next().expect("y title");
        let baseline = svg_title_baseline(&svg::export(&fig));
        let expected = title.left + fig.typography.label_pt * TOP_TO_BASELINE;
        assert!(
            (baseline - expected).abs() < 0.01,
            "{baseline} vs {expected}"
        );
    }
}

#[test]
fn category_labels_centred_in_their_slots_reserve_no_left_overhang() {
    let short = axis_layout(&bars(&["A", "B", "C"]), 252.0, 170.0);
    let wide = axis_layout(&bars(&["PorC1", "PorC4", "Fumasep BPM"]), 252.0, 170.0);
    assert_eq!(wide.margins.left, short.margins.left);
}

#[test]
fn a_left_end_x_label_still_pushes_the_plot_but_not_the_title() {
    let mut x = Axis::new("Shift (Hz)", -8_000.0, 0.0);
    x.thousands_separator = true;
    let mut fig = Figure::new("", x, Axis::new("Count", 0.0, 8.0));
    fig.width = 252.0;
    fig.height = 170.0;
    let layout = axis_layout(&fig, fig.width, fig.height);
    let ty = fig.typography;
    let half_label = ticks::estimated_text_width("-8,000", ty.tick_pt) * 0.5;
    let reserved = OUTER_PAD + ty.label_pt + AXIS_LABEL_GAP;
    assert!((layout.margins.left - reserved - half_label).abs() < 1e-3);

    let (y_gap, _) = title_gaps(&figure_text_boxes(&fig, [fig.width, fig.height]));
    assert!((y_gap - AXIS_LABEL_GAP).abs() < 1e-3, "y gap {y_gap}");
}

#[test]
fn a_title_reaching_into_a_protruding_x_label_row_stays_at_the_outer_edge() {
    let mut x = Axis::new("Shift (Hz)", -8_000.0, 0.0);
    x.thousands_separator = true;
    let mut fig = Figure::new("", x, Axis::new("Normalised intensity (a.u.)", 0.0, 8.0));
    fig.y.show_tick_labels = false;
    let size = [252.0, 90.0];
    let title = |fig: &Figure| {
        let all = figure_text_boxes(fig, size);
        boxes(&all, "axis.y.label").next().expect("y title")
    };
    assert!((title(&fig).left - OUTER_PAD).abs() < 1e-3);

    // A short title clears the x label row, so it moves next to the axis.
    fig.y.label = "I".to_owned();
    assert!(title(&fig).left > OUTER_PAD + 1.0);
}

#[test]
fn stacked_titles_align_beside_the_stacks_widest_tick_labels() {
    // The shared left edge comes from a protruding x label elsewhere in the
    // stack; each member's title still follows the widest y tick labels.
    let placement = StackPlacement {
        left: 40.0,
        right: 12.0,
        y_tick_clearance: 10.0,
        joined_above: false,
        joined_below: true,
    };
    for y_max in [3.0, 1_000.0] {
        let fig = Figure::new("", Axis::new("t", 0.0, 1.0), Axis::new("V", 0.0, y_max));
        let all = figure_text_boxes_placed(&fig, [240.0, 100.0], Some(&placement));
        let title = boxes(&all, "axis.y.label").next().expect("y title");
        let expected = placement.left - placement.y_tick_clearance - AXIS_LABEL_GAP;
        assert!((title.right() - expected).abs() < 1e-3, "{title:?}");
    }
}
