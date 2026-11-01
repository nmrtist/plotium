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
        display_baseline: 0.0,
        center: [0.5, 5.0],
        negative: 5.0,
        positive: 5.0,
        color: plotium_figure::Color::TRACE,
        width: 1.0,
        cap_width: 4.0,
        draw_over_data: false,
        cap_data_width: None,
        y_axis: plotium_figure::YAxisSide::Left,
        horizontal: false,
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
        display_baseline: 0.0,
        center: [f64::NAN, 5.0],
        negative: 1.0,
        positive: 1.0,
        color: plotium_figure::Color::TRACE,
        width: 1.0,
        cap_width: 4.0,
        draw_over_data: false,
        cap_data_width: None,
        y_axis: plotium_figure::YAxisSide::Left,
        horizontal: false,
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

/// Ten long-named flat curves filling the lower 60% of the plot. A single
/// column is too tall for the free band above them.
fn ten_curves() -> Figure {
    figure(
        (0..10)
            .map(|i| {
                let y = f64::from(i) * 0.6;
                Series::line(
                    format!("HL01_op2_back.{:03}.xy", i * 4 + 2),
                    vec![[0.0, y], [10.0, y]],
                )
            })
            .collect(),
    )
}

fn columns(f: &Figure) -> usize {
    crate::placed_legend(f, PLOT, 1.0, None).unwrap().0.columns
}

#[test]
fn a_tall_legend_adds_columns_until_it_clears_the_data() {
    let f = ten_curves();
    assert_eq!(columns(&f), 2);
    let r = legend_rect(&f, PLOT, 1.0).unwrap();
    assert_eq!(r.top, PLOT.top + 8.0);
    assert_eq!(coverage(&f, PLOT, &[r]), [0]);
}

#[test]
fn a_legend_that_already_fits_keeps_one_column() {
    let f = figure(vec![
        Series::points("A", vec![[5.0, 5.0]]),
        Series::points("B", vec![[5.0, 5.0]]),
    ]);
    assert_eq!(columns(&f), 1);
}

#[test]
fn extra_columns_must_fit_inside_the_plot() {
    let f = ten_curves();
    let narrow = Rect::new(40.0, 10.0, 150.0, 200.0);
    let (layout, _) = crate::placed_legend(&f, narrow, 1.0, None).unwrap();
    assert_eq!(layout.columns, 1);
}

#[test]
fn authored_settings_fix_the_column_count() {
    let mut f = ten_curves();
    f.guide_columns = Some(3);
    assert_eq!(columns(&f), 3);
    f.guide_columns = Some(40);
    assert_eq!(columns(&f), 10);
    f.guide_columns = None;
    f.guide_layout = plotium_figure::GuideLayout::Vertical;
    assert_eq!(columns(&f), 1);
    f.guide_layout = plotium_figure::GuideLayout::Horizontal;
    assert_eq!(columns(&f), 10);
    f.guide_columns = Some(5);
    assert_eq!(columns(&f), 5);
}

#[test]
fn only_automatic_placement_searches_columns() {
    let mut f = ten_curves();
    f.guide_placement = GuidePlacement::OutsideRight;
    assert_eq!(columns(&f), 1);
    f.guide_placement = GuidePlacement::Inside;
    assert_eq!(columns(&f), 1);
}

#[test]
fn a_pinned_legend_keeps_its_size_when_placed_by_hand() {
    let mut f = ten_curves();
    let before = legend_rect(&f, PLOT, 1.0).unwrap();
    let pinned = crate::legend_columns_to_pin(&f, PLOT, 1.0);
    assert_eq!(pinned, Some(2));
    f.guide_columns = pinned;
    f.guide_placement = GuidePlacement::Inside;
    f.legend_position = Some([0.2, 0.3]);
    let after = legend_rect(&f, PLOT, 1.0).unwrap();
    assert_eq!((after.width, after.height), (before.width, before.height));
    assert_eq!(crate::legend_columns_to_pin(&f, PLOT, 1.0), None);
}

#[test]
fn a_hand_placed_legend_never_searches_columns() {
    let mut f = ten_curves();
    f.legend_position = Some([0.2, 0.3]);
    assert_eq!(columns(&f), 1);
    f.guide_placement = GuidePlacement::Inside;
    assert_eq!(columns(&f), 1);
}

#[test]
fn a_single_column_needs_no_pin() {
    let f = figure(vec![
        Series::points("A", vec![[5.0, 5.0]]),
        Series::points("B", vec![[5.0, 5.0]]),
    ]);
    assert_eq!(crate::legend_columns_to_pin(&f, PLOT, 1.0), None);
}

#[test]
fn legend_sizes_follow_the_text_size() {
    let mut f = ten_curves();
    let (normal, _) = crate::placed_legend(&f, PLOT, 1.0, None).unwrap();
    f.typography.legend_pt *= 0.5;
    let (small, _) = crate::placed_legend(&f, PLOT, 1.0, None).unwrap();
    assert_eq!(small.row, normal.row * 0.5);
    assert_eq!(small.swatch, normal.swatch * 0.5);
    assert_eq!(small.padding, normal.padding * 0.5);
    assert!(normal.row < f.typography.legend_pt * 2.0 * 1.3);
}
