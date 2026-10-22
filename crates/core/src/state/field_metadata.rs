use super::{
    CAP_FIELD_COLORED_RASTER_2D, CAP_FIELD_SCALAR_GRID_2D_REGULAR, CapabilityId, FieldId,
    SummaryPart,
};
use std::collections::{BTreeMap, BTreeSet};

/// Stable child-resource metadata, including the capabilities used by encoding
/// and chart applicability checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldDescriptor {
    pub id: FieldId,
    pub local_id: String,
    pub name: String,
    /// The scientific concept represented by this field. This is required so
    /// every new field participates in the v1 summary contract by construction.
    pub scientific_observation: SummaryPart,
    pub capabilities: FieldCapabilities,
    pub dimensions: Vec<usize>,
    pub units: Vec<String>,
    pub metadata: FieldMetadata,
}

impl FieldDescriptor {
    /// Native geometry emitted for a curve field, independent of the persisted
    /// line encoding used to style it. Descriptors never materialize its data.
    pub fn curve_kind(&self) -> plotium_figure::SeriesKind {
        match self.metadata.0.get("curve_kind").map(String::as_str) {
            Some("points") => plotium_figure::SeriesKind::Points,
            Some("stick") => plotium_figure::SeriesKind::Stick,
            Some("bar") => plotium_figure::SeriesKind::Bar,
            _ => plotium_figure::SeriesKind::Line,
        }
    }

    pub(crate) fn with_curve_kind(mut self, kind: plotium_figure::SeriesKind) -> Self {
        self.metadata.0.insert(
            "curve_kind".into(),
            match kind {
                plotium_figure::SeriesKind::Line => "line",
                plotium_figure::SeriesKind::Points => "points",
                plotium_figure::SeriesKind::Stick => "stick",
                plotium_figure::SeriesKind::Bar => "bar",
            }
            .into(),
        );
        self
    }

    pub(crate) fn with_line_x_unit(mut self, unit: impl Into<String>) -> Self {
        self.metadata
            .0
            .insert(LINE_X_UNIT_METADATA_KEY.to_owned(), unit.into());
        self
    }

    pub fn line_x_unit(&self) -> Option<&str> {
        self.metadata.line_x_unit()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FieldCapabilities(BTreeSet<CapabilityId>);

impl FieldCapabilities {
    pub fn new(values: impl IntoIterator<Item = CapabilityId>) -> Self {
        Self(values.into_iter().collect())
    }

    pub fn contains(&self, capability: &str) -> bool {
        self.0.contains(capability)
    }

    /// Reject scalar-grid renderers for a colored raster even when a malformed
    /// provider advertises both mutually exclusive capabilities.
    pub fn supports(&self, required: &[&str]) -> bool {
        required.iter().all(|capability| self.contains(capability))
            && !(required.contains(&CAP_FIELD_SCALAR_GRID_2D_REGULAR)
                && self.contains(CAP_FIELD_COLORED_RASTER_2D))
    }

    pub fn iter(&self) -> impl Iterator<Item = &CapabilityId> {
        self.0.iter()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FieldMetadata(pub BTreeMap<String, String>);

pub(super) const LINE_X_UNIT_METADATA_KEY: &str = "line_x_unit";

impl FieldMetadata {
    pub fn recommended_encoding(&self) -> Option<&str> {
        self.0.get("recommended_encoding").map(String::as_str)
    }

    pub fn line_x_unit(&self) -> Option<&str> {
        self.0
            .get(LINE_X_UNIT_METADATA_KEY)
            .map(String::as_str)
            .filter(|unit| !unit.is_empty())
    }
}
