use egui::{Rect, Response, Ui};
use plotium_core::state::{CanvasId, FrameRef, PlotiumApp};

use super::board::{frame_header_at, frame_zoom_menu};
use crate::ui::{commands, menus};

pub(super) fn show_header_menu(app: &mut PlotiumApp, board: Rect, response: &Response) -> bool {
    let ctx = &response.ctx;
    let key = response.id.with("canvas_header_menu");
    if response.secondary_clicked() {
        let canvas = response
            .interact_pointer_pos()
            .filter(|point| board.contains(*point))
            .and_then(|point| match frame_header_at(app, board, point)? {
                FrameRef::Page(index) => Some(app.doc.canvases.get(index)?.resource_id),
                FrameRef::Sheet(_) => None,
            });
        ctx.data_mut(|data| data.insert_temp(key, canvas));
    }
    let Some(canvas) = ctx.data(|data| data.get_temp::<Option<CanvasId>>(key).flatten()) else {
        return false;
    };
    response
        .context_menu(|ui| {
            detached_preview_menu(app, ui, canvas);
            frame_zoom_menu(app, ui);
        })
        .is_some()
}

pub(crate) fn detached_preview_menu(app: &mut PlotiumApp, ui: &mut Ui, canvas: CanvasId) {
    let id = commands::CommandId::DetachedPreview(Some(canvas));
    if menus::command_row(app, ui, id, None) {
        commands::execute_without_clipboard(id, app, ui.ctx());
        ui.close();
    }
}
