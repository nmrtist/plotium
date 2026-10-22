use plotium_core::state::{CanvasDocument, PlotiumApp};

pub(super) fn setup(app: &mut PlotiumApp, content: bool) {
    let empty = PlotiumApp::new();
    app.doc = empty.doc;
    app.session = empty.session;
    app.session.recent_files.clear();
    if content {
        app.new_table_dataset();
        app.new_table_dataset();
        app.doc.canvases.push(CanvasDocument::new(
            "Publication figure".into(),
            [180.0, 120.0],
        ));
        app.session.ui.sheet_open = None;
        app.session.active_canvas = None;
    }
}
