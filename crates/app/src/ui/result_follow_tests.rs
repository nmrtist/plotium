use super::*;
use plotium_core::automation::{
    CallerType, DocumentRevision, ExecutionAuthority, TargetSelector, ToolRequest, execute_tool,
    plan_tool,
};
use plotium_core::state::ActivitySourceId;

fn app() -> PlotiumApp {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/jcamp-ppm.dx");
    app.load_from(&path);
    app.load_from(&path);
    let source = ActivitySourceId::default();
    app.session
        .result_follow
        .synchronize(app.session.project_context, Some(source));
    app.session.background_activity.execution_source = Some(source);
    app
}

fn rename_canvas(app: &mut PlotiumApp, index: usize) {
    rename_canvases(app, &[index]);
}

fn rename_canvases(app: &mut PlotiumApp, indices: &[usize]) {
    let ids = indices
        .iter()
        .map(|index| app.doc.canvases[*index].resource_id.to_string())
        .collect();
    rename_resources(app, ids, ExecutionAuthority::ReversibleModify).unwrap();
}

fn rename_resources(
    app: &mut PlotiumApp,
    ids: Vec<String>,
    authority: ExecutionAuthority,
) -> Result<plotium_core::automation::ToolResult, plotium_core::automation::AutomationError> {
    let request = ToolRequest {
        tool_id: "resource.rename".into(),
        tool_version: 1,
        parameters: serde_json::json!({"name": "Updated result"}),
        targets: TargetSelector::Explicit { ids },
        expected_revision: DocumentRevision(app.doc.automation_revision),
        caller: CallerType::Agent,
    };
    let plan = plan_tool(app, request)?;
    execute_tool(app, plan, authority)
}

#[test]
fn one_operation_groups_its_results_and_repeated_target_updates_preserve_view() {
    let mut app = app();
    assert_eq!(app.doc.canvases.len(), 2);
    rename_canvases(&mut app, &[0, 1]);
    app.session.primary_sidebar_visible = false;
    app.session.secondary_sidebar_visible = false;
    let ctx = crate::typography::test_context();
    let _ = ctx.run_ui(Default::default(), |ui| apply(&mut app, ui));
    assert_eq!(app.session.ui.frame_selection.len(), 2);
    assert!(app.session.view == PrimaryView::Canvas);
    app.session.viewport_mode = plotium_core::state::ViewportMode::Manual;
    app.session.ui.frame_selection.clear();
    let active = app.session.active_canvas;
    rename_canvases(&mut app, &[0, 1]);
    let _ = ctx.run_ui(Default::default(), |ui| apply(&mut app, ui));
    assert!(app.session.ui.frame_selection.is_empty());
    assert_eq!(app.session.active_canvas, active);
    assert_eq!(
        app.session.viewport_mode,
        plotium_core::state::ViewportMode::Manual
    );
    assert!(!app.session.primary_sidebar_visible);
    assert!(!app.session.secondary_sidebar_visible);
}

#[test]
fn foreign_connection_results_do_not_navigate_and_view_latest_does_not_resume() {
    let mut app = app();
    app.session.background_activity.execution_source = Some(ActivitySourceId::default());
    rename_canvas(&mut app, 0);
    let selected = app.session.active_canvas;
    let ctx = crate::typography::test_context();
    let _ = ctx.run_ui(Default::default(), |ui| apply(&mut app, ui));
    assert_eq!(app.session.active_canvas, selected);
    assert!(gate(&app, CommandId::ViewLatestResults).is_err());
    app.session.background_activity.execution_source = app.session.result_follow.source();
    rename_canvas(&mut app, 0);
    app.session.result_follow.pause("Browsing");
    commands::execute_without_clipboard(CommandId::ViewLatestResults, &mut app, &ctx);
    assert_eq!(app.session.result_follow.mode(), ResultFollowMode::Paused);
    assert_eq!(app.session.active_canvas, Some(0));
    assert!(
        commands::catalog(&app)
            .iter()
            .any(|item| item.id == CommandId::FollowResults)
    );
}

