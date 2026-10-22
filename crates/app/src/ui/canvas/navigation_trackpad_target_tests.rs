use super::*;

pub(super) fn select_plot(app: &mut PlotiumApp) {
    app.session.active_canvas = Some(0);
    app.select_object(0, PLOT_ID);
}

fn command() -> egui::Modifiers {
    egui::Modifiers {
        command: true,
        ..Default::default()
    }
}

fn swipe_start(modifiers: egui::Modifiers, delta: Vec2) -> Vec<egui::Event> {
    vec![
        mouse_wheel_with_phase(
            egui::MouseWheelUnit::Point,
            Vec2::ZERO,
            egui::TouchPhase::Start,
            modifiers,
        ),
        mouse_wheel_with(egui::MouseWheelUnit::Point, delta, modifiers),
    ]
}

fn swipe_end() -> Vec<egui::Event> {
    vec![mouse_wheel_with_phase(
        egui::MouseWheelUnit::Point,
        Vec2::ZERO,
        egui::TouchPhase::End,
        egui::Modifiers::NONE,
    )]
}

#[test]
fn point_scroll_over_unselected_plot_pans_board() {
    let (mut app, _, _) = zoomed_plot_fixture();
    app.session.active_canvas = Some(0);
    let before_viewport = plot_viewport(&app);

    let (consumed, _) = run_navigation_frame(
        &mut app,
        Pos2::new(100.0, 60.0),
        Vec2::new(4.0, -3.0),
        egui::Modifiers::NONE,
    );

    assert!(consumed);
    assert_eq!(plot_viewport(&app), before_viewport);
    assert_eq!(app.session.board.world_center, [196.0, 153.0]);
}

#[test]
fn point_scroll_over_selected_plot_pans_viewport_without_moving_board() {
    let (mut app, _, _) = zoomed_plot_fixture();
    select_plot(&mut app);
    let before_viewport = plot_viewport(&app);
    let before_board = app.session.board;

    let (consumed, repaint_delay) = run_navigation_frame(
        &mut app,
        Pos2::new(100.0, 60.0),
        Vec2::new(4.0, -3.0),
        egui::Modifiers::NONE,
    );

    let after_viewport = plot_viewport(&app);
    assert!(consumed);
    assert_ne!(after_viewport.view_x, before_viewport.view_x);
    assert_ne!(after_viewport.view_y, before_viewport.view_y);
    assert!((after_viewport.view_x.span() - before_viewport.view_x.span()).abs() < 1e-12);
    assert!((after_viewport.view_y.span() - before_viewport.view_y.span()).abs() < 1e-12);
    assert_eq!(app.session.board, before_board);
    assert!(repaint_delay > std::time::Duration::ZERO);
    assert!(repaint_delay <= std::time::Duration::from_millis(200));
}

#[test]
fn point_scroll_over_selected_plot_axis_pans_board() {
    let (mut app, _, _) = zoomed_plot_fixture();
    select_plot(&mut app);
    let screen = egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0));
    let (_, outer, plot) = plot_under_cursor(&app, 0, screen, Pos2::new(100.0, 60.0)).unwrap();
    let pointer = Pos2::new(
        (outer.left() + plot.left) * 0.5,
        plot.top + plot.height * 0.5,
    );
    let before_viewport = plot_viewport(&app);

    let (consumed, _) = run_navigation_frame(
        &mut app,
        pointer,
        Vec2::new(4.0, -3.0),
        egui::Modifiers::NONE,
    );

    assert!(consumed);
    assert_eq!(plot_viewport(&app), before_viewport);
    assert_eq!(app.session.board.world_center, [196.0, 153.0]);
}

#[test]
fn command_point_scroll_over_selected_plot_zooms_board() {
    let (mut app, _, _) = zoomed_plot_fixture();
    select_plot(&mut app);
    let before_viewport = plot_viewport(&app);

    let (consumed, _) = run_navigation_frame(
        &mut app,
        Pos2::new(100.0, 60.0),
        Vec2::new(0.0, 20.0),
        command(),
    );

    assert!(consumed);
    assert_eq!(plot_viewport(&app), before_viewport);
    assert!((app.session.board.zoom - 0.1_f32.exp()).abs() < 1e-5);
}

