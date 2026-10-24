//! Arrow kind of the plot annotation edits in a figure transaction.
use super::figure_edit_reference_line::MISSING_ANNOTATION;
use super::plot_label_placement::guard_hand_position;
use super::*;
use plotium_figure::{ArrowHead, Color};
use serde_json::{Value, json};

struct ArrowFields<'a> {
    to: Option<[f64; 2]>,
    head: Option<ArrowHead>,
    line_width_pt: Option<f32>,
    color: Option<&'a str>,
}

impl ArrowFields<'_> {
    fn is_empty(&self) -> bool {
        self.is_shape_empty() && self.color.is_none()
    }

    /// No arrow-only field; `color` is shared with text annotations.
    fn is_shape_empty(&self) -> bool {
        self.to.is_none() && self.head.is_none() && self.line_width_pt.is_none()
    }
}

fn arrow_fields(edit: &FigureEdit) -> ArrowFields<'_> {
    match edit {
        FigureEdit::AnnotationCreate {
            to,
            head,
            line_width_pt,
            color,
            ..
        }
        | FigureEdit::AnnotationSet {
            to,
            head,
            line_width_pt,
            color,
            ..
        } => ArrowFields {
            to: *to,
            head: *head,
            line_width_pt: *line_width_pt,
            color: color.as_deref(),
        },
        _ => ArrowFields {
            to: None,
            head: None,
            line_width_pt: None,
            color: None,
        },
    }
}

/// Whether the edit carries text or reference-line fields, which an arrow
/// would otherwise silently ignore.
fn has_other_kind_fields(edit: &FigureEdit) -> bool {
    match edit {
        FigureEdit::AnnotationSet {
            offset_pt: Some(_), ..
        }
        | FigureEdit::AnnotationSet {
            placement: Some(_), ..
        } => true,
        FigureEdit::AnnotationCreate {
            text,
            font_pt,
            orientation,
            value,
            label,
            line,
            ..
        }
        | FigureEdit::AnnotationSet {
            text,
            font_pt,
            orientation,
            value,
            label,
            line,
            ..
        } => {
            text.is_some()
                || font_pt.is_some()
                || orientation.is_some()
                || value.is_some()
                || label.is_some()
                || line.is_some()
        }
        _ => false,
    }
}

pub(super) fn parse_color(color: Option<&str>) -> Result<Option<Color>, String> {
    color
        .map(|text| {
            crate::automation::parse_color(text)
                .ok_or_else(|| format!("'{text}' is not a '#rrggbb' colour."))
        })
        .transpose()
}

fn update(
    arrow: &mut PlotArrow,
    at: Option<[f64; 2]>,
    fields: &ArrowFields,
    override_hand_edits: bool,
) -> Result<(), String> {
    let moves = at.is_some_and(|at| at != arrow.at) || fields.to.is_some_and(|to| to != arrow.to);
    if moves {
        guard_hand_position(arrow.hand_moved, arrow.id, override_hand_edits)?;
        arrow.hand_moved = false;
    }
    if let Some(at) = at {
        arrow.at = at;
    }
    if let Some(to) = fields.to {
        arrow.to = to;
    }
    if let Some(head) = fields.head {
        arrow.head = head;
    }
    if let Some(width) = fields.line_width_pt {
        arrow.line_width = width;
    }
    if let Some(color) = parse_color(fields.color)? {
        arrow.color = color;
    }
    arrow.validate()
}

/// Apply `edit` when it creates or addresses an arrow, returning the id it
/// addressed. `None` leaves the edit to the other annotation kinds, after
/// rejecting arrow fields that those kinds would silently ignore.
pub(super) fn apply(
    annotations: &mut PlotAnnotations,
    edit: &FigureEdit,
    default_width: Option<f32>,
) -> Result<Option<AnnotationId>, String> {
    let fields = arrow_fields(edit);
    let not_arrow = "Arrows take at, to, head, line_width_pt and color; label an arrow with a separate text annotation.";
    match edit {
        FigureEdit::AnnotationCreate {
            kind: AnnotationKind::Arrow,
            at,
            ..
        } => {
            if has_other_kind_fields(edit) {
                return Err(not_arrow.into());
            }
            let at = at.ok_or("An arrow requires a tail point at.")?;
            let to = fields.to.ok_or("An arrow requires a head point to.")?;
            annotations
                .add_arrow(
                    at,
                    to,
                    fields.head.unwrap_or(ArrowHead::Filled),
                    fields
                        .line_width_pt
                        .or(default_width)
                        .unwrap_or(DEFAULT_ARROW_LINE_WIDTH_PT),
                    parse_color(fields.color)?.unwrap_or(Color::BLACK),
                )
                .map(Some)
        }
        FigureEdit::AnnotationCreate { .. } if !fields.is_shape_empty() => Err(
            "Set kind \"arrow\" to create an arrow; to, head and line_width_pt apply only to arrows."
                .into(),
        ),
        FigureEdit::AnnotationSet {
            annotation_id,
            at,
            override_hand_edits,
            ..
        } => {
            let id = *annotation_id;
            let exists_elsewhere =
                annotations.get(id).is_some() || annotations.reference_line(id).is_some();
            let Some(arrow) = annotations.arrow_mut(id) else {
                if fields.is_shape_empty() {
                    return Ok(None);
                }
                return Err(if exists_elsewhere {
                    "to, head and line_width_pt apply only to arrows.".into()
                } else {
                    MISSING_ANNOTATION.into()
                });
            };
            if has_other_kind_fields(edit) {
                return Err(not_arrow.into());
            }
            if fields.is_empty() && at.is_none() {
                return Err("Arrow edit requires at, to, head, line_width_pt or color.".into());
            }
            update(arrow, *at, &fields, *override_hand_edits)?;
            Ok(Some(id))
        }
        FigureEdit::AnnotationDelete { annotation_id, .. } => Ok(annotations
            .remove_arrow(*annotation_id)
            .map(|arrow| arrow.id)),
        _ => Ok(None),
    }
}

pub(crate) fn arrow_reading(arrow: &PlotArrow) -> Value {
    let mut reading = json!({"id":arrow.id,"kind":"arrow","at":arrow.at,"to":arrow.to,"head":arrow.head,"line_width_pt":reference_line_width_bound(arrow.line_width),"color":arrow.color.to_hex(),"anchor_space":"data"});
    if arrow.hand_moved {
        reading["hand_moved"] = json!(true);
    }
    reading
}