#[test]
fn pointer_motion_and_history_details_do_not_pause_but_workspace_press_does() {
    let mut app = app();
    let ctx = crate::typography::test_context();
    let _ = ctx.run_ui(Default::default(), |ui| remember_workspace(ui));
    let position = egui::pos2(100.0, 100.0);
    pause_on_input(
        &mut app,
        &ctx,
        &egui::RawInput {
            events: vec![egui::Event::PointerMoved(position)],
            ..Default::default()
        },
    );
    assert_eq!(
        app.session.result_follow.mode(),
        ResultFollowMode::Following
    );
    pause_for_command(&mut app, CommandId::OperationHistory);
    assert_eq!(
        app.session.result_follow.mode(),
        ResultFollowMode::Following
    );
    pause_on_input(
        &mut app,
        &ctx,
        &egui::RawInput {
            events: vec![egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            }],
            ..Default::default()
        },
    );
    assert_eq!(app.session.result_follow.mode(), ResultFollowMode::Paused);
    app.session.result_follow.resume();
    pause_for_command(&mut app, CommandId::ZoomToFit);
    assert_eq!(app.session.result_follow.mode(), ResultFollowMode::Paused);
}
#[test]
fn empty_canvas_does_not_move_the_view_and_history_overlay_keeps_following() {
    let mut app = app();
    app.doc
        .canvases
        .push(plotium_core::state::CanvasDocument::new(
            "Empty".into(),
            [100.0, 80.0],
        ));
    rename_canvas(&mut app, 2);
    assert!(available_results(&app, true).is_empty());
    let active = app.session.active_canvas;
    let ctx = crate::typography::test_context();
    let mut details_position = egui::Pos2::ZERO;
    let _ = ctx.run_ui(Default::default(), |ui| {
        apply(&mut app, ui);
        remember_workspace(ui);
        let shown = egui::Window::new("Operation details")
            .fixed_pos(egui::pos2(40.0, 40.0))
            .show(ui.ctx(), |ui| {
                ui.label("Completed operation details");
            })
            .unwrap();
        details_position = shown.response.rect.center();
    });
    assert_eq!(app.session.active_canvas, active);
    pause_on_input(
        &mut app,
        &ctx,
        &egui::RawInput {
            events: vec![egui::Event::PointerButton {
                pos: details_position,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            }],
            ..Default::default()
        },
    );
    assert_eq!(
        app.session.result_follow.mode(),
        ResultFollowMode::Following
    );
}

#[test]
fn resume_returns_to_existing_result_after_manual_browsing() {
    let mut app = app();
    rename_canvas(&mut app, 0);
    let ctx = crate::typography::test_context();
    let _ = ctx.run_ui(Default::default(), |ui| apply(&mut app, ui));
    assert_eq!(app.session.active_canvas, Some(0));
    app.session.result_follow.pause("Browsing another figure");
    app.activate_canvas(1);
    rename_canvas(&mut app, 0);
    let _ = ctx.run_ui(Default::default(), |ui| apply(&mut app, ui));
    assert_eq!(app.session.active_canvas, Some(1));
    commands::execute_without_clipboard(CommandId::FollowResults, &mut app, &ctx);
    assert_eq!(app.session.active_canvas, Some(0));
    assert_eq!(
        app.session.result_follow.mode(),
        ResultFollowMode::Following
    );
}

#[test]
fn modified_edit_key_pauses_before_background_polls() {
    let mut app = app();
    let ctx = crate::typography::test_context();
    let _ = ctx.run_ui(Default::default(), |ui| remember_workspace(ui));
    let position = egui::pos2(100.0, 100.0);
    let _ = ctx.run_ui(
        egui::RawInput {
            events: vec![egui::Event::PointerMoved(position)],
            ..Default::default()
        },
        |ui| remember_workspace(ui),
    );
    pause_on_input(
        &mut app,
        &ctx,
        &egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::Backspace,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers {
                    ctrl: true,
                    ..Default::default()
                },
            }],
            ..Default::default()
        },
    );
    assert_eq!(app.session.result_follow.mode(), ResultFollowMode::Paused);
}
#[test]
fn data_sheet_interaction_pauses_following_before_results_are_applied() {
    let mut app = app();
    app.new_table_dataset();
    app.session.ui.sheet_open = Some(app.doc.datasets.len() - 1);
    let ctx = crate::typography::test_context();
    let _ = ctx.run_ui(Default::default(), |ui| {
        begin_frame(ui.ctx());
        remember_workspace(ui);
        super::super::data_sheet::data_sheet_dock(&mut app, ui, false);
    });
    // The docked sheet is part of the remembered workspace.
    let position = super::super::data_sheet::dock_rect(&ctx)
        .expect("the sheet is docked")
        .center();
    pause_on_input(
        &mut app,
        &ctx,
        &egui::RawInput {
            events: vec![egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            }],
            ..Default::default()
        },
    );
    assert_eq!(app.session.result_follow.mode(), ResultFollowMode::Paused);
    let active = app.session.active_canvas;
    rename_canvas(&mut app, 0);
    let _ = ctx.run_ui(Default::default(), |ui| apply(&mut app, ui));
    assert_eq!(app.session.active_canvas, active);
}

