//! Vertical uncertainty whiskers.
use crate::Color;

/// A vertical uncertainty whisker in data-space coordinates. `center` is the
/// plotted observation and `negative`/`positive` are non-negative distances
/// below and above it. Cap width is expressed in output-space logical units so
/// it stays legible independently of the x-axis scale, unless the whisker
/// belongs to a mark with a data-space width (see `cap_data_width`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ErrorBar {
    #[serde(default)]
    pub display_baseline: f64,
    pub center: [f64; 2],
    pub negative: f64,
    pub positive: f64,
    pub color: Color,
    pub width: f32,
    pub cap_width: f32,
    /// Paint after the corresponding data series. Bars use this so their wide
    /// stroke cannot obscure the lower whisker; point markers leave it false so
    /// they remain visually on top of the whisker center.
    #[serde(default)]
    pub draw_over_data: bool,
    /// Cap width in x-axis data units, replacing `cap_width` so a bar's caps
    /// keep their proportion to the bar at any size or zoom.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cap_data_width: Option<f64>,
    /// The vertical axis of the curve this whisker belongs to.
    #[serde(default, skip_serializing_if = "crate::is_left")]
    pub y_axis: crate::YAxisSide,
    /// Spread along x instead of y (horizontal bars): `center[0]` is then the
    /// observation, `negative`/`positive` x distances and `cap_data_width` a
    /// y-axis length.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub horizontal: bool,
}

impl ErrorBar {
    pub fn symmetric(center: [f64; 2], error: f64) -> Self {
        Self {
            display_baseline: 0.0,
            center,
            negative: error,
            positive: error,
            color: Color::TRACE,
            width: 1.0,
            cap_width: 8.0,
            draw_over_data: false,
            cap_data_width: None,
            y_axis: crate::YAxisSide::Left,
            horizontal: false,
        }
    }

    pub fn colored(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn over_data(mut self) -> Self {
        self.draw_over_data = true;
        self
    }

    pub fn with_cap_data_width(mut self, width: f64) -> Self {
        self.cap_data_width = Some(width);
        self
    }
}
