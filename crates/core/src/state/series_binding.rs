use super::{
    Dataset, DatasetId, FieldId, OVERLAY_PALETTE, PresentationProfile, RequestedChart, SeriesId,
    default_encoding, field_peak_magnitude,
};
use plotium_figure::Color;
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;

/// What a curve's error bars show, as the user states it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorBarKind {
    Sd,
    Sem,
    Ci95,
}

impl ErrorBarKind {
    const ALL: [Self; 3] = [Self::Sd, Self::Sem, Self::Ci95];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Sd => "sd",
            Self::Sem => "sem",
            Self::Ci95 => "ci95",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.id() == id)
    }
}

/// One overlaid series' field source. A field is a child resource of its
/// dataset, not a component of the plot object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeriesSource {
    pub resource: DatasetId,
    pub field: FieldId,
    pub selection: SeriesSelection,
}

/// One overlaid series and its concrete visual encoding. Encoding-specific
/// values (colour, scaling, contour levels) live below `encoding`, so a contour
/// cannot accidentally inherit line-only semantics.
#[derive(Clone, Debug, PartialEq)]
pub struct SeriesBinding {
    pub id: SeriesId,
    pub source: SeriesSource,
    pub visible: bool,
    pub label: Option<String>,
    pub encoding: plotium_figure::SeriesEncoding,
    /// Replicates behind each plotted value when the table holds only
    /// summaries; raw replicate rows are counted instead.
    pub replicates: Option<NonZeroU32>,
    pub error_bars: Option<ErrorBarKind>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeriesSelection {
    FieldDefault,
    TraceItem(plotium_data::TraceItemId),
    TableColumns(super::TableCurveColumns),
}
impl SeriesSelection {
    pub fn from_item(item: Option<plotium_data::TraceItemId>) -> Self {
        item.map_or(Self::FieldDefault, Self::TraceItem)
    }
}
impl SeriesSource {
    pub fn item(&self) -> Option<plotium_data::TraceItemId> {
        match self.selection {
            SeriesSelection::TraceItem(item) => Some(item),
            _ => None,
        }
    }
    pub fn columns(&self) -> Option<super::TableCurveColumns> {
        match self.selection {
            SeriesSelection::TableColumns(columns) => Some(columns),
            _ => None,
        }
    }
}

impl SeriesBinding {
    /// Materialize the first canonical source for callers that add one series.
    pub fn from_dataset(dataset: &Dataset) -> Option<Self> {
        Self::from_dataset_all(dataset).into_iter().next()
    }

    /// Expand the dataset's default field into canonical item-addressed series.
    pub fn from_dataset_all(dataset: &Dataset) -> Vec<Self> {
        let Some(field) = dataset.default_field_id() else {
            return Vec::new();
        };
        Self::from_field_all(dataset, field)
    }

    pub(crate) fn from_field_all(dataset: &Dataset, field: FieldId) -> Vec<Self> {
        dataset.trace_collection(field).map_or_else(
            || {
                Self::from_field_item(dataset, field, None, 0)
                    .into_iter()
                    .collect()
            },
            |collection| {
                collection
                    .items
                    .iter()
                    .enumerate()
                    .filter_map(|(index, item)| {
                        Self::from_field_item(dataset, field, Some(item.id), index)
                    })
                    .collect()
            },
        )
    }

    pub(crate) fn from_field_item(
        dataset: &Dataset,
        field: FieldId,
        item: Option<plotium_data::TraceItemId>,
        palette_index: usize,
    ) -> Option<Self> {
        let descriptor = dataset.field_descriptor(field)?;
        let mut encoding = default_encoding(
            &descriptor.capabilities,
            &descriptor.metadata,
            RequestedChart::Auto,
            &PresentationProfile::default(),
            &|| field_peak_magnitude(dataset, field),
        );
        if let plotium_figure::SeriesEncoding::Line(line) = &mut encoding {
            line.color = plotium_figure::ColorSource::Explicit(
                OVERLAY_PALETTE[palette_index % OVERLAY_PALETTE.len()],
            );
        }
        Some(Self {
            id: SeriesId::default(),
            source: SeriesSource {
                resource: dataset.resource_id(),
                field,
                selection: SeriesSelection::from_item(item),
            },
            visible: true,
            label: None,
            encoding,
            replicates: None,
            error_bars: None,
        })
    }

    pub fn with_source(source: SeriesSource) -> Self {
        Self {
            id: SeriesId::default(),
            source,
            visible: true,
            label: None,
            encoding: plotium_figure::SeriesEncoding::default(),
            replicates: None,
            error_bars: None,
        }
    }

    pub fn line_scale(&self) -> f64 {
        match &self.encoding {
            plotium_figure::SeriesEncoding::Line(line) => line.scale,
            plotium_figure::SeriesEncoding::Contour(_)
            | plotium_figure::SeriesEncoding::Heatmap(_)
            | plotium_figure::SeriesEncoding::Image(_) => 1.0,
        }
    }

    /// The vertical axis this series is measured against.
    pub fn y_axis(&self) -> plotium_figure::YAxisSide {
        match &self.encoding {
            plotium_figure::SeriesEncoding::Line(line) => line.appearance.y_axis,
            _ => plotium_figure::YAxisSide::Left,
        }
    }

    pub fn line_x_shift(&self) -> Option<f64> {
        match &self.encoding {
            plotium_figure::SeriesEncoding::Line(line) => Some(line.x_shift.get()),
            _ => None,
        }
    }

    pub fn set_line_x_shift(&mut self, value: f64) -> bool {
        let Some(value) = plotium_figure::FiniteF64::new(value) else {
            return false;
        };
        let plotium_figure::SeriesEncoding::Line(line) = &mut self.encoding else {
            return false;
        };
        line.x_shift = value;
        true
    }

    pub fn primary_color(&self) -> Option<Color> {
        match &self.encoding {
            plotium_figure::SeriesEncoding::Line(line) => Some(line.color.resolve()),
            plotium_figure::SeriesEncoding::Contour(contour) => {
                Some(contour.style.positive_color.resolve())
            }
            plotium_figure::SeriesEncoding::Heatmap(_)
            | plotium_figure::SeriesEncoding::Image(_) => None,
        }
    }

    pub fn set_primary_color(&mut self, color: Color) {
        match &mut self.encoding {
            plotium_figure::SeriesEncoding::Line(line) => {
                line.color = plotium_figure::ColorSource::Explicit(color);
            }
            plotium_figure::SeriesEncoding::Contour(contour) => {
                contour.style.positive_color = plotium_figure::ColorSource::Explicit(color);
            }
            plotium_figure::SeriesEncoding::Heatmap(_)
            | plotium_figure::SeriesEncoding::Image(_) => {}
        }
    }
}
