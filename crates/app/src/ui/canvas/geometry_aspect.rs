use super::*;

pub(crate) fn preserve_aspect_frame(
    before: ObjectFrame,
    candidate: ObjectFrame,
    kind: ObjectDragKind,
) -> ObjectFrame {
    let ObjectDragKind::Resize(handle) = kind else {
        return candidate;
    };
    let [width, height] =
        plotium_core::state::aspect_locked_size(before, candidate, MIN_OBJECT_SIZE_PT);
    let right = before.x + before.width;
    let bottom = before.y + before.height;
    match handle {
        ResizeHandle::TopLeft => ObjectFrame::new(right - width, bottom - height, width, height),
        ResizeHandle::TopRight => ObjectFrame::new(before.x, bottom - height, width, height),
        ResizeHandle::BottomLeft => ObjectFrame::new(right - width, before.y, width, height),
        ResizeHandle::BottomRight => ObjectFrame::new(before.x, before.y, width, height),
    }
}

/// Data area a resize must leave inside a plot's axes, ticks and colour scale.
/// Without it the fixed margins swallow the whole frame and the data collapses
/// to a 1 pt sliver while the furniture keeps its size.
const MIN_PLOT_AREA_PT: f32 = 24.0;

/// The smallest frame a plot may be resized to. Margins are measured at the
/// frame's current size, and a plot already smaller than the limit is never
/// forced to grow.
pub(crate) fn plot_min_size(figure: &plotium_figure::Figure, frame: ObjectFrame) -> [f32; 2] {
    let margins = plotium_render::axis_layout(figure, frame.width, frame.height).margins;
    [
        (margins.left + margins.right + MIN_PLOT_AREA_PT).min(frame.width),
        (margins.top + margins.bottom + MIN_PLOT_AREA_PT).min(frame.height),
    ]
}

/// Grow a resized `candidate` to at least `min`, keeping the edges opposite the
/// handle where they were in `before`; the candidate's own anchored edges drift
/// once the drag clamps to the minimum object size. `keep_aspect` grows both
/// axes by one factor so an aspect-preserving resize stays proportional.
pub(crate) fn enforce_min_size(
    before: ObjectFrame,
    candidate: ObjectFrame,
    kind: ObjectDragKind,
    min: [f32; 2],
    keep_aspect: bool,
) -> ObjectFrame {
    let ObjectDragKind::Resize(handle) = kind else {
        return candidate;
    };
    let [mut width, mut height] = [candidate.width, candidate.height];
    if keep_aspect {
        let grow = (min[0] / width).max(min[1] / height).max(1.0);
        width *= grow;
        height *= grow;
    } else {
        width = width.max(min[0]);
        height = height.max(min[1]);
    }
    let moves_left = matches!(handle, ResizeHandle::TopLeft | ResizeHandle::BottomLeft);
    let moves_top = matches!(handle, ResizeHandle::TopLeft | ResizeHandle::TopRight);
    if width == candidate.width && height == candidate.height {
        return candidate;
    }
    let x = if moves_left {
        before.x + before.width - width
    } else {
        before.x
    };
    let y = if moves_top {
        before.y + before.height - height
    } else {
        before.y
    };
    ObjectFrame::new(x, y, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_size_keeps_the_anchored_edges() {
        let before = ObjectFrame::new(0.0, 0.0, 100.0, 15.0);
        // A drag past the opposite edge, already clamped to the object minimum.
        let candidate = ObjectFrame::new(200.0, 10.0, 24.0, 24.0);
        let frame = enforce_min_size(
            before,
            candidate,
            ObjectDragKind::Resize(ResizeHandle::TopLeft),
            [40.0, 30.0],
            false,
        );
        assert_eq!(frame, ObjectFrame::new(60.0, -15.0, 40.0, 30.0));
        let candidate = ObjectFrame::new(0.0, 0.0, 10.0, 5.0);
        let frame = enforce_min_size(
            before,
            candidate,
            ObjectDragKind::Resize(ResizeHandle::BottomRight),
            [40.0, 30.0],
            false,
        );
        assert_eq!(frame, ObjectFrame::new(0.0, 0.0, 40.0, 30.0));
    }

    #[test]
    fn aspect_kept_min_size_grows_both_axes_together() {
        let candidate = ObjectFrame::new(0.0, 0.0, 20.0, 10.0);
        let frame = enforce_min_size(
            ObjectFrame::new(0.0, 0.0, 60.0, 30.0),
            candidate,
            ObjectDragKind::Resize(ResizeHandle::BottomRight),
            [40.0, 10.0],
            true,
        );
        assert_eq!(frame, ObjectFrame::new(0.0, 0.0, 40.0, 20.0));
    }

    #[test]
    fn colour_scaled_plot_keeps_room_for_its_data() {
        let mut figure = plotium_figure::Figure::new(
            "",
            plotium_figure::Axis::new("x", 0.0, 10.0),
            plotium_figure::Axis::new("y", 0.0, 10.0),
        );
        figure.heatmap = Some(plotium_figure::HeatmapGrid {
            rows: 1,
            cols: 2,
            values: vec![-408.733, 421.99],
            x_bounds: [0.0, 10.0],
            y_bounds: [0.0, 10.0],
            colormap: plotium_figure::ColormapId::Viridis,
            value_range: [-408.733, 421.99],
        });
        let before = ObjectFrame::new(0.0, 0.0, 160.0, 160.0);
        let [width, height] = plot_min_size(&figure, before);
        assert!((76.0..160.0).contains(&width));
        assert!((24.0..160.0).contains(&height));
        assert_eq!(
            plot_min_size(&figure, ObjectFrame::new(0.0, 0.0, 30.0, 30.0)),
            [30.0, 30.0]
        );
    }
}
