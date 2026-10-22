//! Where a text annotation sits relative to its data point.

/// Placement of an annotation's text around its point. `Point` keeps the
/// original placement (text centred, baseline on the point); the others set
/// the text a small gap away on one side, as value labels on bars need.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAnchor {
    #[default]
    Point,
    Above,
    Below,
    Left,
    Right,
}

impl TextAnchor {
    /// Gap between a value label and the mark it labels, in points.
    pub const GAP_PT: f32 = 3.0;

    pub fn is_point(&self) -> bool {
        *self == Self::Point
    }
}
