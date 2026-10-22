use super::*;
use plotium_core::state::{CanvasDocument, ImportItem};

fn app() -> PlotiumApp {
    PlotiumApp::new_with_settings(Default::default())
}

fn finish_import(app: &mut PlotiumApp) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.poll_data_import() {
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
}

fn queue_fixture(app: &mut PlotiumApp) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/jcamp-ppm.dx");
    app.queue_user_data_import(path.clone(), move || Ok(vec![path]));
}

#[test]
fn first_native_import_opens_its_result_while_hovering() {
    let mut app = app();
    queue_fixture(&mut app);
    cancel_on_input(
        &mut app,
        &egui::RawInput {
            events: vec![egui::Event::PointerMoved(egui::pos2(10.0, 10.0))],
            ..Default::default()
        },
    );
    finish_import(&mut app);
    assert!(app.session.active_canvas.is_none());
    let ctx = crate::typography::test_context();
    let _ = ctx.run_ui(Default::default(), |ui| apply_open_target(&mut app, ui));
    assert_eq!(app.session.active_canvas, Some(0));
    assert_eq!(
        app.session.ui.frame_selection,
        vec![plotium_core::state::BoardFrameId::Page(
            app.doc.canvases[0].resource_id
        )]
    );
}

#[test]
fn first_native_batch_shows_all_spectra_together_without_results_window() {
    let mut app = app();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/jcamp-ppm.dx");
    app.queue_user_data_import(path.clone(), move || Ok(vec![path.clone(), path]));
    finish_import(&mut app);
    assert_eq!(app.doc.canvases.len(), 2);
    assert!(app.session.active_canvas.is_none());
    let ctx = crate::typography::test_context();
    let _ = ctx.run_ui(Default::default(), |ui| apply_open_target(&mut app, ui));
    assert_eq!(app.session.active_canvas, Some(0));
    assert_eq!(app.session.ui.data_selection, vec![0, 1]);
    assert!(app.session.ui.frame_selection.is_empty());
    assert!(app.session.board_reveal.is_none());
    assert_eq!(
        app.session.viewport_mode,
        plotium_core::state::ViewportMode::Fit(plotium_core::state::BoardFitTarget::AllFrames)
    );
    assert!(
        !ctx.data_mut(|data| data
            .get_temp::<ResultsState>(window_id(&app))
            .unwrap_or_default())
            .open
    );
    let first = app.doc.canvases[0].board_pos;
    let second = app.doc.canvases[1].board_pos;
    assert_ne!(first, second);
}

#[test]
fn another_foreground_command_cancels_opening_and_keeps_new_selection() {
    let mut app = app();
    queue_fixture(&mut app);
    let ctx = crate::typography::test_context();
    commands::execute_without_clipboard(CommandId::NewTable, &mut app, &ctx);
    let selected = app.session.ui.data_selection.clone();
    let sheet = app.session.ui.sheet_open;
    finish_import(&mut app);
    let _ = ctx.run_ui(Default::default(), |ui| apply_open_target(&mut app, ui));
    assert_eq!(app.doc.datasets.len(), 2);
    assert_eq!(app.session.ui.data_selection, selected);
    assert_eq!(app.session.ui.sheet_open, sheet);
    assert!(app.session.active_canvas.is_none());
}

#[test]
fn results_command_is_discoverable_and_requires_an_import() {
    let mut app = app();
    assert!(!commands::describe(&app, CommandId::ImportResults).enabled);
    assert!(
        commands::catalog(&app)
            .iter()
            .any(|c| c.id == CommandId::ImportResults)
    );
    app.record_completed_user_import("empty".into(), Vec::new());
    assert!(commands::describe(&app, CommandId::ImportResults).enabled);
    assert_eq!(
        summary(app.session.data_imports.batches().back().unwrap()),
        "No data files were found."
    );
}

#[test]
fn result_links_resolve_identity_and_do_not_retarget_deleted_data() {
    let mut app = app();
    app.new_table_dataset();
    let target = ImportTarget {
        dataset: app.doc.datasets[0].resource_id(),
        canvas: None,
    };
    app.session.ui.sheet_open = None;
    let ctx = crate::typography::test_context();
    let _ = ctx.run_ui(Default::default(), |ui| {
        assert!(open_target(&mut app, ui, &target))
    });
    assert_eq!(app.session.ui.sheet_open, Some(0));
    app.doc.datasets.clear();
    app.new_table_dataset();
    let replacement = app.doc.datasets[0].resource_id();
    assert_ne!(replacement, target.dataset);
    let _ = ctx.run_ui(Default::default(), |ui| {
        assert!(!open_target(&mut app, ui, &target))
    });
}

#[test]
fn table_result_opens_sheet_even_when_a_canvas_is_available() {
    let mut app = app();
    app.new_table_dataset();
    app.doc
        .canvases
        .push(CanvasDocument::new("result".into(), [180.0, 120.0]));
    let canvas = app.doc.canvases[0].resource_id;
    let target = ImportTarget {
        dataset: app.doc.datasets[0].resource_id(),
        canvas: Some(canvas),
    };
    let ctx = crate::typography::test_context();
    let _ = ctx.run_ui(Default::default(), |ui| {
        open_target(&mut app, ui, &target);
    });
    assert_eq!(
        app.session.ui.frame_selection,
        vec![plotium_core::state::BoardFrameId::Sheet(target.dataset)]
    );
    assert_eq!(app.session.ui.sheet_open, Some(0));
}

#[test]
fn result_window_state_is_scoped_to_the_project() {
    let mut app = app();
    let batch = app.record_completed_user_import(
        "sample".into(),
        vec![ImportItem {
            path: "sample".into(),
            result: Err("Invalid data".into()),
        }],
    );
    let ctx = crate::typography::test_context();
    open_results(&app, &ctx, Some(batch));
    assert!(
        ctx.data_mut(|data| data.get_temp::<ResultsState>(window_id(&app)).unwrap())
            .open
    );
    app.start_new_project();
    assert!(
        ctx.data_mut(|data| data.get_temp::<ResultsState>(window_id(&app)))
            .is_none()
    );
}

#[test]
fn user_input_cancels_queued_open_intent_including_return_navigation() {
    let mut app = app();
    app.queue_user_data_import("empty".into(), || Ok(Vec::new()));
    let mut input = egui::RawInput::default();
    input
        .events
        .push(egui::Event::PointerMoved(egui::pos2(10.0, 10.0)));
    cancel_on_input(&mut app, &input);
    // A pointer press is intent even if no selection or document value changes.
    input.events = vec![egui::Event::PointerButton {
        pos: egui::pos2(10.0, 10.0),
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Default::default(),
    }];
    cancel_on_input(&mut app, &input);
    finish_import(&mut app);
    assert!(app.take_import_open_target().is_none());
}
