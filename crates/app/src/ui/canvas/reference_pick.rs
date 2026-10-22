//! The one-shot on-plot pick for a Reference step's source position.
//!
//! While the step editor arms a pick, hovering the target plot previews the
//! snapped position and a click writes the step's `at_ppm` through the
//! property catalog (one undo step, same recompute as typing the number).
//! The mode's lifetime is owned by `PlotiumApp::resolve_reference_pick`.

use super::*;
use plotium_core::automation::{ComponentRef, ResourceRef, TargetRef};
use plotium_core::properties::PropertyValue;
use plotium_core::state::{PhaseOrient, ResolvedReferencePick};

#[cfg(test)]
#[path = "reference_pick_tests.rs"]
mod tests;

/// Screen-space geometry of the pick target plot: the axis mappings copied out
/// of the figure so previews and commits share one conversion.
#[derive(Clone)]
struct PickGeometry {
    x_axis: plotium_figure::Axis,
    y_axis: plotium_figure::Axis,
}

impl PickGeometry {
    fn of(figure: &plotium_figure::Figure) -> Self {
        Self {
            x_axis: figure.x.clone(),
            y_axis: figure.y.clone(),
        }
    }
}

/// The previewed pick under the pointer: the position in finished-axis ppm and
/// where its guide line and optional apex dot sit on screen.
struct PickedPosition {
    ppm: f64,
    line_px: f32,
    apex: Option<Pos2>,
}

/// Input half of the pick. Returns `true` while the pointer hovers the armed
/// plot, so the caller keeps layout and data gestures from starting under a
/// click that is meant for the pick. `pointer_allowed` carries the caller's
/// pointer-ownership verdict; Escape disarms regardless of it.
pub(crate) fn handle_reference_pick(
    app: &mut PlotiumApp,
    ci: usize,
    board_rect: egui::Rect,
    ui: &Ui,
    pointer_allowed: bool,
) -> bool {
    let Some(resolved) = app.resolve_reference_pick() else {
        return false;
    };
    let (esc, hover, pressed, shift) = ui.input(|input| {
        (
            input.key_pressed(egui::Key::Escape),
            input.pointer.hover_pos(),
            input.pointer.primary_pressed(),
            input.modifiers.shift,
        )
    });
    if esc {
        app.session.ui.reference_pick = None;
        app.session.status = "Reference pick cancelled.".to_owned();
        return false;
    }
    if !pointer_allowed || app.interaction().is_active() {
        return false;
    }
    let Some((plot, geometry)) = pick_plot(app, ci, board_rect, &resolved) else {
        return false;
    };
    let Some(p) = hover.filter(|p| plot_contains(plot, *p)) else {
        return false;
    };
    if pressed && let Some(picked) = picked_position(app, &resolved, geometry, plot, p, shift) {
        commit_reference_pick(app, &resolved, picked.ppm);
    }
    true
}

/// Preview half of the pick, painted with the other plot chrome: a guide line
/// at the snapped position, its apex, and a ppm readout by the cursor.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_reference_pick(
    app: &PlotiumApp,
    ci: usize,
    object_id: ObjectId,
    di: usize,
    plot: PlotRect,
    ui: &Ui,
    painter: &egui::Painter,
    chrome: ChromeStyle,
) {
    let Some(resolved) = app.resolve_reference_pick() else {
        return;
    };
    if resolved.dataset_index != di || app.interaction().is_active() {
        return;
    }
    let Some(figure) = app.doc.canvases[ci]
        .object(object_id)
        .and_then(|object| object.plot())
        .map(|plot| plot.figure())
    else {
        return;
    };
    let geometry = PickGeometry::of(figure);
    let (hover, shift) = ui.input(|input| (input.pointer.hover_pos(), input.modifiers.shift));
    let Some(p) = hover.filter(|p| plot_contains(plot, *p)) else {
        return;
    };
    ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    let Some(picked) = picked_position(app, &resolved, geometry, plot, p, shift) else {
        return;
    };
    let stroke = Stroke::new(1.5_f32, chrome.pivot);
    let (a, b) = match resolved.axis.orient() {
        PhaseOrient::Vertical => (
            Pos2::new(picked.line_px, plot.top),
            Pos2::new(picked.line_px, plot.bottom()),
        ),
        PhaseOrient::Horizontal => (
            Pos2::new(plot.left, picked.line_px),
            Pos2::new(plot.right(), picked.line_px),
        ),
    };
    // Dashed, so the transient pick guide never reads as the phase pivot line.
    painter.add(egui::Shape::dashed_line(&[a, b], stroke, 5.0, 4.0));
    if let Some(apex) = picked.apex {
        painter.circle_filled(apex, 3.5, chrome.pivot);
    }
    let galley = painter.layout_no_wrap(
        format!("{:.3} ppm", picked.ppm),
        egui::FontId::proportional(11.0),
        chrome.pivot,
    );
    let anchor = p + egui::vec2(12.0, -18.0);
    painter.rect_filled(
        egui::Rect::from_min_size(anchor, galley.size()).expand(3.0),
        3.0,
        Color32::from_black_alpha(if ui.visuals().dark_mode { 150 } else { 20 }),
    );
    painter.galley(anchor, galley, chrome.pivot);
}

