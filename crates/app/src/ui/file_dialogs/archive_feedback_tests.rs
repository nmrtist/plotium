use super::*;
use plotium_core::state::{ImportOpenTarget, PrimaryView};

fn load_fixture(app: &mut PlotiumApp, count: usize) {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../io/tests/fixtures/nmr/jcamp-ppm.dx");
    for _ in 0..count {
        app.load_from(&fixture);
    }
}

#[test]
fn archive_batch_keeps_existing_workspace_and_records_each_native_result() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    load_fixture(&mut app, 1);
    let selected = app.session.ui.data_selection.clone();
    let canvas = app.session.active_canvas;
    app.session.view = PrimaryView::Data;
    app.session.ui.data_browser_filter = "editing".into();
    let mut noted = false;
    load_with(
        &mut app,
        Path::new("batch.zip"),
        |app, _| load_fixture(app, 2),
        |_, _| noted = true,
    );
    assert!(noted);
    assert_eq!(app.doc.datasets.len(), 3);
    assert_eq!(app.session.active_canvas, canvas);
    assert_eq!(app.session.ui.data_selection, selected);
    assert_eq!(app.session.ui.data_browser_filter, "editing");
    assert!(app.session.view == PrimaryView::Data);
    assert!(app.take_import_open_target().is_none());
    let batch = app.session.data_imports.batches().back().unwrap();
    assert_eq!(batch.loaded(), 2);
    assert!(!batch.retryable);
    for (item, dataset) in batch.items.iter().zip(&app.doc.datasets[1..]) {
        let target = item.result.as_ref().unwrap();
        assert_eq!(target.dataset, dataset.resource_id());
        assert!(target.canvas.is_some());
    }
}

#[test]
fn first_archive_opens_single_result_or_combined_workspace() {
    for count in [1, 2] {
        let mut app = PlotiumApp::new_with_settings(Default::default());
        load_with(
            &mut app,
            Path::new("batch.zip"),
            |app, _| load_fixture(app, count),
            |_, _| {},
        );
        assert!(app.session.active_canvas.is_none());
        match app.take_import_open_target().unwrap() {
            ImportOpenTarget::Dataset(target) => {
                assert_eq!(count, 1);
                assert_eq!(target.dataset, app.doc.datasets[0].resource_id());
            }
            ImportOpenTarget::Workspace(targets) => {
                assert_eq!(count, 2);
                assert_eq!(targets.len(), count);
            }
            ImportOpenTarget::Batch(_) => panic!("imports do not open results dialogs"),
        }
    }
}

#[test]
fn failed_archive_retains_error_without_noting_recent_path() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    load_with(
        &mut app,
        Path::new("missing.zip"),
        |app, path| app.load_from(path),
        |_, _| panic!("failed archive must not become recent"),
    );
    let batch = app.session.data_imports.batches().back().unwrap();
    assert_eq!((batch.loaded(), batch.failed()), (0, 1));
    assert!(!batch.items[0].result.as_ref().unwrap_err().is_empty());
}

#[test]
fn skipped_archive_entry_counts_as_failure_but_metadata_warning_does_not() {
    use plotium_core::operation::{Diagnostic, OperationKind, OperationReport, Severity};
    let mut app = PlotiumApp::new_with_settings(Default::default());
    load_with(
        &mut app,
        Path::new("batch.zip"),
        |app, _| {
            load_fixture(app, 1);
            let id = app.session.begin_operation();
            app.session.record_operation(
                OperationReport::warning(id, OperationKind::DatasetLoad, "Partial archive", ())
                    .with_diagnostic(
                        Diagnostic::new(
                            Severity::Error,
                            DiagnosticCode::DatasetLoadFailed,
                            "Invalid spectrum",
                        )
                        .with_context("path", "broken.dx"),
                    )
                    .with_diagnostic(Diagnostic::new(
                        Severity::Warning,
                        DiagnosticCode::DatasetLoadWarning,
                        "Optional metadata missing",
                    )),
            );
        },
        |_, _| {},
    );
    let batch = app.session.data_imports.batches().back().unwrap();
    assert_eq!(
        (batch.loaded(), batch.failed(), batch.total),
        (1, 1, Some(2))
    );
    assert_eq!(batch.items[1].path, Path::new("broken.dx"));
    assert_eq!(
        batch.items[1].result.as_ref().unwrap_err(),
        "Invalid spectrum"
    );
}
