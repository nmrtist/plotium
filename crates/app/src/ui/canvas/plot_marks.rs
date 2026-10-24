//! Picking and moving author arrows and reference lines inside a plot.
use super::*;
use plotium_core::state::{AnnotationId, AxisOverrides, MarkGrip, PlotAnnotations, PlotObject};
use plotium_figure::{Figure, ReferenceOrientation};

/// Screen px around a mark's stroke that still picks it.
const PICK_PX: f32 = 4.0;
/// Screen px around an arrow end within which a drag moves only that end.
const END_PX: f32 = 6.0;

/// An arrow or reference line under the pointer.
#[derive(Clone, Copy, Debug)]
pub(super) struct MarkHit {
    pub annotation: AnnotationId,
    pub grip: MarkGrip,
}

fn project(figure: &Figure, plot: PlotRect, point: [f64; 2]) -> Option<Pos2> {
    let p = Pos2::new(
        to_screen_x(point[0], plot, &figure.x),
        to_screen_y(point[1], plot, &figure.y),
    );
    (p.x.is_finite() && p.y.is_finite()).then_some(p)
}

fn unproject(figure: &Figure, plot: PlotRect, p: Pos2) -> Option<[f64; 2]> {
    let point = [
        from_screen_x(p.x, plot, &figure.x),
        from_screen_y(p.y, plot, &figure.y),
    ];
    point.iter().all(|v| v.is_finite()).then_some(point)
}

