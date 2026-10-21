use super::*;
use crate::legend_rect;
use plotium_figure::{GuideVisibility, Series};

fn figure(series: Vec<Series>) -> Figure {
    let mut f = Figure::new("", Axis::new("x", 0.0, 10.0), Axis::new("y", 0.0, 10.0));
    f.series = series;
    f.guide_visibility = GuideVisibility::Show;
    f
}

const SIZE: [f32; 2] = [320.0, 220.0];

/// Horizontal lines every half unit leave no clear anchor inside the plot;
/// only the first two are named, so the legend stays small.
fn crowded() -> Figure {
    figure(
        (0..=20)
            .map(|i| {
                let y = f64::from(i) * 0.5;
                let name = if i < 2 {
                    format!("S{i}")
                } else {
                    String::new()
                };
                Series::line(&name, vec![[0.0, y], [10.0, y]])
            })
            .collect(),
    )
}

fn legend_is_clear(f: &Figure) -> bool {
    let layout = axis_layout(f, SIZE[0], SIZE[1]);
    let outer = Rect::new(0.0, 0.0, SIZE[0], SIZE[1]);
    let plot = Projector::new(f, outer, &layout.margins).plot;
    let r = legend_rect(f, plot, 1.0).unwrap();
    coverage(f, plot, &[grow(r, LEGEND_INSET * 0.25)]) == [0]
}

fn window(axis: &Axis) -> [f64; 2] {
    [axis.min, axis.max]
}

#[test]
fn a_clear_plot_keeps_its_axis_windows() {
    let mut f = figure(vec![
        Series::line("A", vec![[0.0, 0.0], [10.0, 10.0]]),
        Series::line("B", vec![[0.0, 0.0], [10.0, 9.0]]),
    ]);
    assert!(!make_legend_room(&mut f, SIZE, true, true));
    assert_eq!((window(&f.x), window(&f.y)), ([0.0, 10.0], [0.0, 10.0]));
}

#[test]
fn a_crowded_plot_grows_one_automatic_end_until_the_legend_is_clear() {
    let mut f = crowded();
    assert!(!legend_is_clear(&f));
    assert!(make_legend_room(&mut f, SIZE, true, true));
    assert!(legend_is_clear(&f));
    let moved = [
        f.x.min != 0.0,
        f.x.max != 10.0,
        f.y.min != 0.0,
        f.y.max != 10.0,
    ];
    assert!(f.x.min <= 0.0 && f.x.max >= 10.0 && f.y.min <= 0.0 && f.y.max >= 10.0);
    assert!((1..=2).contains(&moved.iter().filter(|m| **m).count()));
    assert!(!(moved[0] && moved[1]) && !(moved[2] && moved[3]));
    // The figure is already clear, so a second pass changes nothing.
    let before = (window(&f.x), window(&f.y));
    assert!(!make_legend_room(&mut f, SIZE, true, true));
    assert_eq!((window(&f.x), window(&f.y)), before);
}

#[test]
fn only_growable_axes_move_and_reversed_axes_grow_on_screen_side() {
    let mut f = crowded();
    f.x.reversed = true;
    assert!(make_legend_room(&mut f, SIZE, false, true));
    assert_eq!(window(&f.x), [0.0, 10.0]);
    assert!(legend_is_clear(&f));

    // A reversed y axis puts the upper anchors at its minimum.
    let mut f = crowded();
    f.y.reversed = true;
    assert!(make_legend_room(&mut f, SIZE, false, true));
    assert!(f.y.min < 0.0 && f.y.max == 10.0);
    assert!(legend_is_clear(&f));

    // Horizontal lines can only be cleared along x when y is fixed; a
    // reversed x axis puts the right-hand anchors at its minimum.
    let mut f = crowded();
    f.x.reversed = true;
    assert!(make_legend_room(&mut f, SIZE, true, false));
    assert_eq!(window(&f.y), [0.0, 10.0]);
    assert!(f.x.min < 0.0 && f.x.max == 10.0);
    assert!(legend_is_clear(&f));
}

#[test]
fn fixed_axes_manual_legends_and_explicit_placement_are_left_alone() {
    let mut f = crowded();
    assert!(!make_legend_room(&mut f, SIZE, false, false));
    let mut f = crowded();
    f.guide_placement = GuidePlacement::Inside;
    assert!(!make_legend_room(&mut f, SIZE, true, true));
    let mut f = crowded();
    f.legend_position = Some([0.5, 0.5]);
    assert!(!make_legend_room(&mut f, SIZE, true, true));
    let mut f = crowded();
    f.lock_aspect = true;
    assert!(!make_legend_room(&mut f, SIZE, true, true));
    let mut f = crowded();
    f.y.categories = Some(vec!["a".into(); 11]);
    assert!(make_legend_room(&mut f, SIZE, true, true));
    assert_eq!(window(&f.y), [0.0, 10.0]);
}
