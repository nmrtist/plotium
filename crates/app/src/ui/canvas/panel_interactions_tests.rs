use super::*;

fn panel_page() -> (PlotiumApp, PanelId, Vec<ObjectId>) {
    let (mut app, ids) = crate::ui::properties::fixture::contour_page(2);
    let page = &mut app.doc.canvases[0];
    let panel = page.create_panel(
        "Panel".to_owned(),
        ObjectFrame::new(20.0, 20.0, 100.0, 60.0),
    );
    page.panel_mut(panel).unwrap().item_order = ids.clone();
    for &id in &ids {
        page.object_mut(id).unwrap().frame = ObjectFrame::new(0.0, 0.0, 80.0, 40.0);
    }
    (app, panel, ids)
}

#[test]
fn clipped_content_cannot_be_committed_out_of_reach() {
    for kind in [
        ObjectDragKind::Move,
        ObjectDragKind::Resize(ResizeHandle::TopLeft),
    ] {
        for target in [false, true] {
            let (mut app, panel, ids) = panel_page();
            app.doc.canvases[0].panel_mut(panel).unwrap().clip_children = true;
            let before = app.doc.canvases[0].object(ids[1]).unwrap().frame;
            app.doc.canvases[0].object_mut(ids[1]).unwrap().frame =
                ObjectFrame::new(400.0, -150.0, 80.0, 40.0);
            finish_content_drag(
                &mut app,
                0,
                ObjectDrag {
                    canvas: 0,
                    object: ids[1],
                    kind,
                    before,
                    start_pointer: [30.0, 30.0],
                    start_pointer_screen: [30.0, 30.0],
                    others: vec![],
                    active: true,
                    space: ObjectDragSpace::Panel(panel),
                },
                target.then_some(panel),
            );
            let after = ObjectFrame::new(20.0, 0.0, 80.0, 40.0);
            assert_eq!(app.doc.canvases[0].object(ids[1]).unwrap().frame, after);
            assert_eq!(
                hit_content_object(
                    &app.doc.canvases[0],
                    Pos2::new(80.0, 40.0),
                    1.0,
                    Some(panel)
                )
                .unwrap()
                .object,
                ids[1]
            );
            app.undo();
            assert_eq!(app.doc.canvases[0].object(ids[1]).unwrap().frame, before);
            app.redo();
            assert_eq!(app.doc.canvases[0].object(ids[1]).unwrap().frame, after);
        }
    }
}

#[test]
fn clipped_group_preserves_spacing_when_it_fits_and_keeps_oversized_members_reachable() {
    for oversized in [false, true] {
        let (mut app, panel, ids) = panel_page();
        let page = &mut app.doc.canvases[0];
        page.panel_mut(panel).unwrap().clip_children = true;
        page.object_mut(ids[0]).unwrap().frame = ObjectFrame::new(400.0, -100.0, 30.0, 30.0);
        page.object_mut(ids[1]).unwrap().frame =
            ObjectFrame::new(if oversized { 600.0 } else { 440.0 }, -100.0, 30.0, 30.0);
        let mut state = PanelState::of(page);
        contain_clipped_contents(&mut state, &ids, ObjectDragKind::Move);
        assert_eq!(
            state.objects[0].frame,
            ObjectFrame::new(if oversized { 0.0 } else { 30.0 }, 0.0, 30.0, 30.0)
        );
        assert_eq!(
            state.objects[1].frame,
            ObjectFrame::new(70.0, 0.0, 30.0, 30.0)
        );
    }
}

#[test]
fn clipped_resize_trims_at_the_panel_edge() {
    let (mut app, panel, ids) = panel_page();
    let page = &mut app.doc.canvases[0];
    page.panel_mut(panel).unwrap().clip_children = true;
    page.object_mut(ids[0]).unwrap().frame = ObjectFrame::new(-20.0, -20.0, 80.0, 70.0);
    let mut state = PanelState::of(page);
    contain_clipped_contents(
        &mut state,
        &[ids[0]],
        ObjectDragKind::Resize(ResizeHandle::TopLeft),
    );
    assert_eq!(
        state.objects[0].frame,
        ObjectFrame::new(0.0, 0.0, 60.0, 50.0)
    );
}

#[test]
fn content_move_group_move_and_resize_keep_off_panel_geometry_with_undo_redo() {
    for kind in [
        ObjectDragKind::Move,
        ObjectDragKind::Resize(ResizeHandle::TopLeft),
    ] {
        for group in [false, true] {
            if group && kind != ObjectDragKind::Move {
                continue;
            }
            let (mut app, panel, ids) = panel_page();
            let before = app.doc.canvases[0].object(ids[0]).unwrap().frame;
            let after = ObjectFrame::new(1400.0, -150.0, 180.0, 140.0);
            let other_after = ObjectFrame::new(1500.0, -120.0, 80.0, 40.0);
            app.doc.canvases[0].object_mut(ids[0]).unwrap().frame = after;
            if group {
                app.doc.canvases[0].object_mut(ids[1]).unwrap().frame = other_after;
            }
            let panel_before = app.doc.canvases[0].panel(panel).unwrap().frame;
            finish_content_drag(
                &mut app,
                0,
                ObjectDrag {
                    canvas: 0,
                    object: ids[0],
                    kind,
                    before,
                    start_pointer: [30.0, 30.0],
                    start_pointer_screen: [30.0, 30.0],
                    others: if group {
                        vec![(ids[1], before)]
                    } else {
                        vec![]
                    },
                    active: true,
                    space: ObjectDragSpace::Panel(panel),
                },
                None,
            );
            assert_eq!(app.doc.canvases[0].object(ids[0]).unwrap().frame, after);
            assert_eq!(
                app.doc.canvases[0].panel(panel).unwrap().frame,
                panel_before
            );
            app.undo();
            assert_eq!(app.doc.canvases[0].object(ids[0]).unwrap().frame, before);
            assert_eq!(app.doc.canvases[0].object(ids[1]).unwrap().frame, before);
            app.redo();
            assert_eq!(app.doc.canvases[0].object(ids[0]).unwrap().frame, after);
            if group {
                assert_eq!(
                    app.doc.canvases[0].object(ids[1]).unwrap().frame,
                    other_after
                );
            }
        }
    }
}

