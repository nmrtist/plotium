use super::*;

#[test]
fn discovered_source_failure_does_not_discard_successful_files() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/jcamp-ppm.dx");
    app.queue_user_data_import_discovery(fixture.clone(), move || {
        Ok(ImportDiscovery {
            paths: vec![fixture],
            failures: vec![("unreadable-folder".into(), "Permission denied".into())],
        })
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.poll_data_import() {
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    let batch = app.session.data_imports.batches().back().unwrap();
    assert_eq!(
        (batch.loaded(), batch.failed(), batch.total),
        (1, 1, Some(2))
    );
    assert_eq!(
        batch.items[0].path,
        std::path::Path::new("unreadable-folder")
    );
    assert!(!batch.retryable);
    assert!(matches!(
        app.take_import_open_target(),
        Some(ImportOpenTarget::Dataset(_))
    ));
}
use crate::state::XrdDataset;

fn prepared() -> PreparedImport {
    PreparedImport {
        dataset: Dataset::Xrd(Box::new(XrdDataset::load(plotium_io::XrdData {
            two_theta_deg: vec![1.0, 2.0, 3.0],
            intensity: vec![2.0, 4.0, 2.0],
            attenuation: None,
            source: "sample.raw".into(),
            instrument: None,
            target: None,
            wavelength_angstrom: None,
            voltage_kv: None,
            current_ma: None,
            scan_step_deg: None,
            scan_speed_deg_min: None,
        }))),
        source: "sample.raw".into(),
        format: plotium_io::DataFormat::Xrd(plotium_io::XrdFormat::RigakuRaw),
        warnings: Vec::new(),
    }
}

fn app_with_channel() -> (PlotiumApp, mpsc::Sender<Event>) {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let (sender, receiver) = mpsc::channel();
    let id = app
        .session
        .data_imports
        .add_batch("batch".into(), ImportPhase::Scanning, true);
    app.session.data_imports.active = Some(Job { receiver, id });
    (app, sender)
}

#[test]
fn commits_only_one_item_per_poll_and_preserves_selection_and_undo() {
    let (mut app, sender) = app_with_channel();
    assert!(app.install_prepared_import(std::path::Path::new("existing"), Ok(prepared())));
    let selected = app.session.ui.data_selection.clone();
    let active = app.session.active_canvas;
    for _ in 0..2 {
        sender
            .send(Event::Item("next".into(), Ok(prepared())))
            .unwrap();
    }
    assert!(app.poll_data_import());
    assert_eq!(app.doc.datasets.len(), 2);
    assert_eq!(app.session.active_canvas, active);
    assert_eq!(app.session.ui.data_selection, selected);
    assert!(app.poll_data_import());
    assert_eq!(app.doc.datasets.len(), 3);
    app.undo();
    assert_eq!(app.doc.datasets.len(), 2);
    app.redo();
    assert_eq!(app.doc.datasets.len(), 3);
}

#[test]
fn failed_item_is_reported_and_does_not_prevent_later_success() {
    let (mut app, sender) = app_with_channel();
    sender
        .send(Event::Item("bad".into(), Err("broken source".into())))
        .unwrap();
    sender
        .send(Event::Item("good".into(), Ok(prepared())))
        .unwrap();
    app.poll_data_import();
    assert!(app.doc.datasets.is_empty());
    assert_eq!(
        app.session
            .operation_history
            .operations()
            .next_back()
            .unwrap()
            .outcome,
        crate::operation::OperationOutcome::Failure
    );
    app.poll_data_import();
    assert_eq!(app.doc.datasets.len(), 1);
    assert!(app.session.status.contains("1 loaded, 1 failed"));
}

#[test]
fn disconnected_worker_reaches_user_feedback() {
    let (mut app, sender) = app_with_channel();
    drop(sender);
    assert!(!app.poll_data_import());
    assert!(app.session.status.contains("stopped unexpectedly"));
    assert_eq!(app.session.operation_history.operations().count(), 1);
}

#[test]
fn document_swap_discards_ready_results_and_queued_requests() {
    let (mut app, sender) = app_with_channel();
    sender
        .send(Event::Item("old".into(), Ok(prepared())))
        .unwrap();
    app.queue_data_import("old queued".into(), || {
        panic!("old request must be dropped")
    });
    app.close_project();

    assert!(sender.send(Event::Finished).is_err());
    assert!(!app.poll_data_import());
    assert!(app.doc.datasets.is_empty());
}

#[test]
fn discovery_runs_off_thread_and_poll_does_not_wait_for_it() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let main_thread = std::thread::current().id();
    let (entered, started) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    app.queue_data_import("batch".into(), move || {
        entered.send(std::thread::current().id()).unwrap();
        wait.recv().unwrap();
        Ok(vec![])
    });
    assert!(app.poll_data_import());
    assert_ne!(
        started
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap(),
        main_thread
    );
    assert!(app.poll_data_import());
    app.start_new_project();
    release.send(()).unwrap();
    assert!(!app.poll_data_import());
}

#[test]
fn preparation_is_parallel_bounded_ordered_and_keeps_item_errors() {
    use std::sync::{
        Arc, Barrier,
        atomic::{AtomicUsize, Ordering},
    };
    let (sender, receiver) = mpsc::sync_channel(1);
    let barrier = Arc::new(Barrier::new(2));
    let running = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let peak_worker = peak.clone();
    let worker = std::thread::spawn(move || {
        let paths = (0..4)
            .map(|i| PathBuf::from(i.to_string()))
            .collect::<Vec<_>>();
        prepare_paths(&paths, &sender, 2, paths.len(), &|path| {
            let active = running.fetch_add(1, Ordering::SeqCst) + 1;
            peak_worker.fetch_max(active, Ordering::SeqCst);
            barrier.wait();
            running.fetch_sub(1, Ordering::SeqCst);
            if path == std::path::Path::new("1") {
                Err("bad item".into())
            } else {
                Ok(prepared())
            }
        })
    });
    let mut items = Vec::new();
    while let Ok(event) = receiver.recv_timeout(std::time::Duration::from_secs(10)) {
        if let Event::Item(path, result) = event {
            items.push((path, result.is_ok()));
        }
    }
    assert!(worker.join().unwrap());
    assert_eq!(peak.load(Ordering::SeqCst), 2);
    assert_eq!(
        items,
        vec![
            ("0".into(), true),
            ("1".into(), false),
            ("2".into(), true),
            ("3".into(), true)
        ]
    );
}

#[test]
fn disconnected_import_does_not_prepare_another_window() {
    let (sender, receiver) = mpsc::sync_channel(1);
    drop(receiver);
    assert!(!prepare_paths(&["unused".into()], &sender, 2, 1, &|_| {
        panic!("closed document must not start preparation")
    }));
}

fn finish_one(app: &mut PlotiumApp, sender: &mpsc::Sender<Event>) {
    sender
        .send(Event::Item("good".into(), Ok(prepared())))
        .unwrap();
    sender.send(Event::Finished).unwrap();
    assert!(app.poll_data_import());
    assert!(!app.poll_data_import());
}

#[test]
fn user_import_opens_only_after_completion_and_uses_stable_result_identity() {
    let (mut app, sender) = app_with_channel();
    let id = app.session.data_imports.active.as_ref().unwrap().id;
    app.session.data_imports.open_intent = Some(id);
    sender
        .send(Event::Item("good".into(), Ok(prepared())))
        .unwrap();
    assert!(app.poll_data_import());
    assert!(app.take_import_open_target().is_none());
    assert!(app.session.active_canvas.is_none());
    sender.send(Event::Finished).unwrap();
    assert!(!app.poll_data_import());
    let Some(ImportOpenTarget::Dataset(target)) = app.take_import_open_target() else {
        panic!("single imported result should be opened");
    };
    assert_eq!(app.doc.dataset_index(target.dataset), Some(0));
    assert_eq!(target.canvas, Some(app.doc.canvases[0].resource_id));
    assert!(app.take_import_open_target().is_none());
}

#[test]
fn canceled_open_intent_stays_canceled_after_leaving_and_returning() {
    let (mut app, sender) = app_with_channel();
    let id = app.session.data_imports.active.as_ref().unwrap().id;
    app.session.data_imports.open_intent = Some(id);
    app.session.view = super::super::PrimaryView::Data;
    app.cancel_import_open_intent();
    app.session.view = super::super::PrimaryView::Canvas;
    finish_one(&mut app, &sender);
    assert!(app.take_import_open_target().is_none());
    assert!(app.session.active_canvas.is_none());
}

#[test]
fn background_import_preserves_camera_frame_selection_and_edit_drafts() {
    let (mut app, sender) = app_with_channel();
    assert!(app.install_prepared_import(std::path::Path::new("existing"), Ok(prepared())));
    let frame = super::super::BoardFrameId::Page(app.doc.canvases[0].resource_id);
    app.session.ui.frame_selection = vec![frame];
    app.session.board_reveal = Some(frame);
    app.session.viewport_mode = super::super::ViewportMode::Manual;
    app.session.ui.text_edit = Some(super::super::TextEditState {
        canvas: 0,
        object: app.doc.canvases[0].objects[0].id,
        buffer: "uncommitted draft".into(),
        focus: true,
    });
    app.session.board.world_center = [73.0, -19.0];
    app.session.primary_sidebar_visible = false;
    app.session.secondary_sidebar_visible = false;
    app.session.ui.sheet_open = Some(0);
    app.session.ui.data_browser_filter = "still typing".into();
    let board = app.session.board;
    finish_one(&mut app, &sender);
    assert_eq!(app.session.board.world_center, board.world_center);
    assert!(!app.session.primary_sidebar_visible);
    assert!(!app.session.secondary_sidebar_visible);
    assert_eq!(app.session.ui.sheet_open, Some(0));
    assert_eq!(app.session.ui.data_browser_filter, "still typing");
    assert_eq!(app.session.ui.frame_selection, vec![frame]);
    assert_eq!(app.session.board_reveal, Some(frame));
    assert_eq!(
        app.session.viewport_mode,
        super::super::ViewportMode::Manual
    );
    assert_eq!(
        app.session.ui.text_edit.as_ref().unwrap().buffer,
        "uncommitted draft"
    );
    assert!(app.take_import_open_target().is_none());
}

#[test]
fn batch_retains_errors_and_retry_contains_only_failed_paths() {
    let (mut app, sender) = app_with_channel();
    let id = app.session.data_imports.active.as_ref().unwrap().id;
    app.session.data_imports.open_intent = Some(id);
    sender.send(Event::Started("bad".into(), 2)).unwrap();
    sender
        .send(Event::Item("bad".into(), Err("broken source".into())))
        .unwrap();
    sender
        .send(Event::Item("good".into(), Ok(prepared())))
        .unwrap();
    sender.send(Event::Finished).unwrap();
    for _ in 0..4 {
        app.poll_data_import();
    }
    let Some(ImportOpenTarget::Dataset(target)) = app.take_import_open_target() else {
        panic!("the successful result should open without an error dialog");
    };
    assert_eq!(target.dataset, app.doc.datasets[0].resource_id());
    let batch = app.session.data_imports.batches().back().unwrap();
    assert_eq!(
        (batch.loaded(), batch.failed(), batch.total),
        (1, 1, Some(2))
    );
    assert!(
        batch.items[0]
            .result
            .as_ref()
            .unwrap_err()
            .contains("broken source")
    );
    assert!(app.retry_failed_import(id).is_some());
    let request = app.session.data_imports.pending.pop_front().unwrap();
    assert_eq!(
        (request.discover)().unwrap().paths,
        vec![PathBuf::from("bad")]
    );
}

#[test]
fn first_batch_opens_all_successes_together_and_all_failure_never_navigates() {
    for successes in [0, 2] {
        let (mut app, sender) = app_with_channel();
        let id = app.session.data_imports.active.as_ref().unwrap().id;
        app.session.data_imports.open_intent = Some(id);
        sender
            .send(Event::Item("bad".into(), Err("broken source".into())))
            .unwrap();
        for _ in 0..successes {
            sender
                .send(Event::Item("good".into(), Ok(prepared())))
                .unwrap();
        }
        sender.send(Event::Finished).unwrap();
        while app.poll_data_import() {}
        if successes == 0 {
            assert!(app.take_import_open_target().is_none());
        } else {
            let Some(ImportOpenTarget::Workspace(targets)) = app.take_import_open_target() else {
                panic!("successful batch should open its workspace");
            };
            assert_eq!(targets.len(), 2);
            for (target, dataset) in targets.iter().zip(&app.doc.datasets) {
                assert_eq!(target.dataset, dataset.resource_id());
                assert!(target.canvas.is_some());
            }
        }
    }
}

#[test]
fn new_user_import_cancels_previous_continuation_and_existing_content_never_opens() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let first = app.queue_user_data_import("first".into(), || Ok(Vec::new()));
    let second = app.queue_user_data_import("second".into(), || Ok(Vec::new()));
    assert_ne!(first, second);
    assert_eq!(app.session.data_imports.open_intent, Some(second));
    app.install_prepared_import(std::path::Path::new("existing"), Ok(prepared()));
    app.queue_user_data_import("append".into(), || Ok(Vec::new()));
    assert!(app.session.data_imports.open_intent.is_none());
}

#[test]
fn background_append_preserves_dynamic_fit_to_all_intent() {
    let (mut app, sender) = app_with_channel();
    assert!(app.install_prepared_import(std::path::Path::new("existing"), Ok(prepared())));
    app.session.viewport_mode =
        super::super::ViewportMode::Fit(super::super::BoardFitTarget::AllFrames);
    app.session.board.world_center = [100.0, -35.0];
    let board = app.session.board;
    finish_one(&mut app, &sender);
    assert_eq!(app.session.board, board);
    assert_eq!(
        app.session.viewport_mode,
        super::super::ViewportMode::Fit(super::super::BoardFitTarget::AllFrames)
    );
}
