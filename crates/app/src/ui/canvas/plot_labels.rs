//! Dragging author text annotations and reference-line labels inside a plot.
use super::*;
use plotium_core::state::{
    AnnotationId, AxisOverrides, LabelPlacement, MAX_LABEL_OFFSET_PT, PlotObject,
};
use plotium_figure::{ReferenceLabelPosition, ReferenceLabelSide, ReferenceOrientation};

/// A plot label under the pointer.
#[derive(Clone, Copy, Debug)]
pub(super) struct LabelHit {
    pub annotation: AnnotationId,
    /// The point a drag moves: a text's baseline centre, or a reference
    /// label's box centre.
    pub origin: [f32; 2],
}

fn centre(rect: PlotRect) -> [f32; 2] {
    [rect.left + 0.5 * rect.width, rect.top + 0.5 * rect.height]
}

/// The topmost label of `plot_object` whose box `contains` accepts. Texts are
/// drawn over reference labels, so they are tested first.
pub(super) fn label_hit(
    plot_object: &PlotObject,
    plot: PlotRect,
    scale: f32,
    contains: impl Fn(PlotRect) -> bool,
) -> Option<LabelHit> {
    let annotations = &plot_object.axis_overrides.annotations;
    if annotations.items.is_empty() && annotations.reference_lines.is_empty() {
        return None;
    }
    let layout = plotium_render::label_layout(plot_object.figure(), plot, scale);
    let texts = layout
        .texts
        .iter()
        .zip(&annotations.items)
        .rev()
        .filter_map(|(label, a)| label.map(|l| (l.bounds, a.id, [l.x, l.y])));
    let lines = layout
        .reference
        .iter()
        .zip(&annotations.reference_lines)
        .rev()
        .filter_map(|(label, line)| label.map(|l| (l.bounds, line.id, centre(l.bounds))));
    texts
        .chain(lines)
        .find(|(bounds, ..)| contains(*bounds))
        .map(|(_, annotation, origin)| LabelHit { annotation, origin })
}

/// Whether `origin` differs from where a label's reference point is drawn;
/// a click without movement leaves the label's placement alone.
fn moves(origin: [f32; 2], drawn: [f32; 2]) -> bool {
    (origin[0] - drawn[0]).hypot(origin[1] - drawn[1]) >= 0.5
}

/// The screen box label `annotation` is drawn in.
pub(super) fn label_bounds(
    plot_object: &PlotObject,
    plot: PlotRect,
    scale: f32,
    annotation: AnnotationId,
) -> Option<PlotRect> {
    let annotations = &plot_object.axis_overrides.annotations;
    let layout = plotium_render::label_layout(plot_object.figure(), plot, scale);
    if let Some(index) = annotations.items.iter().position(|a| a.id == annotation) {
        return layout.texts.get(index).copied().flatten().map(|l| l.bounds);
    }
    let index = annotations
        .reference_lines
        .iter()
        .position(|l| l.id == annotation)?;
    layout
        .reference
        .get(index)
        .copied()
        .flatten()
        .map(|l| l.bounds)
}

/// `overrides` with label `annotation` moved so its reference point sits at
/// `origin`, placed by hand; `None` when nothing moves.
pub(super) fn moved_label(
    plot_object: &PlotObject,
    plot: PlotRect,
    scale: f32,
    annotation: AnnotationId,
    origin: [f32; 2],
) -> Option<AxisOverrides> {
    place_label(plot_object, plot, scale, annotation, Placing::To(origin))
}

/// `overrides` with label `annotation` shifted by `delta` screen px, placed by
/// hand. Unlike a drag, a shift below half a pixel still moves it: a keyboard
/// step stays one fixed distance at any zoom.
pub(super) fn nudged_label(
    plot_object: &PlotObject,
    plot: PlotRect,
    scale: f32,
    annotation: AnnotationId,
    delta: [f32; 2],
) -> Option<AxisOverrides> {
    place_label(plot_object, plot, scale, annotation, Placing::By(delta))
}

