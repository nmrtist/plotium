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
    let request = ToolRequest {
        tool_id: "resource.rename".into(),
        tool_version: 1,
        parameters: serde_json::json!({"name": format!("Updated {index}")}),
        targets: TargetSelector::Explicit {
            ids: vec![app.doc.canvases[index].resource_id.to_string()],
        },
        expected_revision: DocumentRevision(app.doc.automation_revision),
        caller: CallerType::Agent,
    };
    let plan = plan_tool(app, request).unwrap();
    execute_tool(app, plan, ExecutionAuthority::ReversibleModify).unwrap();
}

#[test]
fn simultaneous_results_form_one_collection_and_updates_preserve_view_and_selection() {
    let mut app = app();
    assert_eq!(app.doc.canvases.len(), 2);
    rename_canvas(&mut app, 0);
    rename_canvas(&mut app, 1);
    app.session.primary_sidebar_visible = false;
    app.session.secondary_sidebar_visible = false;
    let ctx = crate::typography::test_context();
    let _ = ctx.run_ui(Default::default(), |ui| apply(&mut app, ui));
    assert_eq!(app.session.ui.frame_selection.len(), 2);
    assert!(app.session.view == PrimaryView::Canvas);
    app.session.viewport_mode = plotium_core::state::ViewportMode::Manual;
    app.session.ui.frame_selection.clear();
    let active = app.session.active_canvas;
    rename_canvas(&mut app, 1);
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
        super::super::data_sheet::data_sheet_window(&mut app, ui.ctx());
    });
    let layer = ctx
        .data(|data| data.get_temp::<Vec<egui::LayerId>>(egui::Id::new(EDITABLE_LAYERS)))
        .unwrap()[0];
    let position = ctx
        .memory(|memory| memory.area_rect(layer.id))
        .unwrap()
        .center();
    assert_eq!(ctx.layer_id_at(position), Some(layer));
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