/// The plot the armed pick may act on: the canvas's data-edit or active plot,
/// but only while it shows the picked dataset.
fn pick_plot(
    app: &PlotiumApp,
    ci: usize,
    board_rect: egui::Rect,
    resolved: &ResolvedReferencePick,
) -> Option<(PlotRect, PickGeometry)> {
    let object_id =
        data_edit_target(app, ci).or_else(|| app.doc.canvases[ci].active_plot_object_id())?;
    let object = app.doc.canvases[ci].object(object_id)?;
    let di = object.dataset().and_then(|id| app.doc.dataset_index(id))?;
    if di != resolved.dataset_index {
        return None;
    }
    let outer = object_screen_rect(
        app.session.board,
        &app.doc.canvases[ci],
        object_id,
        board_rect,
    )?;
    let figure = object.plot()?.figure();
    let zoom = app.session.board.zoom;
    let layout = plotium_render::axis_layout(figure, outer.width / zoom, outer.height / zoom);
    let plot = plotium_render::Projector::new(figure, outer, &layout.margins.scaled(zoom)).plot;
    Some((plot, PickGeometry::of(figure)))
}

/// Resolve the pointer into a picked position. On a 1D trace the pick snaps to
/// the same zoom-scaled apex search manual peak picking uses (`Shift` = nearest
/// sample); an axis without a 1D trace (a 2D dimension) picks the raw
/// coordinate.
fn picked_position(
    app: &PlotiumApp,
    resolved: &ResolvedReferencePick,
    geometry: PickGeometry,
    plot: PlotRect,
    p: Pos2,
    shift: bool,
) -> Option<PickedPosition> {
    match resolved.axis.orient() {
        PhaseOrient::Vertical => {
            let x = from_screen_x(p.x, plot, &geometry.x_axis);
            let trace = app
                .doc
                .datasets
                .get(resolved.dataset_index)
                .and_then(|dataset| dataset.displayed_trace(None));
            match trace {
                Some(trace) => {
                    let snap = super::peaks::manual_peak_snap(shift, plot, &geometry.x_axis, p.x);
                    let (px, py) = trace.pick(x, snap);
                    let sx = to_screen_x(px, plot, &geometry.x_axis);
                    let sy = to_screen_y(py, plot, &geometry.y_axis);
                    Some(PickedPosition {
                        ppm: px,
                        line_px: sx,
                        apex: plot_contains(plot, Pos2::new(sx, sy)).then_some(Pos2::new(sx, sy)),
                    })
                }
                None => Some(PickedPosition {
                    ppm: x,
                    line_px: p.x,
                    apex: None,
                }),
            }
        }
        PhaseOrient::Horizontal => Some(PickedPosition {
            ppm: from_screen_y(p.y, plot, &geometry.y_axis),
            line_px: p.y,
            apex: None,
        }),
    }
}

/// Write the picked position into the step's `at_ppm` through the property
/// catalog. The displayed ppm converts into the step's own axis coordinates by
/// removing the calibration that reference steps from this one onward apply
/// (`AxisPipeline::chemical_shift_offset_from_step_ppm`), so after the edit the
/// picked feature reads exactly `target_ppm`.
fn commit_reference_pick(app: &mut PlotiumApp, resolved: &ResolvedReferencePick, picked_ppm: f64) {
    let offset = app
        .doc
        .datasets
        .get(resolved.dataset_index)
        .and_then(|dataset| dataset.axis_pipeline(resolved.axis))
        .map(|pipeline| pipeline.chemical_shift_offset_from_step_ppm(resolved.pick.step))
        .unwrap_or(0.0);
    let at_ppm = picked_ppm - offset;
    app.session.ui.reference_pick = None;
    let target = TargetRef {
        resource: ResourceRef::from(resolved.pick.dataset),
        component: Some(ComponentRef::ProcessingStep(resolved.pick.step)),
    };
    match app.plan_property_write(
        plotium_core::properties::reference::AT_PPM,
        std::slice::from_ref(&target),
        &PropertyValue::Float(at_ppm),
    ) {
        Ok(commit) => {
            app.commit_property(commit);
            app.session.status =
                format!("Reference source set to {at_ppm:.3} ppm — now set the target position.");
        }
        Err(error) => {
            app.session.status = format!("Could not set the reference source: {error}");
        }
    }
}
