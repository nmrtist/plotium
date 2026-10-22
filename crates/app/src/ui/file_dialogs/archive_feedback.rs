use plotium_core::operation::{DiagnosticCode, OperationOutcome};
use plotium_core::state::{ImportItem, ImportTarget, PlotiumApp};
use std::path::Path;

pub(super) fn load(app: &mut PlotiumApp, path: &Path) {
    load_with(
        app,
        path,
        PlotiumApp::load_from,
        PlotiumApp::note_recent_file,
    );
}

fn load_with(
    app: &mut PlotiumApp,
    path: &Path,
    load: impl FnOnce(&mut PlotiumApp, &Path),
    note_recent: impl FnOnce(&mut PlotiumApp, &Path),
) {
    app.cancel_import_open_intent();
    let before = app.doc.datasets.len();
    let empty = before == 0 && app.doc.canvases.is_empty();
    let previous_order = app
        .session
        .operation_history
        .operations()
        .next_back()
        .map_or(0, |report| report.completion_order);
    super::import_navigation::preserve(app, |app| load(app, path));
    let operation_ids = app
        .session
        .operation_history
        .operations()
        .filter(|report| report.completion_order > previous_order)
        .map(|report| report.id)
        .collect();
    let mut items = app.doc.datasets[before..]
        .iter()
        .map(|dataset| {
            let id = dataset.resource_id();
            let canvas = app.doc.canvases.iter().find(|canvas| {
                canvas.objects.iter().any(|object| {
                    object
                        .plot()
                        .is_some_and(|plot| plot.binding.contains_dataset(id))
                })
            });
            ImportItem {
                path: path.to_owned(),
                result: Ok(ImportTarget {
                    dataset: id,
                    canvas: canvas.map(|canvas| canvas.resource_id),
                }),
            }
        })
        .collect::<Vec<_>>();
    for report in app
        .session
        .operation_history
        .operations()
        .filter(|report| report.completion_order > previous_order)
    {
        let before_errors = items.len();
        for diagnostic in &report.diagnostics {
            if diagnostic.code == DiagnosticCode::DatasetLoadFailed {
                items.push(ImportItem {
                    path: diagnostic
                        .context
                        .get("path")
                        .map_or_else(|| path.to_owned(), std::path::PathBuf::from),
                    result: Err(diagnostic.message.clone()),
                });
            }
        }
        if report.outcome == OperationOutcome::Failure && items.len() == before_errors {
            items.push(ImportItem {
                path: path.to_owned(),
                result: Err(report.summary.clone()),
            });
        }
    }
    let loaded = items.iter().any(|item| item.result.is_ok());
    let batch =
        app.record_completed_user_import_with_operations(path.to_owned(), items, operation_ids);
    if empty {
        app.request_completed_import_open(batch);
    }
    if loaded {
        note_recent(app, path);
    }
}

#[cfg(test)]
#[path = "archive_feedback_tests.rs"]
mod tests;
