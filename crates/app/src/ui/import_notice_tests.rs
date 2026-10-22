use super::*;
use plotium_core::operation::{
    Diagnostic, DiagnosticCode, OperationKind, OperationOutcome, OperationReport, Severity,
};
use plotium_core::state::ImportItem;

fn app() -> PlotiumApp {
    PlotiumApp::new_with_settings(Default::default())
}

fn shown(app: &PlotiumApp, ctx: &Context, time: f64) -> bool {
    let mut visible = false;
    let _ = ctx.run_ui(
        egui::RawInput {
            time: Some(time),
            ..Default::default()
        },
        |ui| {
            visible = notice(app, ui.ctx()).is_some();
        },
    );
    visible
}

#[test]
fn success_disappears_without_losing_results_and_new_import_surfaces() {
    let mut app = app();
    let ctx = crate::typography::test_context();
    app.record_completed_user_import("sample.dx".into(), Vec::new());
    assert!(shown(&app, &ctx, 0.0));
    assert!(shown(&app, &ctx, 2.9));
    assert!(!shown(&app, &ctx, 3.0));
    assert!(!shown(&app, &ctx, 10.0));
    assert!(commands::describe(&app, CommandId::ImportResults).enabled);
    app.record_completed_user_import("another.dx".into(), Vec::new());
    assert!(shown(&app, &ctx, 11.0));
}

#[test]
fn failure_remains_until_dismissed_and_does_not_reappear() {
    let mut app = app();
    let ctx = crate::typography::test_context();
    let batch = app.record_completed_user_import(
        "sample.dx".into(),
        vec![ImportItem {
            path: "sample.dx".into(),
            result: Err("Invalid data".into()),
        }],
    );
    assert!(shown(&app, &ctx, 0.0));
    assert!(shown(&app, &ctx, 30.0));
    dismiss_notice(&app, &ctx, batch);
    assert!(!shown(&app, &ctx, 31.0));
    assert!(!shown(&app, &ctx, 60.0));
    assert_eq!(
        app.session.data_imports.batches().back().unwrap().failed(),
        1
    );
}

#[test]
fn dismissed_progress_stays_hidden_but_later_failure_surfaces() {
    let mut app = app();
    let ctx = crate::typography::test_context();
    let batch = app.queue_user_data_import("broken.dx".into(), || Err("Cannot read source".into()));
    assert!(shown(&app, &ctx, 0.0));
    dismiss_notice(&app, &ctx, batch);
    assert!(!shown(&app, &ctx, 1.0));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.poll_data_import() {
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert!(shown(&app, &ctx, 2.0));
    assert!(super::super::pending_feedback(&app).is_none());
}

#[test]
fn associated_warning_uses_batch_notice_without_hiding_unrelated_failure() {
    let mut app = app();
    let related = app.session.begin_operation();
    app.session.record_operation(OperationReport::warning(
        related,
        OperationKind::TableImport,
        "Import warning",
        (),
    ));
    app.record_completed_user_import_with_operations("table.csv".into(), Vec::new(), vec![related]);
    assert!(super::super::pending_feedback(&app).is_none());
    let unrelated = app.session.begin_operation();
    app.session.record_operation(OperationReport::<()>::failure(
        unrelated,
        OperationKind::DataExport,
        "Export failed",
        Diagnostic::new(
            Severity::Error,
            DiagnosticCode::DataExportWriteFailed,
            "Cannot write",
        ),
    ));
    let (id, _, outcome, _, _) = super::super::pending_feedback(&app).unwrap();
    assert_eq!(id, unrelated);
    assert_eq!(outcome, OperationOutcome::Failure);
}