#[test]
fn overflow_remains_in_editor_bounds_and_hits_at_both_selection_scopes() {
    let (mut app, panel, ids) = panel_page();
    app.doc.canvases[0].object_mut(ids[0]).unwrap().frame =
        ObjectFrame::new(1400.0, -150.0, 80.0, 40.0);
    let point = Pos2::new(1440.0, -110.0);
    let screen = EguiRect::from_min_size(Pos2::ZERO, Vec2::new(2000.0, 1000.0));
    let transform = BoardTransform::from_board(app.session.board, screen);
    let page = transform.page_screen_rect(&app.doc.canvases[0]);
    let pointer = page.min + point.to_vec2() * transform.zoom;
    assert!(!page.contains(pointer));
    assert!(
        transform
            .canvas_editor_screen_rect(&app.doc.canvases[0])
            .contains(pointer)
    );
    let hit = panel_at_screen(&app, screen, pointer).unwrap().1;
    assert_eq!(hit.panel, panel);
    assert_eq!(hit.kind, ObjectDragKind::Move);
    assert_eq!(
        hit_content_object(&app.doc.canvases[0], point, transform.zoom, Some(panel))
            .unwrap()
            .object,
        ids[0]
    );
    // A child's corner is not a resize handle for its parent.
    assert_eq!(
        hit_panel(
            &app.doc.canvases[0],
            Pos2::new(1420.0, -130.0),
            transform.zoom
        )
        .unwrap()
        .kind,
        ObjectDragKind::Move
    );
    // Empty space between the Panel and the displaced content stays empty.
    assert!(
        hit_panel(
            &app.doc.canvases[0],
            Pos2::new(700.0, -50.0),
            transform.zoom
        )
        .is_none()
    );

    app.doc.canvases[0].panel_mut(panel).unwrap().clip_children = true;
    assert!(panel_at_screen(&app, screen, pointer).is_none());
    assert!(hit_content_object(&app.doc.canvases[0], point, transform.zoom, Some(panel)).is_none());
    app.doc.canvases[0].panel_mut(panel).unwrap().clip_children = false;
    app.doc.canvases[0].object_mut(ids[0]).unwrap().visible = false;
    assert!(panel_at_screen(&app, screen, pointer).is_none());
    app.doc.canvases[0].object_mut(ids[0]).unwrap().visible = true;
    app.doc.canvases[0].panel_mut(panel).unwrap().visible = false;
    assert!(panel_at_screen(&app, screen, pointer).is_none());
    assert!(hit_content_object(&app.doc.canvases[0], point, transform.zoom, Some(panel)).is_none());
}

#[test]
fn visible_overflow_can_be_grabbed_and_dragged_back_in_both_scopes() {
    for editing in [false, true] {
        let (mut app, panel, ids) = panel_page();
        app.doc.canvases[0].object_mut(ids[0]).unwrap().frame =
            ObjectFrame::new(1400.0, -150.0, 80.0, 40.0);
        app.session.board = BoardViewport {
            zoom: 1.0,
            world_center: [800.0, 0.0],
        };
        app.session.viewport_mode = ViewportMode::Manual;
        app.session.active_canvas = Some(0);
        if editing {
            app.enter_panel(0, panel);
        } else {
            app.exit_panel_scope();
        }
        let screen = EguiRect::from_min_size(Pos2::ZERO, Vec2::new(2000.0, 1000.0));
        let transform = BoardTransform::from_board(app.session.board, screen);
        let origin = transform.page_screen_rect(&app.doc.canvases[0]).min;
        let start = origin + Vec2::new(1440.0, -110.0);
        let end = start + Vec2::new(-1400.0, 150.0);
        let ctx = egui::Context::default();
        for (time, pointer, pressed) in [
            (1.0, start, Some(true)),
            (1.05, end, None),
            (1.1, end, Some(false)),
        ] {
            let mut events = vec![egui::Event::PointerMoved(pointer)];
            if let Some(pressed) = pressed {
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
                    let resp = ui.allocate_rect(screen, egui::Sense::click_and_drag());
                    handle_object_interactions(&mut app, 0, screen, ui, &resp);
                },
            );
            if pressed == Some(true) {
                if editing {
                    assert!(matches!(app.interaction(), Interaction::Object(_)));
                } else {
                    assert!(matches!(app.interaction(), Interaction::Panel(_)));
                }
            }
        }
        let frame = app.doc.canvases[0].content_page_frame(ids[0]).unwrap();
        assert!(
            frame.x.abs() < 50.0 && frame.y.abs() < 50.0,
            "editing={editing}: {frame:?}"
        );
        app.undo();
        assert_eq!(
            app.doc.canvases[0].content_page_frame(ids[0]).unwrap().x,
            1420.0
        );
    }
}
