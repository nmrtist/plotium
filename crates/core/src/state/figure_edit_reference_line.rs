//! Reference-line kind of the plot annotation edits in a figure transaction.
use super::plot_label_placement::{guard_hand_position, request_placement};
use super::*;
use plotium_figure::{LineStyle, ReferenceOrientation};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub(super) const MISSING_ANNOTATION: &str = "Discover an exact annotation id on this plot.";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationKind {
    #[default]
    Text,
    ReferenceLine,
    Arrow,
}

/// Stroke of a reference line; omitted fields keep their current value.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceLineStyleEdit {
    /// `#rrggbb`.
    pub color: Option<String>,
    pub width_pt: Option<f32>,
    pub style: Option<LineStyle>,
}

struct LineFields<'a> {
    orientation: Option<ReferenceOrientation>,
    value: Option<f64>,
    label: Option<&'a str>,
    line: Option<&'a ReferenceLineStyleEdit>,
}

impl LineFields<'_> {
    fn is_empty(&self) -> bool {
        self.orientation.is_none()
            && self.value.is_none()
            && self.label.is_none()
            && self.line.is_none()
    }
}

fn line_fields(edit: &FigureEdit) -> LineFields<'_> {
    match edit {
        FigureEdit::AnnotationCreate {
            orientation,
            value,
            label,
            line,
            ..
        }
        | FigureEdit::AnnotationSet {
            orientation,
            value,
            label,
            line,
            ..
        } => LineFields {
            orientation: *orientation,
            value: *value,
            label: label.as_deref(),
            line: line.as_ref(),
        },
        _ => LineFields {
            orientation: None,
            value: None,
            label: None,
            line: None,
        },
    }
}

fn parse_color(text: &str) -> Result<plotium_figure::Color, String> {
    let invalid = || format!("'{text}' is not a '#rrggbb' colour.");
    let digits = text.strip_prefix('#').ok_or_else(invalid)?;
    if digits.len() != 6 || !digits.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(invalid());
    }
    let channel = |at: usize| u8::from_str_radix(&digits[at..at + 2], 16).map_err(|_| invalid());
    Ok(plotium_figure::Color::rgb(
        channel(0)?,
        channel(2)?,
        channel(4)?,
    ))
}

fn update(
    line: &mut PlotReferenceLine,
    fields: &LineFields,
    font_pt: Option<f32>,
    override_hand_edits: bool,
) -> Result<(), String> {
    let moves = fields.orientation.is_some_and(|o| o != line.orientation)
        || fields.value.is_some_and(|v| v != line.value);
    if moves {
        guard_hand_position(line.hand_moved, line.id, override_hand_edits)?;
        line.hand_moved = false;
    }
    if let Some(orientation) = fields.orientation {
        line.orientation = orientation;
    }
    if let Some(value) = fields.value {
        line.value = value;
    }
    if let Some(label) = fields.label {
        // An empty label removes it; whitespace alone would draw nothing.
        line.label = label.trim().to_owned();
    }
    if let Some(pt) = font_pt {
        line.font_size = Some(pt);
    }
    if let Some(style) = fields.line {
        if let Some(color) = &style.color {
            line.color = parse_color(color)?;
        }
        if let Some(width) = style.width_pt {
            line.width = width;
        }
        if let Some(dash) = style.style {
            line.line_style = dash;
        }
    }
    line.validate()
}

/// Apply `edit` when it creates or addresses a reference line, returning the
/// id it addressed. `None` leaves the edit to the text annotation kind, after
/// rejecting reference-line fields that the text kind would silently ignore.
pub(super) fn apply(
    annotations: &mut PlotAnnotations,
    edit: &FigureEdit,
    default_width: Option<f32>,
) -> Result<Option<AnnotationId>, String> {
    let fields = line_fields(edit);
    let not_text = "Reference lines take value and label, not at or text.";
    let not_color = "Reference lines take line.color, not color.";
    let not_line = "orientation, value, label and line apply only to reference lines.";
    match edit {
        FigureEdit::AnnotationCreate {
            kind: AnnotationKind::ReferenceLine,
            text,
            at,
            font_pt,
            color,
            ..
        } => {
            if text.is_some() || at.is_some() {
                return Err(not_text.into());
            }
            if color.is_some() {
                return Err(not_color.into());
            }
            let orientation = fields
                .orientation
                .ok_or("A reference line requires an orientation.")?;
            let value = fields.value.ok_or("A reference line requires a value.")?;
            let mut line = PlotReferenceLine::new(annotations.next_id, orientation, value);
            if let Some(width) = default_width {
                line.width = width;
            }
            update(&mut line, &fields, *font_pt, false)?;
            annotations.add_reference_line(line).map(Some)
        }
        FigureEdit::AnnotationCreate { .. } if !fields.is_empty() => Err(
            "Set kind \"reference_line\" to create a reference line; orientation, value, label and line apply only to reference lines."
                .into(),
        ),
        FigureEdit::AnnotationSet {
            annotation_id,
            text,
            at,
            font_pt,
            offset_pt,
            placement,
            override_hand_edits,
            color,
            ..
        } => {
            let Some(line) = annotations.reference_line_mut(*annotation_id) else {
                if fields.is_empty() {
                    return Ok(None);
                }
                return Err(if annotations.get(*annotation_id).is_some() {
                    not_line.into()
                } else {
                    MISSING_ANNOTATION.into()
                });
            };
            if text.is_some() || at.is_some() {
                return Err(not_text.into());
            }
            if color.is_some() {
                return Err(not_color.into());
            }
            if offset_pt.is_some() {
                return Err(
                    "offset_pt applies to text annotations; a reference label takes placement \"auto\"."
                        .into(),
                );
            }
            if fields.is_empty() && font_pt.is_none() && placement.is_none() {
                return Err(
                    "Reference line edit requires orientation, value, label, font_pt, line or placement."
                        .into(),
                );
            }
            if let Some(PlacementRequest::Auto) = placement {
                request_placement(
                    &mut line.label_placement,
                    LabelPlacement::Auto,
                    *annotation_id,
                    *override_hand_edits,
                )?;
            }
            update(line, &fields, *font_pt, *override_hand_edits)?;
            Ok(Some(*annotation_id))
        }
        FigureEdit::AnnotationDelete { annotation_id, .. } => Ok(annotations
            .remove_reference_line(*annotation_id)
            .map(|line| line.id)),
        _ => Ok(None),
    }
}

pub(crate) fn reference_line_reading(
    line: &PlotReferenceLine,
    drawn: Option<&plotium_render::ReferenceLabelBox>,
) -> Value {
    let mut reading = json!({"id":line.id,"kind":"reference_line","orientation":line.orientation,"value":line.value,"label":line.label,"font_pt":line.font_size,"line":{"color":line.color.to_hex(),"width_pt":reference_line_width_bound(line.width),"style":line.line_style},"anchor_space":"data"});
    if line.hand_moved {
        reading["hand_moved"] = json!(true);
    }
    if !line.label.is_empty()
        && let (Some(out), Value::Object(placement)) = (
            reading.as_object_mut(),
            reference_placement_reading(line.orientation, line.label_placement, drawn),
        )
    {
        out.extend(placement);
    }
    reading
}
