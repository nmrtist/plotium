//! Keyboard ownership for the data-sheet grid. The grid holds a stable egui
//! focus id only after the user clicks into it; every global shortcut handler
//! already yields while egui reports keyboard focus, so canvas keys reach the
//! canvas unless the table was explicitly chosen.

use egui::{Context, EventFilter, Id, Rect, Sense, Ui};

fn grid_id() -> Id {
    Id::new("plotium_data_sheet_grid")
}

pub(in crate::ui) fn grid_has_focus(ctx: &Context) -> bool {
    ctx.memory(|memory| memory.has_focus(grid_id()))
}

/// Re-requesting an already focused id would reset its lock filter, letting
/// the next arrow or Tab move egui focus away from the grid.
pub(super) fn claim(ctx: &Context) {
    if !grid_has_focus(ctx) {
        ctx.memory_mut(|memory| memory.request_focus(grid_id()));
        // The lock filter applies from the next pass; run it before more keys.
        ctx.request_repaint();
    }
}

pub(super) fn release(ctx: &Context) {
    ctx.memory_mut(|memory| memory.surrender_focus(grid_id()));
}

/// Registers the grid every frame (egui drops focus from ids it no longer
/// sees) and keeps focus across clicks inside the grid, which egui would
/// otherwise surrender because the cell under the pointer is the hovered
/// widget. A press anywhere else gives the keyboard back to the app.
pub(super) fn maintain(ui: &mut Ui, rect: Rect) {
    let ctx = ui.ctx().clone();
    let (pressed, position) =
        ui.input(|input| (input.pointer.any_pressed(), input.pointer.interact_pos()));
    let inside = position.is_some_and(|position| {
        rect.contains(position) && ctx.layer_id_at(position) == Some(ui.layer_id())
    });
    // Read before registering: includes a claim made by a cell, header or
    // editor earlier this frame and excludes an Escape release.
    let wanted = grid_has_focus(&ctx);
    ui.interact(rect, grid_id(), Sense::focusable_noninteractive());
    if pressed && !inside {
        release(&ctx);
    } else if wanted {
        claim(&ctx);
    }
    ctx.memory_mut(|memory| {
        memory.set_focus_lock_filter(
            grid_id(),
            EventFilter {
                tab: true,
                horizontal_arrows: true,
                vertical_arrows: true,
                escape: true,
            },
        );
    });
}
