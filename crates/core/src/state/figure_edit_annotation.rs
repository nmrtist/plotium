//! Data-anchored plot annotation edits inside a native figure transaction.
use super::figure_edit_arrow::parse_color;
use super::plot_label_placement::{guard_hand_position, request_placement};
use super::*;
use crate::automation::{TargetRef, canvas_object_ref};
use serde_json::{Value, json};

fn plot_id(page: &CanvasDocument, target: &TargetRef) -> Result<ContentId, String> {
    let item = page
        .objects
        .iter()
        .find(|o| *target == TargetRef::resource(canvas_object_ref(page.resource_id, o.id)))
        .ok_or("Discover an exact plot target on this canvas.")?;
    if item.plot().is_none() {
        return Err("Annotation target is not a plot.".into());
    }
    page.validate_content_edit(item.id, false)?;
    Ok(item.id)
}

/// Apply one annotation op to the candidate page and report the annotation it
/// addressed, so callers can reference a newly minted id in later edits.
pub(super) fn apply(page: &mut CanvasDocument, edit: &FigureEdit) -> Result<Value, String> {
    let (target, op) = match edit {
        FigureEdit::AnnotationCreate { target, .. } => (target, "created"),
        FigureEdit::AnnotationSet { target, .. } => (target, "modified"),
        FigureEdit::AnnotationDelete { target, .. } => (target, "deleted"),
        _ => return Err("Not an annotation edit.".into()),
    };
    let object = plot_id(page, target)?;
    let plot = page
        .object_mut(object)
        .and_then(ContentItem::plot_mut)
        .ok_or("Plot target disappeared.")?;
    let mut annotations = plot.axis_overrides.annotations.clone();
    // New supporting lines start at the document's supporting-line width,
    // when it is one a reference line or arrow may take.
    let aux_line_pt = plot
        .figure()
        .typography
        .aux_line_pt
        .filter(|width| (0.1..=20.0).contains(width));
    let missing = || "Discover an exact annotation id on this plot.".to_owned();
    let id = if let Some(id) = super::figure_edit_arrow::apply(&mut annotations, edit, aux_line_pt)?
    {
        id
    } else if let Some(id) =
        super::figure_edit_reference_line::apply(&mut annotations, edit, aux_line_pt)?
    {
        id
    } else {
        match edit {
            FigureEdit::AnnotationCreate {
                text,
                at,
                font_pt,
                color,
                ..
            } => match (text, at, font_pt) {
                (Some(text), Some(at), Some(font_pt)) => {
                    let color = parse_color(color.as_deref())?;
                    let id = annotations.add_text(*at, text.clone(), *font_pt)?;
                    if let (Some(color), Some(annotation)) = (color, annotations.get_mut(id)) {
                        annotation.color = color;
                    }
                    id
                }
                _ => return Err("A text annotation requires text, at and font_pt.".into()),
            },
            FigureEdit::AnnotationSet {
                annotation_id: id,
                text,
                at,
                font_pt,
                color,
                offset_pt,
                placement,
                override_hand_edits,
                ..
            } => {
                if text.is_none()
                    && at.is_none()
                    && font_pt.is_none()
                    && color.is_none()
                    && offset_pt.is_none()
                    && placement.is_none()
                {
                    return Err(
                        "Annotation edit requires text, at, font_pt, color, offset_pt or placement."
                            .into(),
                    );
                }
                let id = *id;
                let annotation = annotations.get_mut(id).ok_or_else(missing)?;
                if let Some(text) = text {
                    annotation.text.clone_from(text);
                }
                if let Some(at) = at {
                    // A hand-placed text keeps its offset from the anchor, so
                    // a new anchor would carry it away from where it was put.
                    if *at != annotation.at {
                        guard_hand_position(
                            annotation.placement.is_hand(),
                            id,
                            *override_hand_edits,
                        )?;
                    }
                    annotation.at = *at;
                }
                if let Some(color) = parse_color(color.as_deref())? {
                    annotation.color = color;
                }
                if let Some(pt) = font_pt {
                    annotation.font_size = *pt;
                }
                let requested = match (offset_pt, placement) {
                    (Some(_), Some(_)) => {
                        return Err("Pass offset_pt or placement, not both.".into());
                    }
                    (Some(offset), None) => Some(LabelPlacement::Fixed(*offset)),
                    (None, Some(PlacementRequest::Auto)) => Some(LabelPlacement::Auto),
                    (None, None) => None,
                };
                if let Some(requested) = requested {
                    request_placement(
                        &mut annotation.placement,
                        requested,
                        id,
                        *override_hand_edits,
                    )?;
                }
                annotation.validate()?;
                id
            }
            FigureEdit::AnnotationDelete {
                annotation_id: id, ..
            } => {
                annotations.remove(*id).ok_or_else(missing)?;
                *id
            }
            _ => return Err("Not an annotation edit.".into()),
        }
    };
    plot.set_annotations(annotations);
    let reading = annotation_readings(page, object)
        .into_iter()
        .find(|reading| reading["id"] == json!(id));
    Ok(json!({"target":target,"op":op,"annotation_id":id,"annotation":reading}))
}

/// Every annotation of a plot with its label placement as drawn at the
/// plot's page size.
pub(crate) fn annotation_readings(page: &CanvasDocument, object: ObjectId) -> Vec<Value> {
    let Some(plot) = page.object(object).and_then(ContentItem::plot) else {
        return Vec::new();
    };
    let annotations = &plot.axis_overrides.annotations;
    let layout = super::plot_label_layout(page, object).unwrap_or_default();
    let texts =
        annotations.items.iter().enumerate().map(|(index, a)| {
            annotation_reading(a, layout.texts.get(index).and_then(Option::as_ref))
        });
    let lines = annotations
        .reference_lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            super::reference_line_reading(
                line,
                layout.reference.get(index).and_then(Option::as_ref),
            )
        });
    let arrows = annotations
        .arrows
        .iter()
        .map(super::figure_edit_arrow::arrow_reading);
    texts.chain(lines).chain(arrows).collect()
}

pub(crate) fn annotation_reading(
    annotation: &PlotAnnotation,
    drawn: Option<&plotium_render::TextLabelBox>,
) -> Value {
    let mut reading = json!({"id":annotation.id,"kind":"text","text":annotation.text,"at":annotation.at,"font_pt":annotation.font_size,"color":annotation.color.to_hex(),"anchor_space":"data"});
    if let (Some(out), Value::Object(placement)) = (
        reading.as_object_mut(),
        super::text_placement_reading(annotation.placement, drawn),
    ) {
        out.extend(placement);
    }
    reading
}
