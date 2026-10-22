use super::*;
use crate::ui::properties::LocalizedText;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PointerNavigation {
    Pan(Option<ObjectId>),
    Zoom(ObjectId, ZoomAxis),
}

pub(crate) fn pan_input(ui: &Ui) -> bool {
    !ui.ctx().egui_wants_keyboard_input()
        && ui.input(|i| {
            i.pointer.middle_down() || (i.key_down(egui::Key::Space) && i.pointer.primary_down())
        })
}

/// Resolve the next drag before layout/furniture dispatch, just as navigation
/// does. Both hover feedback and gesture start must use this target and order.
pub(crate) fn pointer_navigation(
    app: &PlotiumApp,
    ci: usize,
    rect: EguiRect,
    p: Pos2,
    ui: &Ui,
) -> Option<PointerNavigation> {
    if !rect.contains(p)
        || ui.ctx().egui_wants_keyboard_input()
        || !matches!(app.interaction(), Interaction::Idle)
    {
        return None;
    }
    let (command, pan, alt) = ui.input(|i| {
        (
            i.modifiers.command || i.modifiers.ctrl,
            i.pointer.middle_down() || i.key_down(egui::Key::Space),
            i.modifiers.alt,
        )
    });
    let target = (!command)
        .then(|| plot_under_cursor(app, ci, rect, p))
        .flatten();
    if pan {
        return Some(PointerNavigation::Pan(target.map(|(id, _, _)| id)));
    }
    let (id, outer, plot) = target?;
    let axis = match hit_zone(p, outer, plot) {
        HitZone::XAxis => ZoomAxis::X,
        HitZone::YAxis => ZoomAxis::Y,
        HitZone::Plot if alt => ZoomAxis::Box,
        HitZone::Plot | HitZone::None => return None,
    };
    Some(PointerNavigation::Zoom(id, axis))
}

pub(crate) fn axis_zoom_hint(axis: ZoomAxis) -> Option<&'static str> {
    match axis {
        ZoomAxis::X => Some(LocalizedText("Drag to zoom X axis").get()),
        ZoomAxis::Y => Some(LocalizedText("Drag to zoom Y axis").get()),
        ZoomAxis::Box => None,
    }
}

pub(crate) fn navigation_cursor(
    app: &PlotiumApp,
    ci: usize,
    rect: EguiRect,
    ui: &Ui,
) -> Option<(egui::CursorIcon, Option<&'static str>)> {
    use egui::CursorIcon::{Crosshair, Grab, Grabbing};
    // Locked gestures outrank hover location and modifier changes.
    match app.interaction() {
        Interaction::Zoom(_) => return Some((Crosshair, None)),
        Interaction::Pan(_) | Interaction::Furniture(_) => return Some((Grabbing, None)),
        _ => {}
    }
    let p = ui.input(|i| i.pointer.hover_pos())?;
    match pointer_navigation(app, ci, rect, p, ui)? {
        PointerNavigation::Zoom(_, axis) => Some((Crosshair, axis_zoom_hint(axis))),
        PointerNavigation::Pan(_) => Some((if pan_input(ui) { Grabbing } else { Grab }, None)),
    }
}

#[cfg(test)]
#[path = "navigation_pointer_tests.rs"]
mod tests;
