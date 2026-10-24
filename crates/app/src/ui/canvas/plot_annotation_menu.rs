//! The right-click menu inside a plot: add an annotation where it was
//! clicked, or edit the annotation under the pointer.
use super::*;
use crate::ui::plot_annotations::{self, NewAnnotation};
use plotium_core::state::{LabelPlacement, PlotMenuPoint, PlotPart};
use plotium_figure::ReferenceOrientation;

/// Screen px from the clicked point to a new arrow's tail along each axis.
const ARROW_TAIL_PX: f32 = 24.0;

/// Record what a right-click points at. An annotation under the pointer is
/// selected; an empty spot inside a plot becomes the place for new ones.
pub(crate) fn note_secondary_click(
    app: &mut PlotiumApp,
    ci: usize,
    canvas_rect: egui::Rect,
    pointer: Pos2,
) {
    app.session.ui.plot_menu_point = None;
    if app.session.tool != Tool::Select {
        return;
    }
    if let Some((object, part)) =
        super::furniture::plot_annotation_at(app, ci, canvas_rect, pointer)
    {
        let (PlotPart::Label(id) | PlotPart::Annotation(id)) = part else {
            return;
        };
        app.select_plot_part(ci, object, part);
        app.session.ui.plot_menu_point = Some(PlotMenuPoint {
            canvas: app.doc.canvases[ci].resource_id,
            object,
            annotation: Some(id),
            at: [0.0, 0.0],
            tail: [0.0, 0.0],
        });
        return;
    }
    let Some((object, _, plot)) = plot_under_cursor(app, ci, canvas_rect, pointer) else {
        return;
    };
    let Some(figure) = app.doc.canvases[ci]
        .object(object)
        .filter(|item| item.visible && !item.locked)
        .and_then(|item| item.plot())
        .map(|plot| plot.figure())
    else {
        return;
    };
    if !plot_contains(plot, pointer) {
        return;
    }
    let data = |p: Pos2| {
        [
            from_screen_x(p.x, plot, &figure.x),
            from_screen_y(p.y, plot, &figure.y),
        ]
    };
    // The tail sits up and to the left of the point, flipped along an axis
    // when that would leave the plot.
    let offset = |at: f32, lo: f32| {
        if at - ARROW_TAIL_PX >= lo {
            -ARROW_TAIL_PX
        } else {
            ARROW_TAIL_PX
        }
    };
    let tail = Pos2::new(
        pointer.x + offset(pointer.x, plot.left),
        pointer.y + offset(pointer.y, plot.top),
    );
    let (at, tail) = (data(pointer), data(tail));
    if at.iter().chain(&tail).all(|v| v.is_finite()) {
        app.session.ui.plot_menu_point = Some(PlotMenuPoint {
            canvas: app.doc.canvases[ci].resource_id,
            object,
            annotation: None,
            at,
            tail,
        });
    }
}

/// The plot annotation entries of the canvas menu. Returns `true` when they
/// are the whole menu, for a right-click on an annotation.
pub(crate) fn menu_items(app: &mut PlotiumApp, ci: usize, ui: &mut Ui) -> bool {
    let page = app.doc.canvases[ci].resource_id;
    let Some(point) = app.session.ui.plot_menu_point.filter(|p| p.canvas == page) else {
        return false;
    };
    let Some(annotation) = point.annotation else {
        for (label, kind) in [
            ("Add text here", NewAnnotation::Text),
            ("Add arrow here", NewAnnotation::Arrow),
            (
                "Add vertical line here",
                NewAnnotation::Line(ReferenceOrientation::Vertical),
            ),
            (
                "Add horizontal line here",
                NewAnnotation::Line(ReferenceOrientation::Horizontal),
            ),
        ] {
            if ui.button(label).clicked() {
                plot_annotations::create(app, point, kind);
                ui.close();
            }
        }
        ui.separator();
        return false;
    };
    let Some(annotations) = app.doc.canvases[ci]
        .object(point.object)
        .and_then(|item| item.plot())
        .map(|plot| &plot.axis_overrides.annotations)
    else {
        ui.close();
        return true;
    };
    let text = annotations.get(annotation);
    let line = annotations.reference_line(annotation);
    let has_text = text.is_some() || line.is_some();
    let pinned = text.is_some_and(|t| t.placement != LabelPlacement::Auto)
        || line.is_some_and(|l| !l.label.is_empty() && l.label_placement != LabelPlacement::Auto);
    if has_text && ui.button("Edit text…").clicked() {
        plot_annotations::open_text_editor(app, ci, point.object, annotation);
        ui.close();
    }
    if pinned && ui.button("Automatic position").clicked() {
        plot_annotations::place_automatically(app, ci, point.object, annotation);
        ui.close();
    }
    if ui.button("Delete annotation").clicked() {
        plot_annotations::delete(app, ci, point.object, annotation);
        ui.close();
    }
    true
}
