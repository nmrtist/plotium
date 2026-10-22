use super::*;
use plotium_figure::{AxisBreak, AxisFrame, Figure};

fn broken(min: f64, max: f64, from: f64, to: f64) -> Axis {
    let mut axis = Axis::new("x", min, max);
    axis.axis_break = Some(AxisBreak::new(from, to));
    axis
}

#[test]
fn ticks_skip_the_hidden_values() {
    let axis = broken(0.0, 100.0, 20.0, 80.0);
    let ticks = crate::ticks::axis_ticks_for(&axis, 8);
    assert!(!ticks.values.is_empty());
    assert!(ticks.values.iter().all(|v| !axis.hides(*v)));
    assert!(ticks.values.iter().any(|v| *v <= 20.0));
    assert!(ticks.values.iter().any(|v| *v >= 80.0));
    assert_eq!(ticks.values.len(), ticks.labels.len());
}

#[test]
fn minor_ticks_stay_inside_the_pieces() {
    let axis = broken(0.0, 100.0, 20.0, 80.0);
    let majors = crate::ticks::axis_ticks_for(&axis, 8);
    let minors = crate::ticks::minor_tick_values(&axis, &majors, 1);
    assert!(!minors.is_empty());
    assert!(minors.iter().all(|v| !axis.hides(*v)));
}

#[test]
fn overlay_opens_the_gap_and_marks_both_ends() {
    let mut fig = Figure::new("", broken(0.0, 100.0, 20.0, 80.0), Axis::new("y", 0.0, 1.0));
    let plot = Rect::new(0.0, 0.0, 100.0, 50.0);
    let open = overlay(&fig, plot, plot.left, 1.0);
    assert_eq!(open.erase.len(), 1);
    assert_eq!(open.marks.len(), 2);
    let gap = open.erase[0];
    assert!(gap.left > 0.0 && gap.right() < 100.0);

    fig.axis_frame = AxisFrame::Box;
    assert_eq!(overlay(&fig, plot, plot.left, 1.0).marks.len(), 4);
    fig.axis_frame = AxisFrame::Hidden;
    let hidden = overlay(&fig, plot, plot.left, 1.0);
    assert_eq!((hidden.erase.len(), hidden.marks.len()), (1, 0));
}

#[test]
fn a_break_outside_the_window_draws_nothing() {
    let fig = Figure::new("", broken(0.0, 10.0, 20.0, 80.0), Axis::new("y", 0.0, 1.0));
    assert!(
        overlay(&fig, Rect::new(0.0, 0.0, 100.0, 50.0), 0.0, 1.0)
            .erase
            .is_empty()
    );
}

#[test]
fn layout_keeps_ticks_on_a_broken_axis() {
    let mut fig = Figure::new(
        "",
        broken(0.0, 100.0, 20.0, 80.0),
        Axis::new("y", 0.0, 1000.0),
    );
    fig.width = 300.0;
    fig.height = 200.0;
    let layout = crate::axis_layout(&fig, 300.0, 200.0);
    assert!(layout.x_ticks.values.len() >= 2);
    assert!(
        layout
            .x_ticks
            .values
            .iter()
            .all(|v| *v != 20.0 && *v != 80.0)
    );
}

#[test]
fn one_tick_pieces_keep_decimal_labels() {
    let axis = broken(0.1, 0.9, 0.35, 0.65);
    let ticks = crate::ticks::axis_ticks_for(&axis, 2);
    assert!(
        ticks.labels.iter().all(|label| label.contains('.')),
        "{:?}",
        ticks.labels
    );
}
