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

pub(super) fn imports(app: &mut PlotiumApp) {
    use plotium_core::state::{ImportItem, ImportTarget};
    setup(app, false);
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/jcamp-ppm.dx");
    for _ in 0..2 {
        app.load_from(&fixture);
    }
    let items = app
        .doc
        .datasets
        .iter()
        .map(|dataset| ImportItem {
            path: fixture.clone(),
            result: Ok(ImportTarget {
                dataset: dataset.resource_id(),
                canvas: app.doc.canvases.iter().find_map(|canvas| {
                    canvas
                        .objects
                        .iter()
                        .any(|object| {
                            object.plot().is_some_and(|plot| {
                                plot.binding.contains_dataset(dataset.resource_id())
                            })
                        })
                        .then_some(canvas.resource_id)
                }),
            }),
        })
        .chain(std::iter::once(ImportItem {
            path: "incomplete.raw".into(),
            result: Err(
                "The data file is incomplete. Select a complete acquisition to retry.".into(),
            ),
        }))
        .collect();
    let batch = app.record_completed_user_import("Imported data".into(), items);
    app.session.active_canvas = None;
    app.session.ui.data_selection.clear();
    app.request_completed_import_open(batch);
}
