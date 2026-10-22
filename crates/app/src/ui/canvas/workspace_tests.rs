use super::*;
use plotium_core::state::CanvasDocument;

fn frame(
    app: &mut PlotiumApp,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
) -> Vec<(String, egui::Rect)> {
    let output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 1000.0),
            )),
            events,
            ..Default::default()
        },
        |ui| super::super::render_central(app, ui),
    );
    fn texts(shape: egui::epaint::Shape, found: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::epaint::Shape::Text(text) => {
                found.push((text.galley.text().to_owned(), text.visual_bounding_rect()))
            }
            egui::epaint::Shape::Vec(shapes) => {
                for shape in shapes {
                    texts(shape, found);
                }
            }
            _ => {}
        }
    }
    let mut found = Vec::new();
    for clipped in output.shapes {
        texts(clipped.shape, &mut found);
    }
    found
}

fn has(texts: &[(String, egui::Rect)], expected: &str) -> bool {
    texts.iter().any(|(text, _)| text.contains(expected))
}

fn click(app: &mut PlotiumApp, ctx: &egui::Context, label: &str) {
    let texts = frame(app, ctx, Vec::new());
    let pos = texts
        .iter()
        .find(|(text, _)| text.contains(label))
        .unwrap()
        .1
        .center();
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
}

#[test]
fn empty_workspace_offers_start_actions() {
    let mut app = PlotiumApp::new();
    let texts = frame(&mut app, &crate::typography::test_context(), Vec::new());
    assert!(has(&texts, "Open data or a project to get started."));
    assert!(has(&texts, "New empty data table"));
    assert!(!has(&texts, "Workspace overview"));
}

#[test]
fn datasets_preserve_navigation_until_table_is_clicked() {
    let mut app = PlotiumApp::new();
    app.new_table_dataset();
    app.new_table_dataset();
    app.session.ui.sheet_open = None;
    app.focus_single(1);
    app.session.view = plotium_core::state::PrimaryView::Canvas;
    app.session.primary_sidebar_visible = false;
    let view = app.session.view;
    let ctx = crate::typography::test_context();
    let texts = frame(&mut app, &ctx, Vec::new());
    assert!(has(&texts, "2 datasets · 0 canvases"));
    assert!(has(&texts, "Table 1 — View table"));
    assert!(!has(&texts, "Open data or a project to get started."));
    assert_eq!(app.session.active_canvas, None);
    assert_eq!(app.session.ui.data_selection, vec![1]);
    assert!(app.session.view == view);
    assert!(!app.session.primary_sidebar_visible);
    assert_eq!(app.session.ui.sheet_open, None);
    click(&mut app, &ctx, "Table 1 — View table");
    assert_eq!(app.session.ui.sheet_open, Some(0));
    assert_eq!(app.session.ui.data_selection, vec![0]);
    assert!(app.session.primary_sidebar_visible);
    assert!(app.session.view == plotium_core::state::PrimaryView::Data);
}

#[test]
fn canvas_without_datasets_can_be_opened() {
    let mut app = PlotiumApp::new();
    app.doc
        .canvases
        .push(CanvasDocument::new("Figure A".into(), [200.0, 150.0]));
    let ctx = crate::typography::test_context();
    let texts = frame(&mut app, &ctx, Vec::new());
    assert!(has(&texts, "0 datasets · 1 canvas"));
    assert_eq!(app.session.active_canvas, None);
    click(&mut app, &ctx, "Figure A");
    assert_eq!(app.session.active_canvas, Some(0));
}

#[test]
fn opening_canvas_replaces_sheet_selection_for_frame_commands() {
    use plotium_core::state::{
        BoardFitTarget, BoardFrameId, FrameRef, SelectionScope, ViewportMode,
    };

    let mut app = PlotiumApp::new();
    app.new_table_dataset();
    app.session.ui.sheet_open = None;
    app.session.ui.selection_scope = SelectionScope::DataList;
    let sheet = BoardFrameId::Sheet(app.doc.datasets[0].resource_id());
    assert_eq!(app.session.ui.frame_selection, vec![sheet]);
    app.doc
        .canvases
        .push(CanvasDocument::new("Figure B".into(), [200.0, 150.0]));
    let canvas = app.doc.canvases[0].resource_id;
    let ctx = crate::typography::test_context();
    click(&mut app, &ctx, "Figure B");

    assert_eq!(
        app.session.ui.frame_selection,
        vec![BoardFrameId::Page(canvas)]
    );
    assert!(matches!(
        app.session.ui.selection_scope,
        SelectionScope::CanvasList
    ));
    assert_eq!(app.session.ui.selection_anchors.canvas, Some(canvas));
    assert_eq!(app.session.ui.selection_anchors.canvas_lead, Some(canvas));
    let rect = plotium_core::state::frame_board_rect(&app, FrameRef::Page(0)).unwrap();
    commands::execute_without_clipboard(CommandId::ZoomToSelection, &mut app, &ctx);
    assert_eq!(
        app.session.viewport_mode,
        ViewportMode::Fit(BoardFitTarget::Region([
            rect.left,
            rect.top,
            rect.right(),
            rect.bottom(),
        ]))
    );
}

#[test]
fn pending_import_remains_visible_alongside_existing_content() {
    for with_content in [false, true] {
        let mut app = PlotiumApp::new();
        if with_content {
            app.new_table_dataset();
            app.session.ui.sheet_open = None;
        }
        app.session.primary_sidebar_visible = false;
        app.queue_data_import("data".into(), || Ok(Vec::new()));
        let texts = frame(&mut app, &crate::typography::test_context(), Vec::new());
        assert!(has(&texts, "Importing data…"));
        assert_eq!(has(&texts, "Workspace overview"), with_content);
        assert!(!has(&texts, "Open data or a project to get started."));
        assert_eq!(app.session.active_canvas, None);
        assert!(!app.session.primary_sidebar_visible);
        assert_eq!(app.session.ui.sheet_open, None);
    }
}

#[test]
fn scroll_viewport_fills_workspace_and_only_overflows_when_content_needs_it() {
    for (height, datasets, overflows) in [
        (1000.0, 0, false),
        (1000.0, 2, false),
        (240.0, 2, true),
        (1000.0, 50, true),
    ] {
        let mut app = PlotiumApp::new();
        app.session.recent_files.clear();
        for _ in 0..datasets {
            app.new_table_dataset();
        }
        app.session.ui.sheet_open = None;
        let ctx = crate::typography::test_context();
        let mut viewport = None;
        let mut available = egui::Rect::NOTHING;
        for _ in 0..3 {
            let _ = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1100.0, height),
                    )),
                    ..Default::default()
                },
                |ui| {
                    available = ui.available_rect_before_wrap();
                    viewport = Some(render(&mut app, ui));
                },
            );
        }
        let viewport = viewport.unwrap();
        assert!((viewport.inner_rect.left() - available.left()).abs() < 1.0);
        assert!(viewport.inner_rect.right() > available.right() - 30.0);
        assert!((viewport.inner_rect.bottom() - available.bottom()).abs() < 1.0);
        assert_eq!(
            viewport.content_size.y > viewport.inner_rect.height(),
            overflows
        );
        assert_eq!(viewport.state.offset.y, 0.0);
    }
}