#[test]
fn focused_floating_editor_text_input_pauses_without_a_new_pointer_press() {
    let mut app = app();
    let ctx = crate::typography::test_context();
    let mut text = String::new();
    let _ = ctx.run_ui(Default::default(), |ui| {
        begin_frame(ui.ctx());
        remember_workspace(ui);
        egui::Window::new("Edit scientific parameters").show(ui.ctx(), |ui| {
            register_editor(ui);
            ui.text_edit_singleline(&mut text).request_focus();
        });
    });
    app.session.result_follow.resume();
    pause_on_input(
        &mut app,
        &ctx,
        &egui::RawInput {
            events: vec![egui::Event::Text("12".into())],
            ..Default::default()
        },
    );
    assert_eq!(app.session.result_follow.mode(), ResultFollowMode::Paused);
}

#[test]
fn switching_operation_target_follows_only_that_canvas_and_latest_does_not_show_history() {
    let mut app = app();
    let ctx = crate::typography::test_context();
    rename_canvas(&mut app, 0);
    let _ = ctx.run_ui(Default::default(), |ui| apply(&mut app, ui));
    assert_eq!(app.session.active_canvas, Some(0));
    rename_canvas(&mut app, 1);
    let _ = ctx.run_ui(Default::default(), |ui| apply(&mut app, ui));
    let expected = vec![BoardFrameId::Page(app.doc.canvases[1].resource_id)];
    assert_eq!(app.session.active_canvas, Some(1));
    assert_eq!(app.session.ui.frame_selection, expected);
    let viewport = app.session.viewport_mode;
    app.session.ui.frame_selection.clear();
    rename_canvas(&mut app, 1);
    let _ = ctx.run_ui(Default::default(), |ui| apply(&mut app, ui));
    assert_eq!(app.session.viewport_mode, viewport);
    assert!(app.session.ui.frame_selection.is_empty());
    pause(&mut app, "Browsing");
    app.activate_canvas(0);
    commands::execute_without_clipboard(CommandId::ViewLatestResults, &mut app, &ctx);
    assert_eq!(app.session.active_canvas, Some(1));
    assert_eq!(app.session.ui.frame_selection, expected);
    assert_eq!(app.session.result_follow.mode(), ResultFollowMode::Paused);
    app.activate_canvas(0);
    commands::execute_without_clipboard(CommandId::FollowResults, &mut app, &ctx);
    assert_eq!(app.session.active_canvas, Some(1));
    assert_eq!(app.session.ui.frame_selection, expected);
}

#[test]
fn pause_control_press_freezes_camera_before_poll_and_release_does_not_resume() {
    let mut app = app();
    let ctx = crate::typography::test_context();
    let _ = ctx.run_ui(Default::default(), |ui| {
        begin_frame(ui.ctx());
        button(&mut app, ui);
    });
    let position = ctx
        .data(|data| {
            data.get_temp::<Vec<(egui::Rect, egui::LayerId)>>(egui::Id::new(PAUSE_CONTROLS))
        })
        .unwrap()[0]
        .0
        .center();
    app.session.viewport_mode =
        plotium_core::state::ViewportMode::Fit(plotium_core::state::BoardFitTarget::AllFrames);
    app.session.board_reveal = Some(BoardFrameId::Page(app.doc.canvases[0].resource_id));
    let camera = app.session.board;
    let active = app.session.active_canvas;
    for pressed in [true, false] {
        let input = egui::RawInput {
            events: vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
            ..Default::default()
        };
        pause_on_input(&mut app, &ctx, &input);
        if pressed {
            rename_canvas(&mut app, 0);
        }
        let _ = ctx.run_ui(input, |ui| {
            begin_frame(ui.ctx());
            button(&mut app, ui);
            apply(&mut app, ui);
            super::super::canvas::consume_board_reveal(&mut app, ui.ctx());
        });
        assert_eq!(app.session.result_follow.mode(), ResultFollowMode::Paused);
        assert_eq!(
            app.session.viewport_mode,
            plotium_core::state::ViewportMode::Manual
        );
        assert!(app.session.board_reveal.is_none());
        assert_eq!(app.session.active_canvas, active);
        assert_eq!(app.session.board.zoom, camera.zoom);
        assert_eq!(app.session.board.world_center, camera.world_center);
    }
    assert!(pause_gesture(&ctx));
    pause_on_input(&mut app, &ctx, &egui::RawInput::default());
    assert!(!pause_gesture(&ctx));
}

