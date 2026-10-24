//! Keyboard ownership for the data-sheet grid. The grid holds a stable egui
//! focus id only after the user clicks into it; every global shortcut handler
//! already yields while egui reports keyboard focus, so canvas keys reach the
//! canvas unless the table was explicitly chosen.

use egui::{Context, EventFilter, Id, Rect, Sense, Ui};
use plotium_core::data::{ColumnId, RowId};
use plotium_core::state::{DatasetId, TableCellEditDraft};

fn grid_id() -> Id {
    Id::new("plotium_data_sheet_grid")
}

/// Stable per cell: auto ids shift when an edit starts mid-frame or the table
/// scrolls to it, and a changed id drops keyboard focus from the editor.
pub(super) fn editor_id(row: RowId, column: ColumnId) -> Id {
    Id::new(("plotium_data_sheet_editor", row, column))
}

fn keyboard_owner_id() -> Id {
    Id::new("plotium_data_sheet_keyboard_owner")
}

/// Records at the end of the sheet's pass whether its grid or cell editor
/// holds the keyboard. egui drops an editor's focus on Escape before any
/// handler runs, so global shortcuts read this to leave that Escape alone.
pub(super) fn record_keyboard_owner(
    ctx: &Context,
    shown: bool,
    draft: Option<&TableCellEditDraft>,
) {
    let editor = draft.map(|edit| editor_id(edit.row, edit.column));
    let owned = shown
        && ctx.memory(|memory| {
            memory
                .focused()
                .is_some_and(|id| id == grid_id() || Some(id) == editor)
        });
    ctx.data_mut(|data| data.insert_temp(keyboard_owner_id(), owned));
}

/// Whether the sheet held the keyboard at the end of the previous pass.
pub(in crate::ui) fn sheet_held_keyboard(ctx: &Context) -> bool {
    ctx.data(|data| data.get_temp::<bool>(keyboard_owner_id()))
        .unwrap_or(false)
}

fn first_cell_request_id() -> Id {
    Id::new("plotium_data_sheet_first_cell")
}

/// Ask the sheet to select the first cell of `dataset` and take the keyboard
/// the next time it shows that table, so typing starts in a new table.
pub(in crate::ui) fn request_first_cell(ctx: &Context, dataset: DatasetId) {
    ctx.data_mut(|data| data.insert_temp(first_cell_request_id(), dataset));
}

/// Whether `dataset`, now shown, was asked to take the keyboard. A request
/// for a table that is not the one shown is dropped instead of firing later.
pub(super) fn first_cell_requested(ctx: &Context, dataset: DatasetId) -> bool {
    ctx.data_mut(
        |data| match data.get_temp::<DatasetId>(first_cell_request_id()) {
            Some(requested) if requested == dataset => true,
            Some(_) => {
                data.remove::<DatasetId>(first_cell_request_id());
                false
            }
            None => false,
        },
    )
}

pub(super) fn take_first_cell_request(ctx: &Context, dataset: DatasetId) -> bool {
    ctx.data_mut(|data| {
        let requested = data.get_temp::<DatasetId>(first_cell_request_id()) == Some(dataset);
        if requested {
            data.remove::<DatasetId>(first_cell_request_id());
        }
        requested
    })
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
    // The docked panel shares the background layer with the board and the
    // sidebars, and a wide table extends past the visible viewport.
    let visible = rect.intersect(ui.clip_rect());
    let inside = position.is_some_and(|position| {
        visible.contains(position) && ctx.layer_id_at(position) == Some(ui.layer_id())
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
