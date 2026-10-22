use super::*;
use crate::legend_rect;
use plotium_figure::{Axis, ErrorBar, GuidePlacement, Series};

const PLOT: Rect = Rect {
    left: 40.0,
    top: 10.0,
    width: 300.0,
    height: 200.0,
};

fn figure(series: Vec<Series>) -> Figure {
    let mut f = Figure::new("", Axis::new("x", 0.0, 10.0), Axis::new("y", 0.0, 10.0));
    f.series = series;
    f.guide_visibility = plotium_figure::GuideVisibility::Show;
    f
}

fn rising(name: &str, offset: f64) -> Series {
    Series::line(
        name,
        (0..=100)
            .map(|i| {
                let x = i as f64 / 10.0;
                [x, x + offset]
            })
            .collect(),
    )
}

fn corner(r: Rect) -> (bool, bool) {
    let cx = r.left + r.width * 0.5;
    let cy = r.top + r.height * 0.5;
    (
        cx > PLOT.left + PLOT.width * 0.5,
        cy < PLOT.top + PLOT.height * 0.5,
    )
}

#[test]
fn uncovered_anchors_prefer_the_upper_right_with_a_consistent_inset() {
    let f = figure(vec![
        Series::points("A", vec![[5.0, 5.0]]),
        Series::points("B", vec![[5.0, 5.0]]),
    ]);
    let r = legend_rect(&f, PLOT, 1.0).unwrap();
    assert_eq!(r.right(), PLOT.right() - 8.0);
    assert_eq!(r.top, PLOT.top + 8.0);
}

#[test]
fn rising_curves_push_the_legend_to_an_empty_corner() {
    // y = x crosses the lower-left and upper-right; the upper-left is free.
    let f = figure(vec![rising("A", 0.0), rising("B", -0.5)]);
    let r = legend_rect(&f, PLOT, 1.0).unwrap();
    assert_eq!(corner(r), (false, true));
    assert_eq!(r.left, PLOT.left + 8.0);
    assert_eq!(r.top, PLOT.top + 8.0);
}

#[test]
fn falling_curves_free_the_upper_right() {
    let falling = |name: &str| {
        Series::line(
            name,
            (0..=100)
                .map(|i| {
                    let x = i as f64 / 10.0;
                    [x, 10.0 - x]
                })
                .collect(),
        )
    };
    let f = figure(vec![falling("A"), falling("B")]);
    assert_eq!(corner(legend_rect(&f, PLOT, 1.0).unwrap()), (true, true));
}

#[test]
fn sparse_line_segments_count_even_without_vertices_inside() {
    // Only two vertices: the diagonal itself must keep the legend off it.
    let f = figure(vec![
        Series::line("A", vec![[0.0, 10.0], [10.0, 0.0]]),
        Series::line("B", vec![[0.0, 9.0], [10.0, 0.0]]),
    ]);
    let r = legend_rect(&f, PLOT, 1.0).unwrap();
    assert!(!(segment_hits((PLOT.left, PLOT.top), (PLOT.right(), PLOT.bottom()), r)));
}

#[test]
fn stems_and_error_bars_occupy_space() {
    let mut f = figure(vec![
        Series::points("A", vec![[9.5, 9.5]]),
        Series::points("B", vec![[0.5, 9.5]]),
    ]);
    f.series[1].kind = SeriesKind::Stick;
    f.series[1].points = vec![[0.5, 9.5], [9.5, 9.5]];
    // Full-height stems at both sides leave only the centre column free.
    let r = legend_rect(&f, PLOT, 1.0).unwrap();
    assert!((r.left + r.width * 0.5 - (PLOT.left + PLOT.width * 0.5)).abs() < 1e-3);
    assert_eq!(r.top, PLOT.top + 8.0);
    f.series[1].points.clear();
    f.error_bars.push(ErrorBar {
        center: [0.5, 5.0],
        negative: 5.0,
        positive: 5.0,
        color: plotium_figure::Color::TRACE,
        width: 1.0,
        cap_width: 4.0,
        draw_over_data: false,
        cap_data_width: None,
        y_axis: plotium_figure::YAxisSide::Left,
    });
    let r = legend_rect(&f, PLOT, 1.0).unwrap();
    assert!(
        r.left > PLOT.left + 8.0,
        "the left edge is covered by the bar"
    );
}

#[test]
fn placement_scales_with_zoom_and_explicit_inside_stays_upper_right() {
    let mut f = figure(vec![rising("A", 0.0), rising("B", -0.5)]);
    let base = legend_rect(&f, PLOT, 1.0).unwrap();
    let zoomed = Rect::new(
        PLOT.left * 2.0,
        PLOT.top * 2.0,
        PLOT.width * 2.0,
        PLOT.height * 2.0,
    );
    let r = legend_rect(&f, zoomed, 2.0).unwrap();
    assert!((r.left - base.left * 2.0).abs() < 1e-3);
    assert!((r.top - base.top * 2.0).abs() < 1e-3);
    f.guide_placement = GuidePlacement::Inside;
    let r = legend_rect(&f, PLOT, 1.0).unwrap();
    assert_eq!(r.right(), PLOT.right() - 8.0);
    assert_eq!(r.top, PLOT.top + 8.0);
}

#[test]
fn dense_lines_are_pooled_without_losing_their_path() {
    // 100k samples along y = x must still block the upper right.
    let dense = |name: &str| {
        Series::line(
            name,
            (0..100_000)
                .map(|i| {
                    let x = i as f64 / 10_000.0;
                    [x, x]
                })
                .collect(),
        )
    };
    let f = figure(vec![dense("A"), dense("B")]);
    assert_eq!(corner(legend_rect(&f, PLOT, 1.0).unwrap()), (false, true));
}

#[test]
fn gaps_and_non_finite_samples_do_not_bridge_or_block() {
    // A NaN break splits the falling line; no segment joins its two halves
    // across the empty upper right, and NaN error bars count nowhere.
    let mut f = figure(vec![
        Series::line("A", vec![[0.0, 2.0], [f64::NAN, 5.0], [10.0, 2.0]]),
        Series::line("B", vec![[0.0, 1.0], [10.0, 1.0]]),
    ]);
    f.error_bars.push(ErrorBar {
        center: [f64::NAN, 5.0],
        negative: 1.0,
        positive: 1.0,
        color: plotium_figure::Color::TRACE,
        width: 1.0,
        cap_width: 4.0,
        draw_over_data: false,
        cap_data_width: None,
        y_axis: plotium_figure::YAxisSide::Left,
    });
    let r = legend_rect(&f, PLOT, 1.0).unwrap();
    assert_eq!(r.right(), PLOT.right() - 8.0);
    assert_eq!(r.top, PLOT.top + 8.0);
}

#[test]
fn oversize_legend_falls_back_to_the_upper_right_position() {
    let mut f = figure(vec![rising("A", 0.0), rising("B", -0.5)]);
    f.guide_title = "Title ".repeat(100);
    let r = legend_rect(&f, PLOT, 1.0).unwrap();
    assert_eq!(r.left, PLOT.left + 2.0);
    assert_eq!(r.top, PLOT.top + 8.0);
}
