use super::*;
use plotium_core::state::{CanvasViewport, PlotAnnotations, PlotReferenceLine};
use plotium_figure::{Axis, Figure};

const PLOT: PlotRect = PlotRect {
    left: 40.0,
    top: 10.0,
    width: 300.0,
    height: 200.0,
};

fn plot_object(overrides: AxisOverrides, x_max: f64) -> PlotObject {
    let mut figure = Figure::new("", Axis::new("x", 0.0, x_max), Axis::new("y", 0.0, 10.0));
    figure.author_annotations = overrides.annotations.lower();
    figure.reference_lines = overrides.annotations.lower_reference_lines();
    let viewport = CanvasViewport::from_figure(&figure);
    PlotObject::new(
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
    )
}

fn fixture() -> (AxisOverrides, AnnotationId, AnnotationId) {
    let mut annotations = PlotAnnotations::default();
    let text = annotations
        .add_text([5.0, 5.0], "peak".into(), 8.0)
        .expect("valid text");
    let mut line = PlotReferenceLine::new(text, ReferenceOrientation::Vertical, 2.0);
    line.label = "onset".into();
    let line = annotations.add_reference_line(line).expect("valid line");
    let overrides = AxisOverrides {
        annotations,
        ..Default::default()
    };
    (overrides, text, line)
}

fn hit_at(object: &PlotObject, origin: [f32; 2]) -> Option<LabelHit> {
    label_hit(object, PLOT, 1.0, |rect| {
        rect.left <= origin[0]
            && origin[0] <= rect.right()
            && rect.top <= origin[1]
            && origin[1] <= rect.bottom()
    })
}

#[test]
fn a_dragged_text_keeps_its_offset_through_range_changes() {
    let (overrides, text, _) = fixture();
    let object = plot_object(overrides, 10.0);
    let anchor = [PLOT.left + 0.5 * PLOT.width, PLOT.top + 0.5 * PLOT.height];
    let hit = hit_at(&object, [anchor[0], anchor[1] - 2.0]).expect("text under pointer");
    assert_eq!(hit.annotation, text);
    assert!(moved_label(&object, PLOT, 1.0, text, hit.origin).is_none());
    let moved = moved_label(
        &object,
        PLOT,
        1.0,
        text,
        [anchor[0] + 20.0, anchor[1] - 10.0],
    )
    .expect("moved text");
    let placement = moved.annotations.get(text).expect("text").placement;
    assert_eq!(placement, LabelPlacement::Hand([20.0, 10.0]));
    // A new x range moves the anchor; the text keeps its offset from it.
    let rescaled = plot_object(moved, 20.0);
    let drawn =
        plotium_render::label_layout(rescaled.figure(), PLOT, 1.0).texts[0].expect("placed text");
    assert_eq!(drawn.offset_pt, [20.0, 10.0]);
    assert!((drawn.x - (PLOT.left + 0.25 * PLOT.width + 20.0)).abs() < 1e-3);
    let reset = auto_label(&rescaled, text);
    assert_eq!(
        reset.annotations.get(text).expect("text").placement,
        LabelPlacement::Auto
    );
}

#[test]
fn a_dragged_reference_label_records_its_side_and_place_along_the_line() {
    let (overrides, _, line) = fixture();
    let object = plot_object(overrides, 10.0);
    let x = PLOT.left + 0.2 * PLOT.width;
    let target = [x - 15.0, PLOT.top + 0.75 * PLOT.height];
    let moved = moved_label(&object, PLOT, 1.0, line, target).expect("moved label");
    let placement = moved
        .annotations
        .reference_line(line)
        .expect("line")
        .label_placement;
    let LabelPlacement::Hand(position) = placement else {
        panic!("hand placement expected, got {placement:?}");
    };
    assert_eq!(position.side, ReferenceLabelSide::Before);
    assert!((position.along - 0.25).abs() < 1e-3);
}

#[test]
fn a_nudged_text_shifts_its_offset_by_the_step_even_below_half_a_pixel() {
    let (overrides, text, _) = fixture();
    let object = plot_object(overrides, 10.0);
    let nudged = nudged_label(&object, PLOT, 0.5, text, [0.25, -0.25]).expect("nudged text");
    let LabelPlacement::Hand(offset) = nudged.annotations.get(text).expect("text").placement else {
        panic!("hand placement expected");
    };
    // Half a screen pixel at zoom 0.5 is 0.5 pt; screen up is offset up.
    let layout = plotium_render::label_layout(object.figure(), PLOT, 0.5).texts[0].expect("text");
    assert!((offset[0] - (layout.offset_pt[0] + 0.5)).abs() < 1e-3);
    assert!((offset[1] - (layout.offset_pt[1] + 0.5)).abs() < 1e-3);
}

#[test]
fn a_nudge_across_a_reference_line_picks_the_side_it_steps_towards() {
    let (overrides, _, line) = fixture();
    let object = plot_object(overrides, 10.0);
    let side = |delta: [f32; 2]| {
        let nudged = nudged_label(&object, PLOT, 1.0, line, delta).expect("nudged label");
        match nudged
            .annotations
            .reference_line(line)
            .expect("line")
            .label_placement
        {
            LabelPlacement::Hand(position) => position,
            other => panic!("hand placement expected, got {other:?}"),
        }
    };
    assert_eq!(side([-0.5, 0.0]).side, ReferenceLabelSide::Before);
    assert_eq!(side([0.5, 0.0]).side, ReferenceLabelSide::After);
    let up = side([0.0, -5.0]);
    let down = side([0.0, 5.0]);
    assert!(up.along > down.along);
}
