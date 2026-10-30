use super::*;
use crate::diagnostics::{TextBox, figure_text_boxes, figure_text_boxes_placed};
use crate::{Rect, axis_layout, legend_rect, svg};
use plotium_figure::{GuidePlacement, GuideVisibility, Series};

fn figure(right_title: &str) -> Figure {
    let mut fig = Figure::new(
        "",
        Axis::new("Time (s)", 0.0, 10.0),
        Axis::new("Yield (%)", 0.0, 50.0),
    );
    fig.width = 260.0;
    fig.height = 170.0;
    fig.series
        .push(Series::line("yield", vec![[0.0, 10.0], [10.0, 40.0]]));
    let mut rate = Series::line("rate", vec![[0.0, 1000.0], [10.0, 9000.0]]);
    rate.appearance.y_axis = YAxisSide::Right;
    fig.series.push(rate);
    fig.y2 = Some(Axis::new(right_title, 0.0, 10000.0));
    fig
}

fn boxes<'a>(all: &'a [TextBox], prefix: &'a str) -> impl Iterator<Item = Rect> + 'a {
    all.iter()
        .filter(move |text| text.component.starts_with(prefix))
        .map(|text| text.bounds)
}

#[test]
fn the_right_margin_holds_the_right_ticks_labels_and_title() {
    let mut plain = figure("Rate (Hz)");
    plain.y2 = None;
    let without = axis_layout(&plain, 260.0, 170.0);
    let fig = figure("Rate (Hz)");
    let layout = axis_layout(&fig, 260.0, 170.0);
    assert!(!layout.y2_ticks.values.is_empty());
    let widest = widest_tick_label(&layout.y2_ticks, fig.typography.tick_pt);
    let reserved = reserved_width(&fig, &layout.y2_ticks, widest);
    assert!(layout.margins.right >= reserved - 1e-3);
    assert!(layout.margins.right > without.margins.right);
    // A blank title reserves no title lane.
    let untitled = axis_layout(&figure(""), 260.0, 170.0);
    let lane = fig.typography.label_pt + crate::AXIS_LABEL_GAP;
    assert!((layout.margins.right - untitled.margins.right - lane).abs() < 1e-3);
}

#[test]
fn the_right_title_keeps_the_left_title_gap_from_its_tick_labels() {
    let fig = figure("Rate (Hz)");
    let all = figure_text_boxes(&fig, [fig.width, fig.height]);
    let title = boxes(&all, "axis.y2.label").next().expect("right title");
    let ticks_right = boxes(&all, "axis.y2.tick:")
        .map(|bounds| bounds.right())
        .fold(f32::NEG_INFINITY, f32::max);
    let left_title = boxes(&all, "axis.y.label").next().expect("left title");
    let ticks_left = boxes(&all, "axis.y.tick:")
        .map(|bounds| bounds.left)
        .fold(f32::INFINITY, f32::min);
    let right_gap = title.left - ticks_right;
    let left_gap = ticks_left - left_title.right();
    assert!(
        (right_gap - left_gap).abs() < 1e-3,
        "{right_gap} vs {left_gap}"
    );
    assert!(title.right() <= fig.width);
}

#[test]
fn right_curves_are_projected_on_the_right_axis() {
    let fig = figure("Rate (Hz)");
    let layout = axis_layout(&fig, fig.width, fig.height);
    let outer = Rect::new(0.0, 0.0, fig.width, fig.height);
    let proj = crate::Projector::new(&fig, outer, &layout.margins);
    let (_, top) = proj.on(YAxisSide::Right).project([0.0, 10000.0]);
    let (_, left) = proj.project([0.0, 10000.0]);
    assert!((top - proj.plot.top).abs() < 1e-3);
    assert!(
        left < proj.plot.top,
        "the left axis would put it far above the plot"
    );
}

