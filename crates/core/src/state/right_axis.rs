//! The right y axis: when a figure has one, its automatic window, the author
//! overrides that apply to it and how navigation carries it along.

use super::{AxisOverrides, AxisRange, CanvasViewport, visible_y_range_on};
use plotium_figure::{Axis, AxisFrame, Figure, SeriesKind, YAxisSide};

pub(crate) const NO_RIGHT_AXIS: &str = "Put a line or scatter curve on the right y axis first.";
pub(crate) const RIGHT_AXIS_KINDS: &str =
    "Only line and scatter curves can use the right y axis; bars and sticks stay on the left.";

/// Whether curves of `kind` may be measured on the right axis. Grouped bars
/// share slot offsets with every bar of the plot, so they stay on the left.
pub fn right_axis_supports(kind: SeriesKind) -> bool {
    matches!(kind, SeriesKind::Line | SeriesKind::Points)
}

/// Give a freshly built figure its right axis, or remove it. The right axis
/// exists exactly while some series is measured on it, and each y axis is
/// then fitted to its own data over the full x window. A figure without
/// right-axis series keeps its builder's y window untouched.
pub(crate) fn split_y_axes(figure: &mut Figure) {
    if !figure.uses_right_axis() || figure.axis_frame == AxisFrame::Hidden {
        figure.y2 = None;
        return;
    }
    let x = AxisRange::from_axis(&figure.x);
    let mut y2 = figure.y2.take().unwrap_or_else(|| Axis::new("", 0.0, 1.0));
    // Builders emit a linear window; author scales are applied afterwards.
    y2.scale = plotium_figure::AxisScale::Linear;
    figure.y2 = Some(y2);
    let right = visible_y_range_on(figure, x, YAxisSide::Right).unwrap_or(AxisRange::new(0.0, 1.0));
    if let Some(axis) = figure.y2.as_mut() {
        axis.min = right.min;
        axis.max = right.max;
    }
    if let Some(left) = visible_y_range_on(figure, x, YAxisSide::Left) {
        figure.y.min = left.min;
        figure.y.max = left.max;
    }
}

impl AxisOverrides {
    /// Right-axis label, range and visibility overrides. Scales are applied
    /// with the left ones in [`AxisOverrides::apply_scales`].
    pub(super) fn apply_right_axis(&self, figure: &mut Figure) {
        let Some(axis) = figure.y2.as_mut() else {
            return;
        };
        if let Some(label) = &self.y2_label {
            axis.label.clone_from(label);
        }
        if let Some(range) = self.y2_range {
            axis.min = range.min;
            axis.max = range.max;
        }
        if let Some(show) = self.y2_show_tick_labels {
            axis.show_tick_labels = show;
        }
        if let Some(show) = self.y2_show_label {
            axis.show_label = show;
        }
    }

    /// Drop the range and display unit of the y axis on `side`, which were
    /// written in a coordinate unit that its first curve has just replaced.
    pub(crate) fn forget_axis_units(&mut self, side: YAxisSide) {
        if side.is_right() {
            self.y2_range = None;
            self.y2_display_unit = None;
        } else {
            self.y_range = None;
            self.y_display_unit = None;
        }
    }

    /// Whether any right-axis override differs between two override sets.
    pub(crate) fn right_axis_changed(&self, other: &Self) -> bool {
        self.y2_display_unit != other.y2_display_unit
            || self.y2_label != other.y2_label
            || self.y2_range != other.y2_range
            || self.y2_scale != other.y2_scale
            || self.y2_show_tick_labels != other.y2_show_tick_labels
            || self.y2_show_label != other.y2_show_label
    }
}

impl CanvasViewport {
    /// Adopt the right-axis window of a rebuilt figure, keeping a zoomed view
    /// inside it. Automatic y refits it later from the visible data.
    pub(super) fn sync_full_y2(&mut self, fig: &Figure) {
        let Some(axis) = fig.y2.as_ref() else {
            self.full_y2 = None;
            self.view_y2 = None;
            return;
        };
        let full = AxisRange::from_axis(axis);
        self.full_y2 = Some(full);
        self.view_y2 = Some(match self.view_y2 {
            Some(view) if !self.auto_y => view.clamp_in(full, axis.scale),
            _ => full,
        });
    }

    /// Move the right-axis window by the same fractions, in each axis scale,
    /// that took the left window from `before` to its current extent.
    pub(super) fn follow_y2(&mut self, fig: &Figure, before: AxisRange) {
        let (Some(view), Some(full), Some(axis)) = (self.view_y2, self.full_y2, fig.y2.as_ref())
        else {
            return;
        };
        let left = fig.y.scale;
        let (b0, b1) = (left.forward(before.min), left.forward(before.max));
        let fraction = |value: f64| (left.forward(value) - b0) / (b1 - b0);
        let (f0, f1) = (fraction(self.view_y.min), fraction(self.view_y.max));
        let right = axis.scale;
        let (r0, r1) = (right.forward(view.min), right.forward(view.max));
        let moved = AxisRange::new(
            right.inverse(r0 + f0 * (r1 - r0)),
            right.inverse(r0 + f1 * (r1 - r0)),
        );
        if moved.is_valid() {
            self.view_y2 = Some(moved.clamp_in(full, right));
        }
    }
}

#[cfg(test)]
#[path = "right_axis_tests.rs"]
mod tests;
