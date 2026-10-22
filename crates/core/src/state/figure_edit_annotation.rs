//! Data-anchored plot annotation edits inside a native figure transaction.
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
    if let Some(id) = super::figure_edit_arrow::apply(&mut annotations, edit, aux_line_pt)? {
        let reading = annotations
            .arrow(id)
            .map(super::figure_edit_arrow::arrow_reading);
        plot.set_annotations(annotations);
        return Ok(json!({"target":target,"op":op,"annotation_id":id,"annotation":reading}));
    }
    if let Some(id) = super::figure_edit_reference_line::apply(&mut annotations, edit, aux_line_pt)?
    {
        let reading = annotations
            .reference_line(id)
            .map(super::figure_edit_reference_line::reference_line_reading);
        plot.set_annotations(annotations);
        return Ok(json!({"target":target,"op":op,"annotation_id":id,"annotation":reading}));
    }
    let id = match edit {
        FigureEdit::AnnotationCreate {
            text, at, font_pt, ..
        } => match (text, at, font_pt) {
            (Some(text), Some(at), Some(font_pt)) => {
                annotations.add_text(*at, text.clone(), *font_pt)?
            }
            _ => return Err("A text annotation requires text, at and font_pt.".into()),
        },
        FigureEdit::AnnotationSet {
            annotation_id: id,
            text,
            at,
            font_pt,
            ..
        } => {
            if text.is_none() && at.is_none() && font_pt.is_none() {
                return Err("Annotation edit requires text, at or font_pt.".into());
            }
            let id = *id;
            let annotation = annotations.get_mut(id).ok_or_else(missing)?;
            if let Some(text) = text {
                annotation.text.clone_from(text);
            }
            if let Some(at) = at {
                annotation.at = *at;
            }
            if let Some(pt) = font_pt {
                annotation.font_size = *pt;
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
    };
    let reading = annotations.get(id).map(annotation_reading);
    plot.set_annotations(annotations);
    Ok(json!({"target":target,"op":op,"annotation_id":id,"annotation":reading}))
}

pub(crate) fn annotation_reading(annotation: &PlotAnnotation) -> Value {
    json!({"id":annotation.id,"kind":"text","text":annotation.text,"at":annotation.at,"font_pt":annotation.font_size,"anchor_space":"data"})
}
