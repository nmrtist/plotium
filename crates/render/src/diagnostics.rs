//! Output-space observations of the native renderer. No document identities or edits.
mod curves;
mod shapes;
pub use shapes::shape_bounds;
mod raster;
mod text;
use crate::Rect;
pub use curves::{CurveScan, CurveScanResult, scan_curve};
pub use raster::raster_bounds;
pub use text::{
    TextBox, annotation_text_boxes, figure_text_boxes, figure_text_boxes_placed,
    overlay_text_bounds,
};

/// An axis that draws fewer than two tick labels at this size although its
/// range has two or more ticks, so readers cannot read its scale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SparseTicks {
    pub component: &'static str,
    pub drawn: usize,
    pub available: usize,
}

/// Axes of `fig`, rendered at `size` with `placement`, whose drawn tick
/// labels are too few to read. Axes whose tick labels are hidden by choice or
/// by the stack are not reported.
pub fn sparse_tick_axes(
    fig: &plotium_figure::Figure,
    size: [f32; 2],
    placement: Option<&crate::StackPlacement>,
) -> Vec<SparseTicks> {
    if fig.axis_frame == plotium_figure::AxisFrame::Hidden {
        return Vec::new();
    }
    let layout = crate::axis_layout_placed(fig, size[0], size[1], placement);
    let outer = Rect::new(0.0, 0.0, size[0].max(0.0), size[1].max(0.0));
    let plot = crate::Projector::new(fig, outer, &layout.margins).plot;
    let furniture = crate::XFurniture::of(fig, placement);
    let drawn_y = |axis: &plotium_figure::Axis, ticks: &crate::AxisTicks| {
        ticks
            .values
            .iter()
            .filter(|&&value| {
                let from_top = (1.0 - axis.normalize(value) as f32) * plot.height;
                furniture.shows_y_label_at(from_top)
            })
            .count()
    };
    let mut axes = Vec::new();
    if furniture.show_tick_labels {
        axes.push((
            "axis.x",
            &fig.x,
            layout.x_ticks.values.len(),
            crate::ticks::MAX_X_TARGET,
        ));
    }
    if fig.y.show_tick_labels {
        let drawn = drawn_y(&fig.y, &layout.y_ticks);
        axes.push(("axis.y", &fig.y, drawn, crate::ticks::MAX_Y_TARGET));
    }
    if let Some(axis) = fig.y2.as_ref().filter(|axis| axis.show_tick_labels) {
        let drawn = drawn_y(axis, &layout.y2_ticks);
        axes.push(("axis.y2", axis, drawn, crate::ticks::MAX_Y_TARGET));
    }
    axes.into_iter()
        .filter_map(|(component, axis, drawn, target)| {
            let available = crate::axis_ticks_for(axis, target).values.len();
            (drawn < 2 && available >= 2).then_some(SparseTicks {
                component,
                drawn,
                available,
            })
        })
        .collect()
}

/// Whether the figure's right y axis draws a visible, non-blank title.
pub fn shows_right_axis_title(fig: &plotium_figure::Figure) -> bool {
    crate::y2_axis::shows_title(fig)
}
pub fn intersection(a: Rect, b: Rect) -> Option<Rect> {
    let l = a.left.max(b.left);
    let t = a.top.max(b.top);
    let r = a.right().min(b.right());
    let d = a.bottom().min(b.bottom());
    (r > l && d > t).then(|| Rect::new(l, t, r - l, d - t))
}
pub fn contains(outer: Rect, inner: Rect) -> bool {
    inner.left >= outer.left
        && inner.top >= outer.top
        && inner.right() <= outer.right()
        && inner.bottom() <= outer.bottom()
}
pub fn translate(r: Rect, x: f32, y: f32) -> Rect {
    Rect::new(r.left + x, r.top + y, r.width, r.height)
}
pub fn scale(r: Rect, s: f32) -> Rect {
    Rect::new(r.left * s, r.top * s, r.width * s, r.height * s)
}
pub fn expanded(r: Rect, n: f32) -> Rect {
    Rect::new(r.left - n, r.top - n, r.width + 2. * n, r.height + 2. * n)
}
pub fn union(a: Rect, b: Rect) -> Rect {
    let l = a.left.min(b.left);
    let t = a.top.min(b.top);
    Rect::new(
        l,
        t,
        a.right().max(b.right()) - l,
        a.bottom().max(b.bottom()) - t,
    )
}
