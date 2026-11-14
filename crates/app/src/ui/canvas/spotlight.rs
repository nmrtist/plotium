use egui::Context;
use plotium_core::state::{
    BoardFitItem, BoardFrameId, FrameRef, Interaction, PlotiumApp, PrimaryView,
    board_filter_active, captured_board_fit,
};

use crate::ui::{data_navigation, primary_sidebar::canvas_filter};

pub(crate) fn active_canvas_is_placeholder(app: &PlotiumApp) -> bool {
    app.session
        .active_canvas
        .is_some_and(|ci| !super::board_frame_display(app, FrameRef::Page(ci)).shows_content())
}

pub(crate) fn refresh_spotlight(app: &mut PlotiumApp, ctx: &Context) {
    if app.session.ui.board_spotlight.is_none() {
        return;
    }
    if !board_filter_active(app) {
        app.session.ui.board_spotlight = None;
        return;
    }
    let matches = canvas_filter::matching_canvases(app, &data_navigation::index(app, ctx));
    let hidden = |ci: usize| {
        app.doc
            .canvases
            .get(ci)
            .is_some_and(|canvas| !matches.contains(&canvas.resource_id))
    };
    let cancels_gesture = match app.interaction() {
        Interaction::Frame(drag) => drag
            .before
            .iter()
            .any(|(id, _)| matches!(id, BoardFrameId::Page(id) if !matches.contains(id))),
        Interaction::Phase(_) => app.session.active_canvas.is_some_and(hidden),
        gesture => gesture.canvas().is_some_and(hidden),
    };
    let active_hidden = app.session.active_canvas.is_some_and(hidden);
    if cancels_gesture {
        app.cancel_interaction();
    }
    if active_hidden {
        super::cancel_volume_drag(app, ctx);
    }
    app.session.ui.board_spotlight = Some(matches);
}

pub(crate) fn toggle_spotlight(app: &mut PlotiumApp, ctx: &Context) {
    if app.session.ui.board_spotlight.take().is_none() {
        enable_spotlight(app, ctx);
    }
    ctx.request_repaint();
}

fn enable_spotlight(app: &mut PlotiumApp, ctx: &Context) {
    app.session.ui.board_spotlight.get_or_insert_default();
    refresh_spotlight(app, ctx);
    app.session.view = PrimaryView::Canvas;
    if app.session.active_canvas.is_none() && !app.doc.canvases.is_empty() {
        let first = (0..app.doc.canvases.len())
            .find(|&ci| super::board_frame_display(app, FrameRef::Page(ci)).shows_content());
        app.activate_canvas(first.unwrap_or(0));
    }
}

pub(crate) fn show_matches(app: &mut PlotiumApp, ctx: &Context) {
    enable_spotlight(app, ctx);
    let target = app.session.ui.board_spotlight.as_ref().and_then(|matches| {
        captured_board_fit(
            app,
            app.doc
                .canvases
                .iter()
                .filter(|canvas| matches.contains(&canvas.resource_id))
                .map(|canvas| BoardFitItem::Frame(BoardFrameId::Page(canvas.resource_id))),
        )
    });
    if let Some(target) = target {
        super::request_board_fit_target(app, ctx, target);
    } else {
        app.session.status = "No canvases match the current filters.".into();
    }
    ctx.request_repaint();
}