#[derive(Clone, Copy)]
enum Placing {
    To([f32; 2]),
    By([f32; 2]),
}

fn place_label(
    plot_object: &PlotObject,
    plot: PlotRect,
    scale: f32,
    annotation: AnnotationId,
    placing: Placing,
) -> Option<AxisOverrides> {
    // A label stays inside the plot, where it is drawn and can be grabbed.
    let inside = |origin: [f32; 2]| {
        [
            origin[0].clamp(plot.left, plot.right()),
            origin[1].clamp(plot.top, plot.bottom()),
        ]
    };
    let target = |drawn: [f32; 2]| match placing {
        Placing::To(origin) => Some(inside(origin)).filter(|origin| moves(*origin, drawn)),
        Placing::By(delta) => Some(inside([drawn[0] + delta[0], drawn[1] + delta[1]])),
    };
    let mut overrides = plot_object.axis_overrides.clone();
    let annotations = &mut overrides.annotations;
    let layout = plotium_render::label_layout(plot_object.figure(), plot, scale);
    if let Some(index) = annotations.items.iter().position(|a| a.id == annotation) {
        let drawn = layout.texts.get(index).copied().flatten()?;
        let origin = target([drawn.x, drawn.y])?;
        // The projected anchor, recovered from where the text is drawn.
        let anchor = [
            drawn.x - drawn.offset_pt[0] * scale,
            drawn.y + drawn.offset_pt[1] * scale,
        ];
        let limit = |v: f32| v.clamp(-MAX_LABEL_OFFSET_PT, MAX_LABEL_OFFSET_PT);
        annotations.items[index].placement = LabelPlacement::Hand([
            limit((origin[0] - anchor[0]) / scale),
            limit((anchor[1] - origin[1]) / scale),
        ]);
        return Some(overrides);
    }
    let index = annotations
        .reference_lines
        .iter()
        .position(|l| l.id == annotation)?;
    let drawn = layout.reference.get(index).copied().flatten()?;
    let origin = target(centre(drawn.bounds))?;
    let line = &mut annotations.reference_lines[index];
    let side = |before: bool| {
        if before {
            ReferenceLabelSide::Before
        } else {
            ReferenceLabelSide::After
        }
    };
    // A label rides along its line; a keyboard step across the line picks the
    // side it steps towards, since a small shift would never reach the line.
    let across = |at: f32, step: f32| match placing {
        Placing::By(_) if step != 0.0 => step < 0.0,
        _ => at < drawn.line,
    };
    let delta = match placing {
        Placing::By(delta) => delta,
        Placing::To(_) => [0.0, 0.0],
    };
    let position = match line.orientation {
        ReferenceOrientation::Vertical => ReferenceLabelPosition {
            along: ((plot.bottom() - origin[1]) / plot.height).clamp(0.0, 1.0),
            side: side(across(origin[0], delta[0])),
        },
        ReferenceOrientation::Horizontal => ReferenceLabelPosition {
            along: ((origin[0] - plot.left) / plot.width).clamp(0.0, 1.0),
            side: side(across(origin[1], delta[1])),
        },
    };
    line.label_placement = LabelPlacement::Hand(position);
    Some(overrides)
}

/// `overrides` with label `annotation` returned to automatic placement.
pub(super) fn auto_label(plot_object: &PlotObject, annotation: AnnotationId) -> AxisOverrides {
    let mut overrides = plot_object.axis_overrides.clone();
    let annotations = &mut overrides.annotations;
    if let Some(text) = annotations.get_mut(annotation) {
        text.placement = LabelPlacement::Auto;
    } else if let Some(line) = annotations.reference_line_mut(annotation) {
        line.label_placement = LabelPlacement::Auto;
    }
    overrides
}

#[cfg(test)]
#[path = "plot_labels_tests.rs"]
mod tests;
