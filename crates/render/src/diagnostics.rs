//! Output-space observations of the native renderer. No document identities or edits.
mod curves;
mod shapes;
pub use shapes::shape_bounds;
mod raster;
mod text;
use crate::Rect;
pub use curves::{CurveScan, CurveScanResult, scan_curve};
pub use raster::raster_bounds;
pub use text::{TextBox, figure_text_boxes, figure_text_boxes_placed, overlay_text_bounds};

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
