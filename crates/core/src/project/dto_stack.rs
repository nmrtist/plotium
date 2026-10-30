use super::*;

#[derive(Serialize, Deserialize, Clone)]
pub struct StackDto {
    pub mode: String,
    pub spacing_y: f64,
    pub shear_x: f64,
    pub normalize: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<usize>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub occlude: bool,
    /// Colormap id; absent = series colours. Unknown ids keep series colours.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gradient: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub sequence_scale: bool,
}

impl StackDto {
    pub fn from_spec(s: &StackSpec) -> Self {
        Self {
            mode: match s.mode {
                StackMode::Superimposed => "superimposed",
                StackMode::Offset => "offset",
                StackMode::ColorOverlay => "color_overlay",
            }
            .to_owned(),
            spacing_y: s.spacing_y,
            shear_x: s.shear_x,
            normalize: s.normalize,
            active: s.active,
            occlude: s.occlude,
            gradient: s.gradient.map(|map| map.id().to_owned()),
            sequence_scale: s.sequence_scale,
        }
    }

    pub fn into_spec(self) -> StackSpec {
        StackSpec {
            mode: match self.mode.as_str() {
                "offset" => StackMode::Offset,
                "color_overlay" => StackMode::ColorOverlay,
                _ => StackMode::Superimposed,
            },
            spacing_y: self.spacing_y,
            shear_x: self.shear_x,
            normalize: self.normalize,
            active: self.active,
            occlude: self.occlude,
            gradient: self
                .gradient
                .as_deref()
                .and_then(plotium_figure::ColormapId::from_id),
            sequence_scale: self.sequence_scale,
        }
    }
}
