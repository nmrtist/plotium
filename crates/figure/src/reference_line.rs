//! Data-valued horizontal or vertical guide lines spanning the plot area.
use crate::{Color, LineStyle};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceOrientation {
    /// Constant x: the line runs from the bottom to the top of the plot.
    Vertical,
    /// Constant y: the line runs from the left to the right of the plot.
    Horizontal,
}

/// A line at one data value that spans the whole plot rectangle. Renderers
/// project only the constant coordinate, so the line follows axis ranges,
/// zoom and pan, and is clipped with the plot when the value is off-axis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReferenceLine {
    pub orientation: ReferenceOrientation,
    pub value: f64,
    pub color: Color,
    /// Stroke width in points.
    pub width: f32,
    pub line_style: LineStyle,
    /// Empty draws no label.
    pub label: String,
    /// Label size in points; `None` follows the figure's tick text size.
    pub label_size: Option<f32>,
    /// Pinned label position; `None` lets the renderer place it clear of data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_position: Option<ReferenceLabelPosition>,
}

/// Which side of its line a reference label sits on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceLabelSide {
    /// Left of a vertical line, above a horizontal one.
    Before,
    /// Right of a vertical line, below a horizontal one.
    After,
}

/// A reference label pinned along its line. `along` is the label centre as a
/// fraction of the plot, from the bottom for a vertical line and from the left
/// for a horizontal one, so the label follows the line through range changes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReferenceLabelPosition {
    pub along: f32,
    pub side: ReferenceLabelSide,
}
