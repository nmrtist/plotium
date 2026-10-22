use super::*;

fn overlapping_plots() -> (PlotiumApp, [PanelId; 2], Vec<ObjectId>) {
    let (mut app, ids) = crate::ui::properties::fixture::contour_page(2);
    let page = &mut app.doc.canvases[0];
    // Panel order deliberately differs from the document's paint order.
    let top = page.create_panel("Top".into(), ObjectFrame::new(20.0, 20.0, 100.0, 60.0));
    let back = page.create_panel("Back".into(), ObjectFrame::new(300.0, 20.0, 100.0, 60.0));
    page.panel_mut(top).unwrap().item_order.push(ids[1]);
    page.panel_mut(back).unwrap().item_order.push(ids[0]);
    page.object_mut(ids[1]).unwrap().frame = ObjectFrame::new(280.0, 0.0, 100.0, 60.0);
    page.object_mut(ids[0]).unwrap().frame = ObjectFrame::new(0.0, 0.0, 100.0, 60.0);
    app.exit_panel_scope();
    (app, [back, top], ids)
}

#[test]
fn visible_content_wins_over_panel_order_and_vacated_panel_frames() {
    let (mut app, [back, top], ids) = overlapping_plots();
    let point = Pos2::new(350.0, 50.0);
    assert_eq!(
        hit_panel(&app.doc.canvases[0], point, 1.0).unwrap().panel,
        top
    );
    assert_eq!(
        hit_object(&app.doc.canvases[0], point, 1.0).unwrap().object,
        ids[1]
    );
    // Even an empty frame or its resize handle cannot steal a visible plot.
    app.doc.canvases[0].object_mut(ids[0]).unwrap().frame.x = 500.0;
    assert_eq!(
        hit_panel(&app.doc.canvases[0], point, 1.0).unwrap().panel,
        top
    );
    assert_eq!(
        hit_panel(&app.doc.canvases[0], Pos2::new(300.0, 20.0), 1.0)
            .unwrap()
            .panel,
        top
    );
    // Changing actual paint order changes which graph receives the hit.
    app.doc.canvases[0].object_mut(ids[0]).unwrap().frame.x = 0.0;
    app.doc.canvases[0].objects.reverse();
    assert_eq!(
        hit_panel(&app.doc.canvases[0], point, 1.0).unwrap().panel,
        back
    );
    app.doc.canvases[0].object_mut(ids[0]).unwrap().visible = false;
    assert_eq!(
        hit_panel(&app.doc.canvases[0], point, 1.0).unwrap().panel,
        top
    );
}

#[test]
fn loose_foreground_content_blocks_panels_and_cross_canvas_hits_follow_paint_order() {
    let (mut app, [back, top], ids) = overlapping_plots();
    let point = Pos2::new(350.0, 50.0);
    app.doc.canvases[0]
        .panel_mut(top)
        .unwrap()
        .item_order
        .clear();
    app.doc.canvases[0].object_mut(ids[1]).unwrap().frame.x = 300.0;
    // Neither semantic Panel frame covers this point. The regression was the
    // background Panel's new overflow target intercepting loose foreground content.
    app.doc.canvases[0].panel_mut(back).unwrap().frame = ObjectFrame::new(20.0, 200.0, 100.0, 60.0);
    app.doc.canvases[0].object_mut(ids[0]).unwrap().frame =
        ObjectFrame::new(280.0, -180.0, 100.0, 60.0);
    assert!(hit_panel(&app.doc.canvases[0], point, 1.0).is_none());
    assert_eq!(
        hit_object(&app.doc.canvases[0], point, 1.0).unwrap().object,
        ids[1]
    );
    app.doc.canvases[0]
        .panel_mut(top)
        .unwrap()
        .item_order
        .push(ids[1]);
    app.doc.canvases[0].object_mut(ids[1]).unwrap().frame.x = 280.0;
    app.doc.canvases[0].panel_mut(back).unwrap().frame = ObjectFrame::new(300.0, 20.0, 100.0, 60.0);
    app.doc.canvases[0].object_mut(ids[0]).unwrap().frame = ObjectFrame::new(0.0, 0.0, 100.0, 60.0);
    // The active canvas paints last. Its overflow must beat another page's plot.
    let mut other = app.doc.canvases[0].clone();
    other.resource_id = plotium_core::state::CanvasId::new();
    app.doc.canvases[0].object_mut(ids[0]).unwrap().visible = false;
    other.panel_mut(back).unwrap().frame.x = 300.0;
    app.doc.canvases.push(other);
    app.session.active_canvas = Some(0);
    let screen = EguiRect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 800.0));
    let transform = BoardTransform::from_board(app.session.board, screen);
    let pointer =
        transform.page_screen_rect(&app.doc.canvases[0]).min + point.to_vec2() * transform.zoom;
    assert_eq!(object_at_screen(&app, screen, pointer).unwrap().0, 0);
}

#[test]
fn dragging_an_overlap_moves_the_visible_plot_in_page_and_content_scopes() {
    for scope in ["page", "background", "foreground"] {
        let (mut app, [back, top], ids) = overlapping_plots();
        if scope == "background" {
            app.enter_panel(0, back);
        } else if scope == "foreground" {
            app.enter_panel(0, top);
        }
        let back_frame = app.doc.canvases[0].panel(back).unwrap().frame;
        let back_content = app.doc.canvases[0].object(ids[0]).unwrap().frame;
        let top_before = app.doc.canvases[0].content_page_frame(ids[1]).unwrap();
        let screen = EguiRect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0));
        let transform = BoardTransform::from_board(app.session.board, screen);
        let origin = transform.page_screen_rect(&app.doc.canvases[0]).min;
        let start = origin + Vec2::new(350.0, 50.0) * transform.zoom;
        let end = start + Vec2::new(140.0, 100.0) * transform.zoom;
        let ctx = egui::Context::default();
        for (time, pointer, button) in [
            (1.0, start, Some(true)),
            (1.05, end, None),
            (1.1, end, Some(false)),
        ] {
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
            if button == Some(true) {
                if scope == "foreground" {
                    assert!(
                        matches!(app.interaction(), Interaction::Object(drag) if drag.object == ids[1])
                    );
                } else {
                    assert!(
                        matches!(app.interaction(), Interaction::Panel(drag) if drag.panel == top)
                    );
                }
            }
        }
        assert_eq!(app.doc.canvases[0].panel(back).unwrap().frame, back_frame);
        assert_eq!(
            app.doc.canvases[0].object(ids[0]).unwrap().frame,
            back_content
        );
        assert!(app.doc.canvases[0].content_page_frame(ids[1]).unwrap().x > top_before.x + 100.0);
        app.undo();
        assert_eq!(
            app.doc.canvases[0].content_page_frame(ids[1]).unwrap(),
            top_before
        );
    }
}
