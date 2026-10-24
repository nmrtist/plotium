use super::*;
use plotium_core::state::{CanvasViewport, PlotReferenceLine};
use plotium_figure::{ArrowHead, Axis, Color, ReferenceOrientation as Orientation};

const PLOT: PlotRect = PlotRect {
    left: 40.0,
    top: 10.0,
    width: 300.0,
    height: 200.0,
};

/// A 0–10 × 0–10 plot with an arrow from (2, 2) to (6, 6) and a vertical line
/// at x = 8. One data unit is 30 px across and 20 px up.
fn fixture() -> (PlotObject, AnnotationId, AnnotationId) {
    let mut annotations = PlotAnnotations::default();
    let arrow = annotations
        .add_arrow([2.0, 2.0], [6.0, 6.0], ArrowHead::Filled, 1.0, Color::BLACK)
        .expect("valid arrow");
    let line = annotations
        .add_reference_line(PlotReferenceLine::new(arrow, Orientation::Vertical, 8.0))
        .expect("valid line");
    let overrides = AxisOverrides {
        annotations,
        ..Default::default()
    };
    let figure = Figure::new("", Axis::new("x", 0.0, 10.0), Axis::new("y", 0.0, 10.0));
    let viewport = CanvasViewport::from_figure(&figure);
    let object = PlotObject::new(
        None,
        plotium_core::state::SeriesId::new(1),
        plotium_core::state::DataBinding {
            coordinates: None,
            series: Vec::new(),
        },
        plotium_core::state::ChartSpec::default(),
        plotium_core::state::StackSpec::default(),
        plotium_core::state::AxisProjections::default(),
        overrides,
        figure,
        viewport,
    );
    (object, arrow, line)
}

fn screen(x: f64, y: f64) -> Pos2 {
    Pos2::new(
        PLOT.left + (x / 10.0) as f32 * PLOT.width,
        PLOT.bottom() - (y / 10.0) as f32 * PLOT.height,
    )
}

#[test]
fn arrows_pick_by_their_ends_or_shaft_and_lines_by_their_stroke() {
    let (object, arrow, line) = fixture();
    let grip = |p: Pos2| mark_hit(&object, PLOT, 1.0, p).map(|hit| (hit.annotation, hit.grip));
    assert_eq!(grip(screen(6.0, 6.0)), Some((arrow, MarkGrip::Head)));
    assert_eq!(grip(screen(2.0, 2.0)), Some((arrow, MarkGrip::Tail)));
    assert_eq!(grip(screen(4.0, 4.0)), Some((arrow, MarkGrip::Whole)));
    assert_eq!(
        grip(screen(8.0, 3.0) + egui::vec2(3.0, 0.0)),
        Some((line, MarkGrip::Whole))
    );
    assert_eq!(grip(screen(4.0, 8.0)), None);
}

#[test]
fn a_dragged_arrow_moves_in_data_space_and_is_marked_as_hand_moved() {
    let (object, arrow, _) = fixture();
    let from = object.axis_overrides.annotations.clone();
    let moved = moved_mark(&object, PLOT, arrow, MarkGrip::Whole, &from, [30.0, -20.0])
        .expect("moved arrow");
    let moved = moved.annotations.arrow(arrow).expect("arrow");
    assert!(moved.hand_moved);
    for (got, want) in moved.at.iter().chain(&moved.to).zip([3.0, 3.0, 7.0, 7.0]) {
        assert!((got - want).abs() < 1e-4, "{moved:?}");
    }

    let head =
        moved_mark(&object, PLOT, arrow, MarkGrip::Head, &from, [60.0, 0.0]).expect("moved head");
    let head = head.annotations.arrow(arrow).expect("arrow");
    assert_eq!(head.at, [2.0, 2.0]);
    assert!((head.to[0] - 8.0).abs() < 1e-4);
}

#[test]
fn a_drag_keeps_the_whole_arrow_inside_the_plot() {
    let (object, arrow, _) = fixture();
    let from = object.axis_overrides.annotations.clone();
    let moved = moved_mark(&object, PLOT, arrow, MarkGrip::Whole, &from, [1000.0, 0.0])
        .expect("moved arrow");
    let moved = moved.annotations.arrow(arrow).expect("arrow");
    // The head stops at the right edge and the tail keeps its distance.
    assert!((moved.to[0] - 10.0).abs() < 1e-4);
    assert!((moved.at[0] - 6.0).abs() < 1e-4);
}

#[test]
fn dragging_one_arrow_end_onto_the_other_is_refused() {
    let (object, arrow, _) = fixture();
    let from = object.axis_overrides.annotations.clone();
    let onto = screen(2.0, 2.0) - screen(6.0, 6.0);
    assert!(
        moved_mark(
            &object,
            PLOT,
            arrow,
            MarkGrip::Head,
            &from,
            [onto.x, onto.y]
        )
        .is_none()
    );
}

#[test]
fn a_dragged_line_moves_across_itself_only() {
    let (object, _, line) = fixture();
    let from = object.axis_overrides.annotations.clone();
    let moved =
        moved_mark(&object, PLOT, line, MarkGrip::Whole, &from, [-30.0, 50.0]).expect("moved line");
    let moved = moved.annotations.reference_line(line).expect("line");
    assert!((moved.value - 7.0).abs() < 1e-4);
    assert!(moved.hand_moved);
    let bounds = mark_bounds(&object, PLOT, line).expect("line bounds");
    assert_eq!((bounds.top, bounds.height), (PLOT.top, PLOT.height));
}

#[test]
fn a_shift_along_a_line_does_not_move_it() {
    let (object, _, line) = fixture();
    let from = object.axis_overrides.annotations.clone();
    assert!(moved_mark(&object, PLOT, line, MarkGrip::Whole, &from, [0.0, 5.0]).is_none());
}
