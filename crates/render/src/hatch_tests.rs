use super::hatch_lines;
use crate::Rect;
use plotium_figure::{
    Axis, Color, Figure, GuideVisibility, HATCH_SPACING_PT, Hatch, HatchPattern, Polygon,
};

const OUTLINE: [(f32, f32); 4] = [(10.0, 110.0), (40.0, 110.0), (40.0, 10.0), (10.0, 10.0)];
const CLIP: Rect = Rect {
    left: 0.0,
    top: 0.0,
    width: 200.0,
    height: 200.0,
};

#[test]
fn forward_lines_rise_to_the_right_inside_the_outline() {
    let lines = hatch_lines(&OUTLINE, HatchPattern::Forward, CLIP, 1.0);
    assert!(!lines.is_empty());
    for [a, b] in lines {
        let (left, right) = if a.0 < b.0 { (a, b) } else { (b, a) };
        assert!(right.1 < left.1, "y grows downward on the page");
        for (x, y) in [a, b] {
            assert!((10.0 - 1e-3..=40.0 + 1e-3).contains(&x));
            assert!((10.0 - 1e-3..=110.0 + 1e-3).contains(&y));
        }
    }
}

#[test]
fn neighbouring_lines_sit_one_spacing_apart() {
    let lines = hatch_lines(&OUTLINE, HatchPattern::Backward, CLIP, 2.0);
    let offset = |[a, _]: [(f32, f32); 2]| (a.0 - a.1) * std::f32::consts::FRAC_1_SQRT_2;
    let (a, b) = (offset(lines[0]), offset(lines[1]));
    assert!(((a - b).abs() - 2.0 * HATCH_SPACING_PT).abs() < 1e-3);
}

#[test]
fn crosshatch_draws_both_diagonals() {
    let one = hatch_lines(&OUTLINE, HatchPattern::Forward, CLIP, 1.0).len();
    let two = hatch_lines(&OUTLINE, HatchPattern::Backward, CLIP, 1.0).len();
    let both = hatch_lines(&OUTLINE, HatchPattern::Cross, CLIP, 1.0).len();
    assert_eq!(both, one + two);
}

#[test]
fn a_bar_far_larger_than_the_plot_builds_only_visible_lines() {
    let huge = [(-1e6, 1e6), (1e6, 1e6), (1e6, -1e6), (-1e6, -1e6)];
    let lines = hatch_lines(&huge, HatchPattern::Forward, CLIP, 1.0);
    assert!(lines.len() < 200, "{}", lines.len());
    assert!(!lines.is_empty());
}

#[test]
fn degenerate_outlines_draw_nothing() {
    assert!(hatch_lines(&[], HatchPattern::Cross, CLIP, 1.0).is_empty());
    let flat = [(5.0, 5.0), (5.0, 5.0), (5.0, 5.0)];
    assert!(hatch_lines(&flat, HatchPattern::Cross, CLIP, 1.0).is_empty());
}

#[test]
fn svg_draws_hatch_as_vector_lines_on_bars_and_legend_swatches() {
    let hatch = Hatch {
        pattern: HatchPattern::Forward,
        color: Color::rgb(0x12, 0x34, 0x56),
    };
    let mut figure = Figure::new("", Axis::new("x", 0.0, 1.0), Axis::new("y", 0.0, 1.0))
        .with_polygon(
            Polygon::rect("bars", 0.2, 0.8, 0.0, 0.9, Color::rgb(255, 255, 255)).with_hatch(hatch),
        );
    figure.guide_visibility = GuideVisibility::Show;
    let svg = crate::svg::export(&figure);
    let paths = svg.matches(r##"fill="none" stroke="#123456" stroke-width="0.5""##);
    assert_eq!(paths.count(), 2);
}
