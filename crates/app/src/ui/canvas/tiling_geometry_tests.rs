use super::*;

#[test]
fn content_drag_entering_tiling_preview_and_returning_preserves_its_panel() {
    for cancel in [false, true] {
        let (mut app, ids) = crate::ui::properties::fixture::contour_page(1);
        let object = ids[0];
        let panel_frame = ObjectFrame::new(80.0, 60.0, 200.0, 120.0);
        let before = ObjectFrame::new(20.0, 10.0, 100.0, 60.0);
        let panel = app.doc.canvases[0].create_panel("Source".into(), panel_frame);
        app.doc.canvases[0]
            .panel_mut(panel)
            .unwrap()
            .item_order
            .push(object);
        app.doc.canvases[0].object_mut(object).unwrap().frame = before;
        let mut target = CanvasDocument::new("Target".into(), [200.0, 200.0]);
        target.board_pos = [600.0, 0.0];
        app.doc.canvases.push(target);
        app.enter_panel(0, panel);
        app.session.board = BoardViewport {
            zoom: 1.0,
            world_center: [500.0, 300.0],
        };
        let screen = EguiRect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 800.0));
        let transform = BoardTransform::from_board(app.session.board, screen);
        let origin = transform.page_screen_rect(&app.doc.canvases[0]).min;
        let start = origin + Vec2::new(150.0, 100.0);
        let preview =
            transform.page_screen_rect(&app.doc.canvases[1]).min + Vec2::new(150.0, 100.0);
        let end = start + Vec2::new(40.0, 30.0);
        let ctx = egui::Context::default();
        let frames = [
            (1.0, start, Some(true)),
            (1.1, preview, None),
            (1.2, preview, None),
            (1.3, end, None),
            (1.4, end, Some(false)),
        ];
        for (time, pointer, button) in frames {
            let mut events = vec![egui::Event::PointerMoved(pointer)];
            if let Some(pressed) = button {
                events.push(egui::Event::PointerButton {
                    pos: pointer,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::default(),
                });
            }
            let _ = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ui| {
                    let resp = ui.allocate_rect(screen, Sense::click_and_drag());
                    handle_object_interactions(&mut app, 0, screen, ui, &resp);
                },
            );
            assert_eq!(app.doc.canvases[0].panel(panel).unwrap().frame, panel_frame);
            if pointer == preview {
                assert!(app.session.ui.tile_drop.is_some());
                assert_eq!(app.doc.canvases[0].object(object).unwrap().frame, before);
                if cancel {
                    app.cancel_interaction();
                    assert_eq!(app.doc.canvases[0].panel(panel).unwrap().frame, panel_frame);
                    assert_eq!(app.doc.canvases[0].object(object).unwrap().frame, before);
                    break;
                }
            }
        }
        if !cancel {
            let after = app.doc.canvases[0].object(object).unwrap().frame;
            assert_eq!(after, ObjectFrame::new(60.0, 40.0, 100.0, 60.0));
            app.undo();
            assert_eq!(app.doc.canvases[0].panel(panel).unwrap().frame, panel_frame);
            assert_eq!(app.doc.canvases[0].object(object).unwrap().frame, before);
        }
    }
}