fn segment_distance(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let length_sq = ab.length_sq();
    let t = if length_sq > 0.0 {
        ((p - a).dot(ab) / length_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.distance(a + ab * t)
}

/// Screen x of a vertical line, or y of a horizontal one.
fn line_screen(
    figure: &Figure,
    plot: PlotRect,
    orientation: ReferenceOrientation,
    value: f64,
) -> f32 {
    match orientation {
        ReferenceOrientation::Vertical => to_screen_x(value, plot, &figure.x),
        ReferenceOrientation::Horizontal => to_screen_y(value, plot, &figure.y),
    }
}

/// The topmost arrow or reference line of `plot_object` near `pointer`.
/// Arrows are drawn over reference lines, so they are tested first.
pub(super) fn mark_hit(
    plot_object: &PlotObject,
    plot: PlotRect,
    scale: f32,
    pointer: Pos2,
) -> Option<MarkHit> {
    let annotations = &plot_object.axis_overrides.annotations;
    let figure = plot_object.figure();
    for arrow in annotations.arrows.iter().rev() {
        let (Some(tail), Some(head)) = (
            project(figure, plot, arrow.at),
            project(figure, plot, arrow.to),
        ) else {
            continue;
        };
        let reach = PICK_PX + 0.5 * arrow.line_width * scale;
        // A short arrow is moved whole, so its ends never hide its middle.
        let ends = tail.distance(head) >= 3.0 * END_PX;
        let grip = if ends && pointer.distance(head) <= END_PX {
            MarkGrip::Head
        } else if ends && pointer.distance(tail) <= END_PX {
            MarkGrip::Tail
        } else if segment_distance(pointer, tail, head) <= reach {
            MarkGrip::Whole
        } else {
            continue;
        };
        return Some(MarkHit {
            annotation: arrow.id,
            grip,
        });
    }
    annotations.reference_lines.iter().rev().find_map(|line| {
        let at = line_screen(figure, plot, line.orientation, line.value);
        let along = match line.orientation {
            ReferenceOrientation::Vertical => pointer.x,
            ReferenceOrientation::Horizontal => pointer.y,
        };
        ((along - at).abs() <= PICK_PX + 0.5 * line.width * scale).then_some(MarkHit {
            annotation: line.id,
            grip: MarkGrip::Whole,
        })
    })
}

/// `delta` limited so every point in `xs` stays within `lo..=hi`; a point
/// already outside may move back in but not further out.
fn limit(delta: f32, xs: &[f32], lo: f32, hi: f32) -> f32 {
    let min = xs.iter().copied().fold(f32::INFINITY, f32::min);
    let max = xs.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    delta.clamp((lo - min).min(0.0), (hi - max).max(0.0))
}

/// `overrides` with mark `annotation` moved `delta` screen px from where it
/// sits in `from`, and marked as moved by hand. The moved part stays inside
/// the plot, where it is drawn and can be grabbed again.
pub(super) fn moved_mark(
    plot_object: &PlotObject,
    plot: PlotRect,
    annotation: AnnotationId,
    grip: MarkGrip,
    from: &PlotAnnotations,
    delta: [f32; 2],
) -> Option<AxisOverrides> {
    let figure = plot_object.figure();
    let mut overrides = plot_object.axis_overrides.clone();
    let annotations = &mut overrides.annotations;
    if let Some(start) = from.arrow(annotation) {
        let tail = project(figure, plot, start.at)?;
        let head = project(figure, plot, start.to)?;
        let moving: &[Pos2] = match grip {
            MarkGrip::Whole => &[tail, head],
            MarkGrip::Tail => &[tail],
            MarkGrip::Head => &[head],
        };
        let xs: Vec<f32> = moving.iter().map(|p| p.x).collect();
        let ys: Vec<f32> = moving.iter().map(|p| p.y).collect();
        let shift = egui::vec2(
            limit(delta[0], &xs, plot.left, plot.right()),
            limit(delta[1], &ys, plot.top, plot.bottom()),
        );
        if shift == egui::Vec2::ZERO {
            return None;
        }
        let tail = match grip {
            MarkGrip::Whole | MarkGrip::Tail => tail + shift,
            MarkGrip::Head => tail,
        };
        let head = match grip {
            MarkGrip::Whole | MarkGrip::Head => head + shift,
            MarkGrip::Tail => head,
        };
        // Dragging one end onto the other leaves no arrow to draw.
        if tail.distance(head) < 1.0 {
            return None;
        }
        let arrow = annotations.arrow_mut(annotation)?;
        // The end that stays keeps its exact coordinates.
        if grip != MarkGrip::Head {
            arrow.at = unproject(figure, plot, tail)?;
        }
        if grip != MarkGrip::Tail {
            arrow.to = unproject(figure, plot, head)?;
        }
        arrow.validate().ok()?;
        arrow.hand_moved = true;
        return Some(overrides);
    }
    let start = from.reference_line(annotation)?;
    let at = line_screen(figure, plot, start.orientation, start.value);
    // A line only moves across itself.
    let shift = match start.orientation {
        ReferenceOrientation::Vertical => limit(delta[0], &[at], plot.left, plot.right()),
        ReferenceOrientation::Horizontal => limit(delta[1], &[at], plot.top, plot.bottom()),
    };
    if shift == 0.0 {
        return None;
    }
    let value = match start.orientation {
        ReferenceOrientation::Vertical => from_screen_x(at + shift, plot, &figure.x),
        ReferenceOrientation::Horizontal => from_screen_y(at + shift, plot, &figure.y),
    };
    if !value.is_finite() {
        return None;
    }
    let line = annotations.reference_line_mut(annotation)?;
    line.value = value;
    line.hand_moved = true;
    Some(overrides)
}

/// The screen box mark `annotation` is drawn in: an arrow's span, or the
/// strip a reference line runs along.
pub(super) fn mark_bounds(
    plot_object: &PlotObject,
    plot: PlotRect,
    annotation: AnnotationId,
) -> Option<PlotRect> {
    let annotations = &plot_object.axis_overrides.annotations;
    let figure = plot_object.figure();
    if let Some(arrow) = annotations.arrow(annotation) {
        let tail = project(figure, plot, arrow.at)?;
        let head = project(figure, plot, arrow.to)?;
        let min = tail.min(head);
        let max = tail.max(head);
        return Some(PlotRect::new(min.x, min.y, max.x - min.x, max.y - min.y));
    }
    let line = annotations.reference_line(annotation)?;
    let at = line_screen(figure, plot, line.orientation, line.value);
    Some(match line.orientation {
        ReferenceOrientation::Vertical => PlotRect::new(at, plot.top, 0.0, plot.height),
        ReferenceOrientation::Horizontal => PlotRect::new(plot.left, at, plot.width, 0.0),
    })
}

/// The screen points of an arrow's two ends, which a drag can move apart.
pub(super) fn arrow_ends(
    plot_object: &PlotObject,
    plot: PlotRect,
    annotation: AnnotationId,
) -> Option<[Pos2; 2]> {
    let arrow = plot_object.axis_overrides.annotations.arrow(annotation)?;
    let figure = plot_object.figure();
    Some([
        project(figure, plot, arrow.at)?,
        project(figure, plot, arrow.to)?,
    ])
}

#[cfg(test)]
#[path = "plot_marks_tests.rs"]
mod tests;
