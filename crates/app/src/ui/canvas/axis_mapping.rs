//! Data ↔ screen mapping through a figure axis, honouring its scale. On a
//! linear axis these match the min/span helpers in `geometry` exactly; on a
//! log axis positions are linear in log10 of the value.

use plotium_figure::Axis;
use plotium_render::Rect as PlotRect;

pub(crate) fn to_screen_x(value: f64, plot: PlotRect, axis: &Axis) -> f32 {
    plot.left + axis.normalize(value) as f32 * plot.width
}

pub(crate) fn from_screen_x(px: f32, plot: PlotRect, axis: &Axis) -> f64 {
    axis.value_at(f64::from((px - plot.left) / plot.width))
}

pub(crate) fn to_screen_y(value: f64, plot: PlotRect, axis: &Axis) -> f32 {
    plot.top + ((1.0 - axis.normalize(value)) as f32) * plot.height
}

pub(crate) fn from_screen_y(py: f32, plot: PlotRect, axis: &Axis) -> f64 {
    axis.value_at(f64::from(1.0 - (py - plot.top) / plot.height))
}