fn assert_modified_point_scroll_keeps_board_zoom_after_modifier_release(
    modifiers: egui::Modifiers,
) {
    let (mut app, _, _) = zoomed_plot_fixture();
    select_plot(&mut app);
    let before_viewport = plot_viewport(&app);
    let ctx = egui::Context::default();
    let pointer = Pos2::new(100.0, 60.0);
    let (start_consumed, _) = run_navigation_events_on(
        &ctx,
        &mut app,
        pointer,
        modifiers,
        swipe_start(modifiers, Vec2::new(0.0, 20.0)),
    );
    let zoom_after_start = app.session.board.zoom;
    assert!(start_consumed);
    assert!(zoom_after_start > 1.0);

    let (move_consumed, _) = run_navigation_events_on(
        &ctx,
        &mut app,
        pointer,
        egui::Modifiers::NONE,
        vec![mouse_wheel_with(
            egui::MouseWheelUnit::Point,
            Vec2::new(0.0, 20.0),
            egui::Modifiers::NONE,
        )],
    );

    assert!(move_consumed);
    assert_eq!(plot_viewport(&app), before_viewport);
    assert!(app.session.board.zoom > zoom_after_start);
}

#[test]
fn command_point_scroll_keeps_board_zoom_after_modifier_release() {
    assert_modified_point_scroll_keeps_board_zoom_after_modifier_release(command());
}

#[test]
fn control_point_scroll_keeps_board_zoom_after_modifier_release() {
    assert_modified_point_scroll_keeps_board_zoom_after_modifier_release(egui::Modifiers {
        ctrl: true,
        ..Default::default()
    });
}

fn assert_board_gesture_finish_releases_target(finish_phase: egui::TouchPhase) {
    let (mut app, _, _) = zoomed_plot_fixture();
    let ctx = egui::Context::default();
    let pointer = Pos2::new(100.0, 60.0);

    let _ = run_navigation_events_on(
        &ctx,
        &mut app,
        pointer,
        command(),
        swipe_start(command(), Vec2::new(0.0, 20.0)),
    );
    let _ = run_navigation_events_on(
        &ctx,
        &mut app,
        pointer,
        egui::Modifiers::NONE,
        vec![mouse_wheel_with_phase(
            egui::MouseWheelUnit::Point,
            Vec2::ZERO,
            finish_phase,
            egui::Modifiers::NONE,
        )],
    );
    let before_viewport = plot_viewport(&app);
    let before_board = app.session.board;

    let (consumed, _) = run_navigation_events_on(
        &ctx,
        &mut app,
        pointer,
        egui::Modifiers::NONE,
        vec![mouse_wheel_with(
            egui::MouseWheelUnit::Point,
            Vec2::new(2.0, -2.0),
            egui::Modifiers::NONE,
        )],
    );

    assert!(consumed);
    assert_ne!(plot_viewport(&app), before_viewport);
    assert_eq!(app.session.board, before_board);
}

#[test]
fn command_point_scroll_releases_board_target_on_end() {
    assert_board_gesture_finish_releases_target(egui::TouchPhase::End);
}

#[test]
fn command_point_scroll_releases_board_target_on_cancel() {
    assert_board_gesture_finish_releases_target(egui::TouchPhase::Cancel);
}

#[test]
fn point_scroll_over_blank_board_pans_both_axes() {
    let (mut app, _, _) = zoomed_plot_fixture();
    let before_viewport = plot_viewport(&app);

    let (consumed, _) = run_navigation_frame(
        &mut app,
        Pos2::new(300.0, 200.0),
        Vec2::new(4.0, -3.0),
        egui::Modifiers::NONE,
    );

    assert!(consumed);
    assert_eq!(plot_viewport(&app), before_viewport);
    assert_eq!(app.session.board.world_center, [196.0, 153.0]);
}

#[test]
fn native_pinch_has_priority_over_command_point_scroll() {
    let (mut app, _, _) = zoomed_plot_fixture();

    let (consumed, _) = run_navigation_events(
        &mut app,
        Pos2::new(100.0, 60.0),
        command(),
        vec![
            mouse_wheel_with(egui::MouseWheelUnit::Point, Vec2::new(4.0, -3.0), command()),
            egui::Event::Zoom(1.25),
        ],
    );

    assert!(consumed);
    assert!((app.session.board.zoom - 1.25).abs() < f32::EPSILON);
    assert_eq!(app.session.board.world_center, [180.0, 132.0]);
}

#[test]
fn native_pinch_over_unselected_plot_zooms_board() {
    let (mut app, _, _) = zoomed_plot_fixture();
    app.session.active_canvas = Some(0);
    let before_viewport = plot_viewport(&app);

    let (consumed, _) = run_navigation_events(
        &mut app,
        Pos2::new(100.0, 60.0),
        egui::Modifiers::NONE,
        vec![egui::Event::Zoom(1.25)],
    );

    assert!(consumed);
    assert_eq!(plot_viewport(&app), before_viewport);
    assert!((app.session.board.zoom - 1.25).abs() < f32::EPSILON);
}

