use super::*;
use plotium_core::state::{
    BoardFitTarget, BoardFrameId, ImportOpenTarget, PrimaryView, ViewportMode,
};

fn preview(app: &mut PlotiumApp, count: usize) {
    let mut candidates = Vec::new();
    for _ in 0..count {
        import_delimited_text(app, "x,y\n1,2\n3,4\n", DelimitedTableSource::Clipboard);
        candidates.append(
            &mut app
                .session
                .ui
                .table_import_preview
                .as_mut()
                .unwrap()
                .candidates,
        );
    }
    app.session
        .ui
        .table_import_preview
        .as_mut()
        .unwrap()
        .candidates = candidates;
}

fn commit(app: &mut PlotiumApp) {
    assert!(commit_table_import_preview_with_recent(app, |_, _| {
        panic!("clipboard import has no recent file")
    }));
}

#[test]
fn first_single_table_opens_its_table_and_retains_result() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    preview(&mut app, 1);
    commit(&mut app);
    assert_eq!(app.session.ui.sheet_open, Some(0));
    assert_eq!(app.session.ui.data_selection, vec![0]);
    assert!(app.session.view == PrimaryView::Data);
    let batch = app.session.data_imports.batches().back().unwrap();
    assert_eq!(batch.loaded(), 1);
    assert!(!batch.retryable);
    assert_eq!(
        batch.items[0].result.as_ref().unwrap().dataset,
        app.doc.datasets[0].resource_id()
    );
}

#[test]
fn appending_tables_preserves_navigation_drafts_and_original_undo_selection() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    preview(&mut app, 1);
    commit(&mut app);
    let frame = BoardFrameId::Sheet(app.doc.datasets[0].resource_id());
    app.session.ui.frame_selection = vec![frame];
    app.session.board_reveal = Some(frame);
    app.session.board.world_center = [61.0, -20.0];
    app.session.viewport_mode = ViewportMode::Fit(BoardFitTarget::AllFrames);
    app.session.primary_sidebar_visible = false;
    app.session.secondary_sidebar_visible = false;
    app.session.ui.data_browser_filter = "uncommitted search".into();
    let board = app.session.board;
    let canvas = app.session.active_canvas;
    preview(&mut app, 2);
    commit(&mut app);
    assert_eq!(app.doc.datasets.len(), 3);
    assert_eq!(app.session.active_canvas, canvas);
    assert_eq!(app.session.ui.sheet_open, Some(0));
    assert_eq!(app.session.ui.data_selection, vec![0]);
    assert_eq!(app.session.ui.frame_selection, vec![frame]);
    assert_eq!(app.session.board_reveal, Some(frame));
    assert_eq!(app.session.board, board);
    assert_eq!(
        app.session.viewport_mode,
        ViewportMode::Fit(BoardFitTarget::AllFrames)
    );
    assert!(!app.session.primary_sidebar_visible);
    assert!(!app.session.secondary_sidebar_visible);
    assert_eq!(app.session.ui.data_browser_filter, "uncommitted search");
    assert!(app.take_import_open_target().is_none());
    app.undo();
    app.undo();
    assert_eq!(app.doc.datasets.len(), 1);
    assert_eq!(app.session.ui.data_selection, vec![0]);
}

#[test]
fn first_multiple_tables_request_one_workspace_without_selecting_last_table() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    preview(&mut app, 2);
    commit(&mut app);
    assert_eq!(app.doc.datasets.len(), 2);
    assert!(app.session.active_canvas.is_none());
    assert!(app.session.ui.sheet_open.is_none());
    assert!(app.session.ui.data_selection.is_empty());
    let Some(ImportOpenTarget::Workspace(targets)) = app.take_import_open_target() else {
        panic!("multi-table import should open its workspace");
    };
    assert_eq!(targets.len(), 2);
    for (target, dataset) in targets.iter().zip(&app.doc.datasets) {
        assert_eq!(target.dataset, dataset.resource_id());
    }
}
