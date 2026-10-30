use plotium_core::state::PlotiumApp;

/// Import actions retain their own undo records while the user's existing
/// editing drafts and navigation remain owned by the current workspace.
pub(super) fn preserve<T>(app: &mut PlotiumApp, import: impl FnOnce(&mut PlotiumApp) -> T) -> T {
    let ui = std::mem::take(&mut app.session.ui);
    // Action snapshots must still record the real pre-import data selection.
    app.session.ui.data_selection.clone_from(&ui.data_selection);
    let active_canvas = app.session.active_canvas;
    let view = app.session.view;
    let board = app.session.board;
    let viewport_mode = app.session.viewport_mode.clone();
    let board_reveal = app.session.board_reveal;
    let tool = app.session.tool;
    let primary_sidebar_visible = app.session.primary_sidebar_visible;
    let secondary_sidebar_visible = app.session.secondary_sidebar_visible;
    let result = import(app);
    app.session.ui = ui;
    app.session.active_canvas = active_canvas;
    app.session.view = view;
    app.session.board = board;
    app.session.viewport_mode = viewport_mode;
    app.session.board_reveal = board_reveal;
    app.session.tool = tool;
    app.session.primary_sidebar_visible = primary_sidebar_visible;
    app.session.secondary_sidebar_visible = secondary_sidebar_visible;
    result
}
