//! Desktop edits of a plot's author annotations. Each one is a single undo
//! step on the plot's axis overrides.
use plotium_core::actions::Action;
use plotium_core::state::{
    AnnotationId, AxisOverrides, DEFAULT_ARROW_LINE_WIDTH_PT, ObjectId, PlotAnnotations,
    PlotMenuPoint, PlotPart, PlotReferenceLine, PlotTextEditState, PlotiumApp, Tool,
};
use plotium_figure::{ArrowHead, Color, ReferenceOrientation};

/// Text a new text annotation starts with, until the user types its own.
const NEW_TEXT: &str = "Text";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NewAnnotation {
    Text,
    Arrow,
    Line(ReferenceOrientation),
}

fn overrides(app: &PlotiumApp, ci: usize, object: ObjectId) -> Option<AxisOverrides> {
    app.doc
        .canvases
        .get(ci)?
        .object(object)?
        .plot()
        .map(|plot| plot.axis_overrides.clone())
}

/// Apply `edit` to plot `object`'s annotations as one undo step. An error
/// leaves the plot unchanged and is shown in the status bar.
pub(crate) fn commit<T>(
    app: &mut PlotiumApp,
    ci: usize,
    object: ObjectId,
    edit: impl FnOnce(&mut PlotAnnotations) -> Result<T, String>,
) -> Option<T> {
    // A live Inspector edit of the same plot lands first, as its own step.
    app.finish_axis_overrides_edit();
    let before = overrides(app, ci, object)?;
    let mut after = before.clone();
    match edit(&mut after.annotations) {
        Ok(value) => {
            if after != before {
                app.execute_action(Action::set_axis_overrides(ci, object, before, after));
            }
            Some(value)
        }
        Err(error) => {
            app.session.status = error;
            None
        }
    }
}

/// Add an annotation at `point`, select it and, for text, open its editor.
pub(crate) fn create(app: &mut PlotiumApp, point: PlotMenuPoint, kind: NewAnnotation) {
    let Some(ci) = app.doc.canvas_index(point.canvas) else {
        return;
    };
    let Some(typography) = app
        .doc
        .canvases
        .get(ci)
        .and_then(|page| page.object(point.object))
        .and_then(|object| object.plot())
        .map(|plot| plot.figure().typography)
    else {
        return;
    };
    // New supporting lines take the document's supporting-line width, when it
    // is one a reference line or arrow may take, as automation edits do.
    let aux_line_pt = typography
        .aux_line_pt
        .filter(|width| (0.1..=20.0).contains(width));
    let font_pt = typography.tick_pt.clamp(1.0, 72.0);
    let Some(id) = commit(app, ci, point.object, |annotations| match kind {
        NewAnnotation::Text => annotations.add_text(point.at, NEW_TEXT.into(), font_pt),
        NewAnnotation::Arrow => annotations.add_arrow(
            point.tail,
            point.at,
            ArrowHead::Filled,
            aux_line_pt.unwrap_or(DEFAULT_ARROW_LINE_WIDTH_PT),
            Color::BLACK,
        ),
        NewAnnotation::Line(orientation) => {
            let value = match orientation {
                ReferenceOrientation::Vertical => point.at[0],
                ReferenceOrientation::Horizontal => point.at[1],
            };
            let mut line = PlotReferenceLine::new(annotations.next_id, orientation, value);
            if let Some(width) = aux_line_pt {
                line.width = width;
            }
            annotations.add_reference_line(line)
        }
    }) else {
        return;
    };
    let part = match kind {
        NewAnnotation::Text => PlotPart::Label(id),
        _ => PlotPart::Annotation(id),
    };
    app.select_plot_part(ci, point.object, part);
    if kind == NewAnnotation::Text {
        open_text_editor(app, ci, point.object, id);
    }
    app.session.status = "Added annotation.".into();
}

/// Open the text of text annotation or reference line `annotation` for
/// editing, selecting it.
pub(crate) fn open_text_editor(
    app: &mut PlotiumApp,
    ci: usize,
    object: ObjectId,
    annotation: AnnotationId,
) {
    let Some(overrides) = overrides(app, ci, object) else {
        return;
    };
    let annotations = &overrides.annotations;
    let (buffer, part) = if let Some(text) = annotations.get(annotation) {
        (text.text.clone(), PlotPart::Label(annotation))
    } else if let Some(line) = annotations.reference_line(annotation) {
        (line.label.clone(), PlotPart::Label(annotation))
    } else {
        return;
    };
    app.select_plot_part(ci, object, part);
    app.session.ui.plot_text_edit = Some(PlotTextEditState {
        canvas: app.doc.canvases[ci].resource_id,
        object,
        annotation,
        buffer,
        focus: true,
    });
}

/// Replace the text of a text annotation, or the label of a reference line
/// (where empty removes it).
pub(crate) fn set_text(
    app: &mut PlotiumApp,
    ci: usize,
    object: ObjectId,
    annotation: AnnotationId,
    text: &str,
) {
    let text = text.trim();
    let changed = commit(app, ci, object, |annotations| {
        if let Some(item) = annotations.get_mut(annotation) {
            item.text = text.to_owned();
            item.validate()
        } else if let Some(line) = annotations.reference_line_mut(annotation) {
            line.label = text.to_owned();
            line.validate()
        } else {
            Err("The annotation no longer exists.".into())
        }
    });
    if changed.is_some() {
        app.session.status = "Annotation text updated.".into();
    }
}

/// Return a text annotation or reference label to automatic placement.
pub(crate) fn place_automatically(
    app: &mut PlotiumApp,
    ci: usize,
    object: ObjectId,
    annotation: AnnotationId,
) {
    let Some(placed) = app
        .doc
        .canvases
        .get(ci)
        .and_then(|page| page.object(object))
        .and_then(|item| item.plot())
        .map(|plot| crate::ui::canvas::auto_label(plot, annotation).annotations)
    else {
        return;
    };
    if commit(app, ci, object, |annotations| {
        *annotations = placed;
        Ok(())
    })
    .is_some()
    {
        app.session.status = "Restored automatic label placement.".into();
    }
}

pub(crate) fn delete(app: &mut PlotiumApp, ci: usize, object: ObjectId, annotation: AnnotationId) {
    let deleted = commit(app, ci, object, |annotations| {
        Ok(annotations.remove_any(annotation))
    });
    if deleted == Some(true) {
        app.session.ui.plot_part_selection = None;
        app.session.status = "Annotation deleted.".into();
    }
}

/// The selected annotation as (canvas index, plot, annotation).
pub(crate) fn selected(app: &PlotiumApp) -> Option<(usize, ObjectId, AnnotationId)> {
    let (ci, selected) = app.selected_plot_part()?;
    match selected.part {
        PlotPart::Label(id) | PlotPart::Annotation(id) => Some((ci, selected.object, id)),
        PlotPart::Legend | PlotPart::RegionLabel { .. } => None,
    }
}

/// Delete the selected annotation; `false` when none is selected. Other
/// tools own Delete for their own marks.
pub(crate) fn delete_selected(app: &mut PlotiumApp) -> bool {
    if app.session.tool != Tool::Select {
        return false;
    }
    let Some((ci, object, annotation)) = selected(app) else {
        return false;
    };
    let locked = app.doc.canvases[ci]
        .object(object)
        .is_none_or(|item| item.locked);
    if !locked {
        delete(app, ci, object, annotation);
    }
    true
}

#[cfg(test)]
#[path = "plot_annotations_tests.rs"]
pub(crate) mod tests;
