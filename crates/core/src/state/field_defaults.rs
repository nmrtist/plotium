use super::{
    CONTOUR_BASE_ABSOLUTE, CONTOUR_BASE_BACKGROUND_SCALE, CONTOUR_BASE_FRACTION_OF_RANGE,
    CONTOUR_BASE_NOISE_FLOOR, FieldCapabilities, FieldMetadata, PeakMagnitude, PresentationProfile,
    RequestedChart, contour_base_policy,
};
use crate::automation::{
    CAP_FIELD_BOUNDED, CAP_FIELD_COLORED_RASTER_2D, CAP_FIELD_CURVE_1D, CAP_FIELD_LOCATION_SCALE,
    CAP_FIELD_NOISE_SCALE, CAP_FIELD_SCALAR_GRID_2D_REGULAR, CAP_FIELD_SIGNED,
};
use plotium_figure::{
    ColorSource, ContourLevelSpec, ContourSpec, ContourStyle, HeatmapSpec, ImageSpec, LineEncoding,
    PositiveFiniteF64, SeriesEncoding,
};

/// Materialize the complete persisted encoding for a newly created series.
/// This is the sole default-policy factory; it never dispatches on `DataDomain`.
pub fn default_encoding(
    source_capabilities: &FieldCapabilities,
    semantic_metadata: &FieldMetadata,
    requested_chart: RequestedChart,
    presentation_profile: &PresentationProfile,
    peak: PeakMagnitude<'_>,
) -> SeriesEncoding {
    let requested_chart = match requested_chart {
        RequestedChart::Auto => presentation_profile
            .preferred_encoding
            .or_else(|| match semantic_metadata.recommended_encoding() {
                Some("line") => Some(RequestedChart::Line),
                Some("contour") => Some(RequestedChart::Contour),
                Some("heatmap") => Some(RequestedChart::Heatmap),
                Some("image") => Some(RequestedChart::Image),
                _ => None,
            })
            .unwrap_or_else(|| {
                if source_capabilities.supports(&[CAP_FIELD_COLORED_RASTER_2D]) {
                    RequestedChart::Image
                } else if source_capabilities.supports(&[CAP_FIELD_SCALAR_GRID_2D_REGULAR]) {
                    RequestedChart::Heatmap
                } else {
                    RequestedChart::Line
                }
            }),
        concrete => concrete,
    };

    match requested_chart {
        RequestedChart::Contour
            if source_capabilities.supports(&[CAP_FIELD_SCALAR_GRID_2D_REGULAR]) =>
        {
            SeriesEncoding::Contour(default_contour_spec(source_capabilities, peak))
        }
        RequestedChart::Heatmap
            if source_capabilities.supports(&[CAP_FIELD_SCALAR_GRID_2D_REGULAR]) =>
        {
            SeriesEncoding::Heatmap(HeatmapSpec::default())
        }
        RequestedChart::Image if source_capabilities.supports(&[CAP_FIELD_COLORED_RASTER_2D]) => {
            SeriesEncoding::Image(ImageSpec::default())
        }
        RequestedChart::Line if source_capabilities.contains(CAP_FIELD_CURVE_1D) => {
            SeriesEncoding::Line(LineEncoding::default())
        }
        RequestedChart::Auto
        | RequestedChart::Line
        | RequestedChart::Contour
        | RequestedChart::Heatmap
        | RequestedChart::Image
            if source_capabilities.supports(&[CAP_FIELD_COLORED_RASTER_2D]) =>
        {
            SeriesEncoding::Image(ImageSpec::default())
        }
        RequestedChart::Auto
        | RequestedChart::Line
        | RequestedChart::Contour
        | RequestedChart::Heatmap
        | RequestedChart::Image
            if source_capabilities.supports(&[CAP_FIELD_SCALAR_GRID_2D_REGULAR]) =>
        {
            SeriesEncoding::Heatmap(HeatmapSpec::default())
        }
        RequestedChart::Auto
        | RequestedChart::Line
        | RequestedChart::Contour
        | RequestedChart::Heatmap
        | RequestedChart::Image => SeriesEncoding::Line(LineEncoding::default()),
    }
}

pub fn default_contour_base_kind(capabilities: &FieldCapabilities) -> &'static str {
    if capabilities.contains(CAP_FIELD_NOISE_SCALE) {
        CONTOUR_BASE_NOISE_FLOOR
    } else if capabilities.contains(CAP_FIELD_LOCATION_SCALE) {
        CONTOUR_BASE_BACKGROUND_SCALE
    } else if capabilities.contains(CAP_FIELD_BOUNDED) {
        CONTOUR_BASE_FRACTION_OF_RANGE
    } else {
        CONTOUR_BASE_ABSOLUTE
    }
}

pub fn default_contour_spec(
    capabilities: &FieldCapabilities,
    peak: PeakMagnitude<'_>,
) -> ContourSpec {
    let base = contour_base_policy(default_contour_base_kind(capabilities), peak)
        .expect("default base kind is a known policy");
    let level = ContourLevelSpec {
        base,
        count: 14,
        ratio: PositiveFiniteF64::new(1.35).expect("literal ratio is valid"),
    };
    ContourSpec {
        positive: level.clone(),
        negative: capabilities.contains(CAP_FIELD_SIGNED).then_some(level),
        style: ContourStyle {
            positive_color: ColorSource::Explicit(plotium_figure::Color::TRACE),
            negative_color: ColorSource::Explicit(plotium_figure::Color::rgb(0xd1, 0x24, 0x2a)),
            ..ContourStyle::default()
        },
    }
}

/// Stored policies must remain applicable after reprocessing changes a field's
/// representation; do not silently interpret modulus amplitudes as Gaussian.
pub(crate) fn contour_estimator_error(
    spec: &ContourSpec,
    caps: &FieldCapabilities,
) -> Option<&'static str> {
    std::iter::once(&spec.positive).chain(spec.negative.iter()).find_map(|level| match level.base {
        plotium_figure::ContourBasePolicy::NoiseFloor { .. } if !caps.contains(CAP_FIELD_NOISE_SCALE) => Some("This field does not support phase-sensitive noise estimation. Choose an absolute contour level or reset contours."),
        plotium_figure::ContourBasePolicy::BackgroundScale { .. } if !caps.contains(CAP_FIELD_LOCATION_SCALE) => Some("This field does not support background estimation. Choose an absolute contour level or reset contours."),
        _ => None,
    })
}
