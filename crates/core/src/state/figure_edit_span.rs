//! Span kind of the plot annotation edits in a figure transaction.
use super::figure_edit_arrow::parse_color;
use super::figure_edit_reference_line::MISSING_ANNOTATION;
use super::plot_label_placement::guard_hand_position;
use super::*;
use plotium_figure::SpanAxis;
use serde_json::{Value, json};

struct SpanFields<'a> {
    axis: Option<SpanAxis>,
    range: Option<[f64; 2]>,
    opacity: Option<f32>,
    label: Option<&'a str>,
    font_pt: Option<f32>,
    color: Option<&'a str>,
}

impl SpanFields<'_> {
    /// No span-only field; label, font_pt and color are shared with other kinds.
    fn is_own_empty(&self) -> bool {
        self.axis.is_none() && self.range.is_none() && self.opacity.is_none()
    }

    fn is_empty(&self) -> bool {
        self.is_own_empty()
            && self.label.is_none()
            && self.font_pt.is_none()
            && self.color.is_none()
    }
}

fn span_fields(edit: &FigureEdit) -> SpanFields<'_> {
    match edit {
        FigureEdit::AnnotationCreate {
            axis,
            range,
            opacity,
            label,
            font_pt,
            color,
            ..
        }
        | FigureEdit::AnnotationSet {
            axis,
            range,
            opacity,
            label,
            font_pt,
            color,
            ..
        } => SpanFields {
            axis: *axis,
            range: *range,
            opacity: *opacity,
            label: label.as_deref(),
            font_pt: *font_pt,
            color: color.as_deref(),
        },
        _ => SpanFields {
            axis: None,
            range: None,
            opacity: None,
            label: None,
            font_pt: None,
            color: None,
        },
    }
}

/// Whether the edit carries fields of the text, reference-line or arrow
/// kinds, which a span would otherwise silently ignore.
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
            at,
            orientation,
            value,
            line,
            to,
            head,
            line_width_pt,
            ..
        }
        | FigureEdit::AnnotationSet {
            text,
            at,
            orientation,
            value,
            line,
            to,
            head,
            line_width_pt,
            ..
        } => {
            text.is_some()
                || at.is_some()
                || orientation.is_some()
                || value.is_some()
                || line.is_some()
                || to.is_some()
                || head.is_some()
                || line_width_pt.is_some()
        }
        _ => false,
    }
}

fn update(
    span: &mut PlotSpan,
    fields: &SpanFields,
    override_hand_edits: bool,
) -> Result<(), String> {
    let moves = fields.axis.is_some_and(|axis| axis != span.axis)
        || fields.range.is_some_and(|range| range != span.range);
    if moves {
        guard_hand_position(span.hand_moved, span.id, override_hand_edits)?;
        span.hand_moved = false;
    }
    if let Some(axis) = fields.axis {
        span.axis = axis;
    }
    if let Some(range) = fields.range {
        span.range = range;
    }
    if let Some(opacity) = fields.opacity {
        span.opacity = opacity;
    }
    if let Some(label) = fields.label {
        // An empty label removes it; whitespace alone would draw nothing.
        span.label = label.trim().to_owned();
    }
    if let Some(pt) = fields.font_pt {
        span.font_size = Some(pt);
    }
    if let Some(color) = parse_color(fields.color)? {
        span.color = color;
    }
    span.validate()
}

/// Apply `edit` when it creates or addresses a span, returning the id it
/// addressed. `None` leaves the edit to the other annotation kinds, after
/// rejecting span fields that those kinds would silently ignore.
pub(super) fn apply(
    annotations: &mut PlotAnnotations,
    edit: &FigureEdit,
) -> Result<Option<AnnotationId>, String> {
    let fields = span_fields(edit);
    let not_span = "Spans take axis, range, opacity, label, font_pt and color.";
    match edit {
        FigureEdit::AnnotationCreate {
            kind: AnnotationKind::Span,
            ..
        } => {
            if has_other_kind_fields(edit) {
                return Err(not_span.into());
            }
            let axis = fields.axis.ok_or("A span requires an axis.")?;
            let range = fields.range.ok_or("A span requires a range.")?;
            let mut span = PlotSpan::new(annotations.next_id, axis, range);
            update(&mut span, &fields, false)?;
            annotations.add_span(span).map(Some)
        }
        FigureEdit::AnnotationCreate { .. } if !fields.is_own_empty() => Err(
            "Set kind \"span\" to create a span; axis, range and opacity apply only to spans."
                .into(),
        ),
        FigureEdit::AnnotationSet {
            annotation_id,
            override_hand_edits,
            ..
        } => {
            let id = *annotation_id;
            let exists_elsewhere = annotations.get(id).is_some()
                || annotations.reference_line(id).is_some()
                || annotations.arrow(id).is_some();
            let Some(span) = annotations.span_mut(id) else {
                if fields.is_own_empty() {
                    return Ok(None);
                }
                return Err(if exists_elsewhere {
                    "axis, range and opacity apply only to spans.".into()
                } else {
                    MISSING_ANNOTATION.into()
                });
            };
            if has_other_kind_fields(edit) {
                return Err(not_span.into());
            }
            if fields.is_empty() {
                return Err(
                    "Span edit requires axis, range, opacity, label, font_pt or color.".into(),
                );
            }
            update(span, &fields, *override_hand_edits)?;
            Ok(Some(id))
        }
        FigureEdit::AnnotationDelete { annotation_id, .. } => {
            Ok(annotations.remove_span(*annotation_id).map(|span| span.id))
        }
        _ => Ok(None),
    }
}

pub(crate) fn span_reading(span: &PlotSpan) -> Value {
    let mut reading = json!({"id":span.id,"kind":"span","axis":span.axis,"range":span.range,"color":span.color.to_hex(),"opacity":reference_line_width_bound(span.opacity),"label":span.label,"font_pt":span.font_size,"anchor_space":"data"});
    if span.hand_moved {
        reading["hand_moved"] = json!(true);
    }
    reading
}