#[test]
fn every_vector_backend_draws_the_right_axis() {
    let fig = figure("Rate (Hz)");
    let out = svg::export(&fig);
    assert!(out.contains(r#"class="axis-y2""#));
    assert!(out.contains(">Rate (Hz)</text>"));
    // Ticks point outward from the right spine of an open frame.
    let mut boxed = figure("Rate (Hz)");
    boxed.axis_frame = AxisFrame::Box;
    let layout = axis_layout(&boxed, boxed.width, boxed.height);
    let outer = Rect::new(0.0, 0.0, boxed.width, boxed.height);
    let proj = crate::Projector::new(&boxed, outer, &layout.margins);
    let right = right_axis(&boxed, &layout, &proj, None, 1.0).expect("right axis");
    assert!(!right.spine, "a box frame already strokes the right edge");
    assert!(!right.ticks.is_empty() && right.tick_reach > 0.0);

    let mut hidden = figure("Rate (Hz)");
    hidden.axis_frame = AxisFrame::Hidden;
    let layout = axis_layout(&hidden, hidden.width, hidden.height);
    assert!(right_axis(&hidden, &layout, &proj, None, 1.0).is_none());
}

#[test]
fn an_outside_legend_stands_clear_of_the_right_axis() {
    let mut fig = figure("Rate (Hz)");
    fig.guide_visibility = GuideVisibility::Show;
    fig.guide_placement = GuidePlacement::OutsideRight;
    let layout = axis_layout(&fig, fig.width, fig.height);
    let outer = Rect::new(0.0, 0.0, fig.width, fig.height);
    let plot = crate::Projector::new(&fig, outer, &layout.margins).plot;
    let legend = legend_rect(&fig, plot, 1.0).expect("legend");
    let all = figure_text_boxes(&fig, [fig.width, fig.height]);
    let title = boxes(&all, "axis.y2.label").next().expect("right title");
    assert!(legend.left >= title.right());
}

#[test]
fn stacked_right_titles_align_on_the_widest_right_ticks() {
    let narrow = figure("Rate (Hz)");
    let mut wide = figure("Rate (Hz)");
    wide.y2 = Some(Axis::new("Rate (Hz)", 0.0, 1_000_000.0));
    let clearance = axis_layout(&wide, 260.0, 120.0).y2_tick_clearance(&wide);
    let placement = StackPlacement {
        left: 48.0,
        right: 60.0,
        y_tick_clearance: 20.0,
        y2_tick_clearance: clearance,
        right_axis_width: 0.0,
        joined_above: false,
        joined_below: false,
    };
    let title = |fig: &Figure| {
        let all = figure_text_boxes_placed(fig, [260.0, 120.0], Some(&placement));
        boxes(&all, "axis.y2.label")
            .next()
            .expect("right title")
            .left
    };
    assert!((title(&narrow) - title(&wide)).abs() < 1e-3);
}

#[test]
fn sequence_scale_takes_the_right_side_and_thins_to_fit() {
    let mut fig = Figure::new("", Axis::new("x", 0.0, 1.0), Axis::new("y", 0.0, 100.0));
    fig.sequence_scale = Some(plotium_figure::SequenceScale {
        axis: Axis::new("Time (min)", 0.0, 100.0),
        ticks: (0..100).map(|i| (f64::from(i), i.to_string())).collect(),
    });
    assert!(super::shows_title(&fig));
    let ticks = crate::ticks::right_ticks(&fig, 100.0);
    assert!(ticks.values.len() >= 2 && ticks.values.len() < 100);
    assert_eq!(ticks.values[0], 0.0);
    assert!(crate::svg::export(&fig).contains("Time (min)"));

    fig.y.min = 10.0;
    fig.y.max = 11.0;
    let ticks = crate::ticks::right_ticks(&fig, 100.0);
    assert_eq!(ticks.values, [10.0, 11.0]);
}

#[test]
fn occluding_traces_fill_to_their_baseline_from_back_to_front() {
    let mut fig = Figure::new("", Axis::new("x", 0.0, 1.0), Axis::new("y", 0.0, 3.0));
    for (name, baseline) in [("front", 0.0), ("back", 1.0)] {
        let mut series = Series::line(name, vec![[0.0, baseline], [1.0, baseline + 1.0]]);
        series.display_baseline = baseline;
        series.occludes = true;
        fig.series.push(series);
    }
    let order: Vec<_> = fig
        .series_in_paint_order()
        .map(|(index, _)| index)
        .collect();
    assert_eq!(order, [1, 0]);
    let svg = crate::svg::export(&fig);
    assert!(svg.matches(r##"fill="#ffffff"/>"##).count() >= 2);
    let fill = svg.find("<polygon").unwrap();
    assert!(fill < svg.find("<polyline").unwrap());
}

#[test]
fn a_baseline_a_log_axis_cannot_place_fills_to_the_plot_edge() {
    let mut fig = Figure::new("", Axis::new("x", 0.0, 1.0), Axis::new("y", 1.0, 100.0));
    fig.y.scale = plotium_figure::AxisScale::Log10;
    let mut series = Series::line("s", vec![[0.0, 2.0], [1.0, 50.0]]);
    series.occludes = true;
    fig.series.push(series);
    let svg = crate::svg::export(&fig);
    assert!(svg.contains("<polygon") && !svg.contains("inf"));
}

#[test]
fn side_labels_space_along_the_run_of_traces() {
    let mut fig = Figure::new("", Axis::new("x", 0.0, 1.0), Axis::new("y", 0.0, 100.0));
    fig.sequence_scale = Some(plotium_figure::SequenceScale {
        axis: Axis::new("", 0.0, 100.0),
        ticks: (0..30)
            .map(|i| (f64::from(i) * 0.5, i.to_string()))
            .collect(),
    });
    let ticks = crate::ticks::right_ticks(&fig, 200.0);
    assert!(ticks.values.len() >= 2 && ticks.values.len() <= 4);
}