#[test]
fn command_native_pinch_over_selected_plot_zooms_board() {
    let (mut app, _, _) = zoomed_plot_fixture();
    select_plot(&mut app);
    let before_viewport = plot_viewport(&app);

    let (consumed, _) = run_navigation_events(
        &mut app,
        Pos2::new(100.0, 60.0),
        command(),
        vec![egui::Event::Zoom(1.25)],
    );

    assert!(consumed);
    assert_eq!(plot_viewport(&app), before_viewport);
    assert!((app.session.board.zoom - 1.25).abs() < f32::EPSILON);
}

#[test]
fn native_pinch_does_not_inherit_board_target_from_point_gesture() {
    let (mut app, _, _) = zoomed_plot_fixture();
    select_plot(&mut app);
    let ctx = egui::Context::default();
    let pointer = Pos2::new(100.0, 60.0);
    let _ = run_navigation_events_on(
        &ctx,
        &mut app,
        pointer,
        command(),
        swipe_start(command(), Vec2::new(0.0, 20.0)),
    );
    let before_viewport = plot_viewport(&app);
    let before_board = app.session.board;

    let (consumed, _) = run_navigation_events_on(
        &ctx,
        &mut app,
        pointer,
        egui::Modifiers::NONE,
        vec![egui::Event::Zoom(1.25)],
    );

    let after_viewport = plot_viewport(&app);
    assert!(consumed);
    assert!(after_viewport.view_x.span() < before_viewport.view_x.span());
    assert!(after_viewport.view_y.span() < before_viewport.view_y.span());
    assert_eq!(app.session.board, before_board);
}

#[test]
fn momentum_start_keeps_board_target_when_selected_plot_moves_under_pointer() {
    let (mut app, _, _) = zoomed_plot_fixture();
    select_plot(&mut app);
    let ctx = egui::Context::default();
    let pointer = Pos2::new(300.0, 60.0);
    let none = egui::Modifiers::NONE;
    let _ = run_navigation_events_at(
        &ctx,
        &mut app,
        pointer,
        none,
        swipe_start(none, Vec2::new(120.0, 0.0)),
        Some(1.0),
    );
    let _ = run_navigation_events_at(&ctx, &mut app, pointer, none, swipe_end(), Some(1.02));
    assert!(plot_under_cursor(&app, 0, screen(), pointer).is_some());
    let before_viewport = plot_viewport(&app);

    let _ = run_navigation_events_at(
        &ctx,
        &mut app,
        pointer,
        none,
        swipe_start(none, Vec2::new(2.0, 0.0)),
        Some(1.06),
    );

    assert_eq!(plot_viewport(&app), before_viewport);
    assert_eq!(app.session.board.world_center, [78.0, 150.0]);
}

#[test]
fn momentum_start_continues_the_plot_pan_undo_step() {
    let (mut app, _, _) = zoomed_plot_fixture();
    select_plot(&mut app);
    let ctx = egui::Context::default();
    let pointer = Pos2::new(100.0, 60.0);
    let none = egui::Modifiers::NONE;
    let before_viewport = plot_viewport(&app);
    let _ = run_navigation_events_at(
        &ctx,
        &mut app,
        pointer,
        none,
        swipe_start(none, Vec2::new(4.0, -3.0)),
        Some(1.0),
    );
    let _ = run_navigation_events_at(&ctx, &mut app, pointer, none, swipe_end(), Some(1.02));
    // Deselection proves the continuation does not re-resolve its target.
    app.set_selection(plotium_core::state::Selection::None);
    let after_finger = plot_viewport(&app);

    let _ = run_navigation_events_at(
        &ctx,
        &mut app,
        pointer,
        none,
        swipe_start(none, Vec2::new(4.0, -3.0)),
        Some(1.06),
    );

    assert_ne!(plot_viewport(&app), after_finger);
    assert_eq!(app.session.board.world_center, [200.0, 150.0]);
    let pending = app.session.ui.wheel_zoom.as_ref().unwrap();
    assert_eq!(pending.before, before_viewport);
    assert!(app.session.undo_stack.is_empty());
}

