use crate::{Color, SeriesOrigin};

/// A filled polygon in data-space coordinates: bars, box bodies, violin
/// strips, stacked segments, and pie wedges all lower to this one primitive.
///
/// Invariant: renderers assume the points describe a **convex** polygon (their
/// fill paths are simple fans). Builders must decompose concave shapes — e.g.
/// a violin outline — into convex pieces (strips, wedges) before emitting them.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Polygon {
    /// Legend entry; polygons sharing a name legend once. Empty = no legend.
    pub name: String,
    pub points: Vec<[f64; 2]>,
    pub fill: Color,
    /// Fill opacity in `[0, 1]`. Exporters without native alpha (EMF)
    /// pre-blend against the figure background.
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<(Color, f32)>,
    /// Native Series a bar, box or violin mark was lowered from; runtime only.
    #[serde(skip)]
    pub origin: Option<SeriesOrigin>,
}

fn default_opacity() -> f32 {
    1.0
}

impl Polygon {
    pub fn new(name: impl Into<String>, points: Vec<[f64; 2]>, fill: Color) -> Self {
        Self {
            name: name.into(),
            points,
            fill,
            opacity: 1.0,
            stroke: None,
            origin: None,
        }
    }

    /// An axis-aligned rectangle spanning `[x0, x1] × [y0, y1]` — the bar/box
    /// workhorse. Callers may pass bounds in either order.
    pub fn rect(name: impl Into<String>, x0: f64, x1: f64, y0: f64, y1: f64, fill: Color) -> Self {
        Self::new(name, vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]], fill)
    }

    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    pub fn with_stroke(mut self, color: Color, width: f32) -> Self {
        self.stroke = Some((color, width));
        self
    }
}
