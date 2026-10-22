//! Logarithmic plot axes: which figures may use them, their automatic
//! windows, and viewport navigation measured in the axis scale.

use super::{AxisOverrides, AxisRange};
use plotium_figure::{AxisFrame, AxisScale, Figure, SeriesKind};

/// Window used when a log axis has no positive data to fit.
const EMPTY_LOG_RANGE: AxisRange = AxisRange {
    min: 1.0,
    max: 10.0,
};
/// Padding at each end of an automatic log window, as a fraction of its
/// extent in decades, matching the linear automatic padding.
const LOG_PADDING: f64 = 0.05;
/// Padding in decades around a single positive value.
const SINGLE_VALUE_DECADES: f64 = 0.5;

/// Why a figure axis cannot switch to a log scale, or `None` when it can.
/// Only 1D curve plots (line, scatter, stick) qualify: 2D maps, bars and
/// distributions encode area or colour that a log axis would misrepresent.
pub fn log_scale_unavailable(figure: &Figure, x: bool) -> Option<&'static str> {
    let axis = if x { &figure.x } else { &figure.y };
    if figure.axis_frame == AxisFrame::Hidden {
        Some("Choose a chart with visible axes to edit axis settings.")
    } else if axis.categories.is_some() {
        Some("A categorical axis has no logarithmic scale.")
    } else if figure.heatmap.is_some()
        || !figure.contours.is_empty()
        || !figure.polygons.is_empty()
        || figure.top_projection.is_some()
        || figure.left_projection.is_some()
        || figure.lock_aspect
        || figure.series.is_empty()
        || !figure.series.iter().all(|series| {
            matches!(
                series.kind,
                SeriesKind::Line | SeriesKind::Points | SeriesKind::Stick
            )
        })
    {
        Some("Logarithmic axes are available on line, scatter and stick plots only.")
    } else {
        None
    }
}

/// Automatic log window over the positive finite `values`, padded in decades.
pub fn log_auto_range(values: impl IntoIterator<Item = f64>) -> AxisRange {
    let (lo, hi) = values
        .into_iter()
        .filter(|value| value.is_finite() && *value > 0.0)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), value| {
            (lo.min(value), hi.max(value))
        });
    if !lo.is_finite() {
        return EMPTY_LOG_RANGE;
    }
    let (lo, hi) = (lo.log10(), hi.log10());
    let pad = if hi > lo {
        (hi - lo) * LOG_PADDING
    } else {
        SINGLE_VALUE_DECADES
    };
    let range = AxisRange::new(10f64.powf(lo - pad), 10f64.powf(hi + pad));
    if range.is_valid() && range.min > 0.0 {
        range
    } else {
        EMPTY_LOG_RANGE
    }
}

/// Positions a figure draws on one axis: curve samples and, for y, both
/// uncertainty ends so automatic windows never clip a whisker. `within`
/// limits y values to samples inside an x window.
pub(crate) fn axis_values(
    figure: &Figure,
    x: bool,
    within: Option<AxisRange>,
) -> impl Iterator<Item = f64> + '_ {
    let inside = move |px: f64| within.is_none_or(|range| range.contains(px));
    let points = figure
        .series
        .iter()
        .flat_map(|series| &series.points)
        .filter(move |point| inside(point[0]))
        .map(move |point| if x { point[0] } else { point[1] });
    let bars = figure
        .error_bars
        .iter()
        .filter(move |bar| inside(bar.center[0]))
        .flat_map(move |bar| {
            if x {
                [bar.center[0], bar.center[0]]
            } else {
                [bar.center[1] - bar.negative, bar.center[1] + bar.positive]
            }
        });
    points.chain(bars)
}

impl AxisOverrides {
    /// Apply author scales after ranges. An automatic window on a log axis is
    /// refitted to the positive data; a manual one is kept as written.
    pub(super) fn apply_scales(&self, figure: &mut Figure) {
        for x in [true, false] {
            let (scale, manual) = if x {
                (self.x_scale, self.x_range)
            } else {
                (self.y_scale, self.y_range)
            };
            let Some(scale) = scale else { continue };
            if log_scale_unavailable(figure, x).is_some() {
                continue;
            }
            let automatic = (scale == AxisScale::Log10 && manual.is_none())
                .then(|| log_auto_range(axis_values(figure, x, None)));
            let axis = if x { &mut figure.x } else { &mut figure.y };
            axis.scale = scale;
            if let Some(range) = automatic {
                axis.min = range.min;
                axis.max = range.max;
            }
        }
    }
}

impl AxisRange {
    fn in_scale(self, scale: AxisScale) -> Option<Self> {
        let range = Self {
            min: scale.forward(self.min),
            max: scale.forward(self.max),
        };
        range.is_valid().then_some(range)
    }

    fn out_of_scale(self, scale: AxisScale) -> Self {
        Self::new(scale.inverse(self.min), scale.inverse(self.max))
    }

    /// [`AxisRange::zoom_around`] measured in the axis scale, so a log axis
    /// zooms by the same factor of decades everywhere.
    pub fn zoom_around_in(self, full: Self, anchor: f64, factor: f64, scale: AxisScale) -> Self {
        match (self.in_scale(scale), full.in_scale(scale)) {
            (Some(view), Some(full_scaled)) if !scale.is_linear() => {
                let zoomed = view.zoom_around(full_scaled, scale.forward(anchor), factor);
                // Returning the stored window keeps "showing everything"
                // comparable after the round trip through log10.
                if zoomed == full_scaled {
                    full
                } else {
                    zoomed.out_of_scale(scale)
                }
            }
            _ => self.zoom_around(full, anchor, factor),
        }
    }

    /// Shift by `delta`, measured in the axis scale (decades on a log
    /// axis), kept inside `full`.
    pub fn shifted_in(self, full: Self, delta: f64, scale: AxisScale) -> Self {
        match self.in_scale(scale) {
            Some(view) if !scale.is_linear() => Self::new(view.min + delta, view.max + delta)
                .out_of_scale(scale)
                .clamp_in(full, scale),
            _ => Self::new(self.min + delta, self.max + delta).clamp_to(full),
        }
    }

    /// [`AxisRange::clamp_to`] measured in the axis scale, so a selection
    /// keeps its extent in decades rather than in linear units.
    pub fn clamp_in(self, full: Self, scale: AxisScale) -> Self {
        match (self.in_scale(scale), full.in_scale(scale)) {
            (Some(view), Some(full_scaled)) if !scale.is_linear() => {
                let clamped = view.clamp_to(full_scaled);
                if clamped == full_scaled {
                    full
                } else {
                    clamped.out_of_scale(scale)
                }
            }
            _ => self.clamp_to(full),
        }
    }
}

#[cfg(test)]
#[path = "log_axes_tests.rs"]
mod tests;