#[test]
fn explicit_operated_canvas_wins_over_input_dataset_and_later_failure_does_not_hide_it() {
    let mut app = app();
    rename_canvas(&mut app, 0);
    let ids = vec![
        app.doc.datasets[0].resource_id().to_string(),
        app.doc.canvases[1].resource_id.to_string(),
    ];
    rename_resources(&mut app, ids, ExecutionAuthority::ReversibleModify).unwrap();
    let resource = ResourceRef::from(app.doc.canvases[1].resource_id);
    assert_eq!(available_results(&app, false), vec![resource.clone()]);
    let denied = vec![app.doc.canvases[0].resource_id.to_string()];
    assert!(rename_resources(&mut app, denied, ExecutionAuthority::Read).is_err());
    assert_eq!(available_results(&app, false), vec![resource]);
    let ctx = crate::typography::test_context();
    let _ = ctx.run_ui(Default::default(), |ui| apply(&mut app, ui));
    assert_eq!(app.session.active_canvas, Some(1));
    assert_eq!(
        app.session.ui.frame_selection,
        vec![BoardFrameId::Page(app.doc.canvases[1].resource_id)]
    );
}

#[test]
fn paused_results_count_distinct_updates_and_view_latest_acknowledges_only_its_targets() {
    let mut app = app();
    let ctx = crate::typography::test_context();
    rename_canvas(&mut app, 0);
    let _ = ctx.run_ui(Default::default(), |ui| apply(&mut app, ui));
    pause(&mut app, "Browsing");
    assert_eq!(unseen_count(&app), 0);
    rename_canvas(&mut app, 0);
    rename_canvas(&mut app, 0);
    assert_eq!(unseen_count(&app), 1);
    rename_canvas(&mut app, 1);
    assert_eq!(unseen_count(&app), 2);
    let active = app.session.active_canvas;
    let _ = ctx.run_ui(Default::default(), |ui| apply(&mut app, ui));
    assert_eq!(app.session.active_canvas, active);
    execute(&mut app, &ctx, CommandId::ViewLatestResults);
    assert_eq!(app.session.result_follow.mode(), ResultFollowMode::Paused);
    assert_eq!(app.session.active_canvas, Some(1));
    assert_eq!(unseen_count(&app), 1);
    rename_canvas(&mut app, 1);
    assert_eq!(unseen_count(&app), 2);
    execute(&mut app, &ctx, CommandId::FollowResults);
    assert_eq!(unseen_count(&app), 0);
    pause(&mut app, "Browsing again");
    assert_eq!(unseen_count(&app), 0);
}

#[test]
fn paused_count_excludes_empty_deleted_and_foreign_results_and_clears_at_boundaries() {
    let mut app = app();
    pause(&mut app, "Browsing");
    rename_canvas(&mut app, 0);
    assert_eq!(unseen_count(&app), 1);
    app.doc
        .canvases
        .push(plotium_core::state::CanvasDocument::new(
            "Empty".into(),
            [100.0, 80.0],
        ));
    rename_canvas(&mut app, 2);
    assert_eq!(unseen_count(&app), 1);
    app.session.background_activity.execution_source = Some(ActivitySourceId::default());
    rename_canvas(&mut app, 1);
    assert_eq!(unseen_count(&app), 1);
    app.doc.canvases.remove(0);
    assert_eq!(unseen_count(&app), 0);
    app.session
        .result_follow
        .synchronize(app.session.project_context, None);
    assert_eq!(unseen_count(&app), 0);
    assert_eq!(app.session.result_follow.mode(), ResultFollowMode::Off);
}

#[test]
fn pausing_establishes_a_baseline_and_viewing_one_result_preserves_other_unread_results() {
    let mut app = app();
    rename_canvas(&mut app, 0);
    pause(&mut app, "Browsing");
    assert_eq!(unseen_count(&app), 0);
    rename_canvas(&mut app, 0);
    rename_canvas(&mut app, 1);
    let row = app
        .session
        .background_activity
        .results()
        .front()
        .unwrap()
        .clone();
    assert_eq!(unseen_count(&app), 2);
    app.session
        .result_follow
        .acknowledge(row.resource.clone(), row.operation);
    assert_eq!(unseen_count(&app), 1);
    app.session
        .result_follow
        .acknowledge(row.resource, row.operation);
    assert_eq!(unseen_count(&app), 1);
    app.session
        .result_follow
        .advance(app.session.background_activity.latest_id());
    assert_eq!(unseen_count(&app), 0);
}