#[test]
fn late_start_after_end_retargets() {
    let (mut app, _, _) = zoomed_plot_fixture();
    select_plot(&mut app);
    let ctx = egui::Context::default();
    let pointer = Pos2::new(100.0, 60.0);
    let none = egui::Modifiers::NONE;
    let _ = run_navigation_events_at(
        &ctx,
        &mut app,
        pointer,
        none,
        swipe_start(none, Vec2::new(4.0, -3.0)),
        Some(1.0),
    );
    let _ = run_navigation_events_at(&ctx, &mut app, pointer, none, swipe_end(), Some(1.02));
    app.set_selection(plotium_core::state::Selection::None);
    let after_finger = plot_viewport(&app);
    let _ = run_navigation_events_at(&ctx, &mut app, pointer, none, Vec::new(), Some(1.3));

    let _ = run_navigation_events_at(
        &ctx,
        &mut app,
        pointer,
        none,
        swipe_start(none, Vec2::new(4.0, -3.0)),
        Some(1.5),
    );

    assert_eq!(plot_viewport(&app), after_finger);
    assert_eq!(app.session.board.world_center, [196.0, 153.0]);
}

#[test]
fn hover_hint_device_follows_the_latest_wheel_input() {
    let ctx = egui::Context::default();
    note_wheel_device(&ctx, &swipe_start(egui::Modifiers::NONE, Vec2::ZERO), true);
    assert!(last_wheel_was_trackpad(&ctx));
    note_wheel_device(&ctx, &[mouse_wheel(egui::MouseWheelUnit::Line)], false);
    assert!(!last_wheel_was_trackpad(&ctx));
    note_wheel_device(&ctx, &[egui::Event::Zoom(1.1)], false);
    assert!(last_wheel_was_trackpad(&ctx));
}

#[test]
fn momentum_start_in_the_next_pass_continues_after_a_slow_frame() {
    let (mut app, _, _) = zoomed_plot_fixture();
    select_plot(&mut app);
    let ctx = egui::Context::default();
    let pointer = Pos2::new(100.0, 60.0);
    let none = egui::Modifiers::NONE;
    let _ = run_navigation_events_at(
        &ctx,
        &mut app,
        pointer,
        none,
        swipe_start(none, Vec2::new(4.0, -3.0)),
        Some(1.0),
    );
    let _ = run_navigation_events_at(&ctx, &mut app, pointer, none, swipe_end(), Some(1.02));
    app.set_selection(plotium_core::state::Selection::None);
    let after_finger = plot_viewport(&app);

    let _ = run_navigation_events_at(
        &ctx,
        &mut app,
        pointer,
        none,
        swipe_start(none, Vec2::new(4.0, -3.0)),
        Some(1.4),
    );

    assert_ne!(plot_viewport(&app), after_finger);
    assert_eq!(app.session.board.world_center, [200.0, 150.0]);
}

#[test]
fn a_gesture_whose_end_was_not_seen_does_not_lend_its_target() {
    let (mut app, _, _) = zoomed_plot_fixture();
    select_plot(&mut app);
    let ctx = egui::Context::default();
    let inside = Pos2::new(100.0, 60.0);
    let outside = Pos2::new(500.0, 60.0);
    let none = egui::Modifiers::NONE;
    let _ = run_navigation_events_on(
        &ctx,
        &mut app,
        inside,
        none,
        swipe_start(none, Vec2::new(4.0, -3.0)),
    );
    let _ = run_navigation_events_on(&ctx, &mut app, outside, none, swipe_end());
    let _ = run_navigation_events_on(&ctx, &mut app, outside, none, swipe_start(none, Vec2::ZERO));
    app.set_selection(plotium_core::state::Selection::None);
    let before_viewport = plot_viewport(&app);

    let _ = run_navigation_events_on(
        &ctx,
        &mut app,
        inside,
        none,
        vec![mouse_wheel_with(
            egui::MouseWheelUnit::Point,
            Vec2::new(4.0, -3.0),
            none,
        )],
    );

    assert_eq!(plot_viewport(&app), before_viewport);
    assert_eq!(app.session.board.world_center, [196.0, 153.0]);
}

#[test]
fn selection_beyond_the_plot_group_leaves_gestures_on_the_board() {
    let (mut app, _, _) = zoomed_plot_fixture();
    app.session.active_canvas = Some(0);
    app.session.ui.selection =
        plotium_core::state::Selection::Objects(vec![PLOT_ID, ObjectId::new(99)]);
    let before_viewport = plot_viewport(&app);

    let (consumed, _) = run_navigation_frame(
        &mut app,
        Pos2::new(100.0, 60.0),
        Vec2::new(4.0, -3.0),
        egui::Modifiers::NONE,
    );

    assert!(consumed);
    assert_eq!(plot_viewport(&app), before_viewport);
    assert_eq!(app.session.board.world_center, [196.0, 153.0]);
}

fn screen() -> egui::Rect {
    egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0))
}
