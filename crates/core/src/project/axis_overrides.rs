use super::dto::RangeDto;
use crate::state::{
    AnnotationId, AxisOverrides, PlotAnnotation, PlotAnnotations, PlotArrow, PlotReferenceLine,
};
use plotium_figure::{
    AxisFrame, GuideLayout, GuidePlacement, GuideVisibility, LineStyle, ReferenceOrientation,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct AxisOverridesDto {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    x_display_unit: Option<Box<plotium_data::UnitSpec>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    y_display_unit: Option<Box<plotium_data::UnitSpec>>,
    #[serde(default)]
    typography: plotium_figure::FigureTypographyOverrides,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    x_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    y_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    x_range: Option<RangeDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    y_range: Option<RangeDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    lock_aspect: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    axis_frame: Option<AxisFrame>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    x_show_tick_labels: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    x_show_label: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    y_show_tick_labels: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    y_show_label: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    x_thousands_separator: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    y_thousands_separator: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    guide_visibility: Option<GuideVisibility>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    guide_placement: Option<GuidePlacement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    guide_layout: Option<GuideLayout>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    guide_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    legend_position: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    annotations: Option<PlotAnnotationsDto>,
}

#[derive(Clone, Serialize, Deserialize)]
struct PlotAnnotationsDto {
    next_id: AnnotationId,
    items: Vec<PlotAnnotationDto>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    reference_lines: Vec<PlotReferenceLineDto>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    arrows: Vec<PlotArrowDto>,
}

#[derive(Clone, Serialize, Deserialize)]
struct PlotArrowDto {
    id: AnnotationId,
    /// Tail and head `[x, y]` in the plot's axis units.
    at: [f64; 2],
    to: [f64; 2],
    head: plotium_figure::ArrowHead,
    line_width: f32,
    color: [u8; 3],
}

#[derive(Clone, Serialize, Deserialize)]
struct PlotReferenceLineDto {
    id: AnnotationId,
    orientation: ReferenceOrientation,
    /// Constant data coordinate in the plot's axis units.
    value: f64,
    color: [u8; 3],
    width: f32,
    line_style: LineStyle,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    font_size: Option<f32>,
}

impl PlotReferenceLineDto {
    fn from_line(line: &PlotReferenceLine) -> Self {
        Self {
            id: line.id,
            orientation: line.orientation,
            value: line.value,
            color: [line.color.r, line.color.g, line.color.b],
            width: line.width,
            line_style: line.line_style,
            label: line.label.clone(),
            font_size: line.font_size,
        }
    }

    fn to_line(&self) -> PlotReferenceLine {
        PlotReferenceLine {
            id: self.id,
            orientation: self.orientation,
            value: self.value,
            color: plotium_figure::Color::rgb(self.color[0], self.color[1], self.color[2]),
            width: self.width,
            line_style: self.line_style,
            label: self.label.clone(),
            font_size: self.font_size,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct PlotAnnotationDto {
    id: AnnotationId,
    /// Data-coordinate anchor `[x, y]` in the plot's axis units.
    at: [f64; 2],
    text: String,
    font_size: f32,
    color: [u8; 3],
}

impl PlotAnnotationsDto {
    fn from_annotations(annotations: &PlotAnnotations) -> Option<Self> {
        (annotations != &PlotAnnotations::default()).then(|| Self {
            next_id: annotations.next_id,
            items: annotations
                .items
                .iter()
                .map(|a| PlotAnnotationDto {
                    id: a.id,
                    at: a.at,
                    text: a.text.clone(),
                    font_size: a.font_size,
                    color: [a.color.r, a.color.g, a.color.b],
                })
                .collect(),
            reference_lines: annotations
                .reference_lines
                .iter()
                .map(PlotReferenceLineDto::from_line)
                .collect(),
            arrows: annotations
                .arrows
                .iter()
                .map(|a| PlotArrowDto {
                    id: a.id,
                    at: a.at,
                    to: a.to,
                    head: a.head,
                    line_width: a.line_width,
                    color: [a.color.r, a.color.g, a.color.b],
                })
                .collect(),
        })
    }

    fn to_annotations(&self) -> PlotAnnotations {
        PlotAnnotations {
            next_id: self.next_id,
            items: self
                .items
                .iter()
                .map(|a| PlotAnnotation {
                    id: a.id,
                    at: a.at,
                    text: a.text.clone(),
                    font_size: a.font_size,
                    color: plotium_figure::Color::rgb(a.color[0], a.color[1], a.color[2]),
                })
                .collect(),
            reference_lines: self
                .reference_lines
                .iter()
                .map(PlotReferenceLineDto::to_line)
                .collect(),
            arrows: self
                .arrows
                .iter()
                .map(|a| PlotArrow {
                    id: a.id,
                    at: a.at,
                    to: a.to,
                    head: a.head,
                    line_width: a.line_width,
                    color: plotium_figure::Color::rgb(a.color[0], a.color[1], a.color[2]),
                })
                .collect(),
        }
    }
}

impl AxisOverridesDto {
    pub(super) fn from_overrides(overrides: &AxisOverrides) -> Option<Self> {
        (overrides != &AxisOverrides::default()).then(|| Self {
            x_display_unit: overrides.x_display_unit.clone(),
            y_display_unit: overrides.y_display_unit.clone(),
            typography: overrides.typography,
            x_label: overrides.x_label.clone(),
            y_label: overrides.y_label.clone(),
            x_range: overrides.x_range.map(RangeDto::from_range),
            y_range: overrides.y_range.map(RangeDto::from_range),
            lock_aspect: overrides.lock_aspect,
            axis_frame: overrides.axis_frame,
            x_show_tick_labels: overrides.x_show_tick_labels,
            x_show_label: overrides.x_show_label,
            y_show_tick_labels: overrides.y_show_tick_labels,
            y_show_label: overrides.y_show_label,
            x_thousands_separator: overrides.x_thousands_separator,
            y_thousands_separator: overrides.y_thousands_separator,
            guide_visibility: overrides.guide_visibility,
            guide_placement: overrides.guide_placement,
            guide_layout: overrides.guide_layout,
            guide_title: overrides.guide_title.clone(),
            legend_position: overrides.legend_position,
            annotations: PlotAnnotationsDto::from_annotations(&overrides.annotations),
        })
    }

    pub(super) fn to_overrides(&self) -> AxisOverrides {
        AxisOverrides {
            x_display_unit: self.x_display_unit.clone(),
            y_display_unit: self.y_display_unit.clone(),
            typography: self.typography,
            x_label: self.x_label.clone(),
            y_label: self.y_label.clone(),
            x_range: self.x_range.map(RangeDto::into_range),
            y_range: self.y_range.map(RangeDto::into_range),
            lock_aspect: self.lock_aspect,
            axis_frame: self.axis_frame,
            x_show_tick_labels: self.x_show_tick_labels,
            x_show_label: self.x_show_label,
            y_show_tick_labels: self.y_show_tick_labels,
            y_show_label: self.y_show_label,
            x_thousands_separator: self.x_thousands_separator,
            y_thousands_separator: self.y_thousands_separator,
            guide_visibility: self.guide_visibility,
            guide_placement: self.guide_placement,
            guide_layout: self.guide_layout,
            guide_title: self.guide_title.clone(),
            legend_position: self.legend_position,
            annotations: self
                .annotations
                .as_ref()
                .map(PlotAnnotationsDto::to_annotations)
                .unwrap_or_default(),
        }
        .normalized()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_label_override_is_not_elided_when_text_matches_the_derived_label() {
        let derived_label = "Chemical shift";
        let overrides = AxisOverrides {
            x_label: Some(derived_label.to_owned()),
            ..AxisOverrides::default()
        };

        let dto = AxisOverridesDto::from_overrides(&overrides)
            .expect("an explicit label makes the override structure non-default");
        assert_eq!(dto.x_label.as_deref(), Some(derived_label));
    }

    #[test]
    fn frame_override_round_trips_through_the_project_format() {
        let overrides = AxisOverrides {
            axis_frame: Some(AxisFrame::Box),
            ..AxisOverrides::default()
        };
        let dto = AxisOverridesDto::from_overrides(&overrides).expect("frame override is stored");
        let json = serde_json::to_string(&dto).expect("serializes");
        let restored: AxisOverridesDto = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(restored.to_overrides(), overrides);
    }

    #[test]
    fn thousands_separator_override_round_trips_through_the_project_format() {
        let overrides = AxisOverrides {
            y_thousands_separator: Some(true),
            ..AxisOverrides::default()
        };
        let dto = AxisOverridesDto::from_overrides(&overrides).expect("separator is stored");
        let json = serde_json::to_string(&dto).expect("serializes");
        assert!(json.contains("\"y_thousands_separator\":true"));
        assert!(!json.contains("x_thousands_separator"));
        let restored: AxisOverridesDto = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(restored.to_overrides(), overrides);
    }
}
