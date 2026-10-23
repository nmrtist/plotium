use super::*;
use plotium_core::automation::{
    CallerType, DocumentRevision, ExecutionAuthority, TargetSelector, ToolRequest, execute_tool,
    plan_tool,
};
use plotium_core::state::ActivitySourceId;

fn app() -> PlotiumApp {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/jcamp-ppm.dx");
    app.load_from(&fixture);
    app.load_from(&fixture);
    let source = ActivitySourceId::default();
    app.session
        .result_follow
        .synchronize(app.session.project_context, Some(source));
    app.session.background_activity.execution_source = Some(source);
    app.session.result_follow.pause("Testing");
    app
}

fn update(app: &mut PlotiumApp, index: usize) {
    let request = ToolRequest {
        tool_id: "resource.rename".into(),
        tool_version: 1,
        parameters: serde_json::json!({"name": "Updated result"}),
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
fn dismiss_preserves_unseen_and_coalesces_repeat_updates_until_new_object() {
    let mut app = app();
    let ctx = crate::typography::test_context();
    update(&mut app, 0);
    assert_eq!(candidate(&app, &ctx), Some(1));
    execute(&mut app, &ctx, CommandId::DismissPausedResults);
    assert_eq!(super::super::result_follow::unseen_count(&app), 1);
    assert_eq!(candidate(&app, &ctx), None);
    update(&mut app, 0);
    assert_eq!(candidate(&app, &ctx), None);
    update(&mut app, 1);
    assert_eq!(candidate(&app, &ctx), Some(2));
}

#[test]
fn connection_and_new_pause_do_not_inherit_dismissal() {
    let mut app = app();
    let ctx = crate::typography::test_context();
    update(&mut app, 0);
    execute(&mut app, &ctx, CommandId::DismissPausedResults);
    app.session.result_follow.resume();
    app.session.result_follow.pause("Again");
    assert_eq!(candidate(&app, &ctx), Some(1));
    let source = ActivitySourceId::default();
    app.session
        .result_follow
        .synchronize(app.session.project_context, Some(source));
    app.session.result_follow.resume();
    app.session.result_follow.pause("New connection");
    assert_eq!(candidate(&app, &ctx), None);
    app.session.background_activity.execution_source = Some(source);
    update(&mut app, 0);
    assert_eq!(candidate(&app, &ctx), Some(1));
}

#[test]
fn preferences_can_disable_and_restore_reminders_without_acknowledging_results() {
    let mut app = app();
    let ctx = crate::typography::test_context();
    update(&mut app, 0);
    app.settings.general.paused_result_reminders = false;
    assert_eq!(candidate(&app, &ctx), None);
    assert_eq!(super::super::result_follow::unseen_count(&app), 1);
    app.settings.general.paused_result_reminders = true;
    assert_eq!(candidate(&app, &ctx), Some(1));
}

#[test]
fn hidden_reminder_does_not_start_or_continue_expiry() {
    let mut app = app();
    let ctx = crate::typography::test_context();
    update(&mut app, 0);
    for _ in 0..10 {
        assert_eq!(candidate(&app, &ctx), Some(1));
    }
    let id = key(&app);
    assert_eq!(
        ctx.data(|data| data.get_temp::<Reminder>(id))
            .unwrap()
            .visible_seconds,
        0.0
    );
    ctx.data_mut(|data| {
        let mut reminder = data.get_temp::<Reminder>(id).unwrap();
        reminder.last_visible = Some(1.0);
        data.insert_temp(id, reminder);
    });
    hidden(&app, &ctx);
    assert!(
        ctx.data(|data| data.get_temp::<Reminder>(id))
            .unwrap()
            .last_visible
            .is_none()
    );
}

#[test]
fn expired_reminder_does_not_reappear_for_same_object_updates() {
    let mut app = app();
    let ctx = crate::typography::test_context();
    update(&mut app, 0);
    assert_eq!(candidate(&app, &ctx), Some(1));
    let id = key(&app);
    ctx.data_mut(|data| {
        let mut reminder = data.get_temp::<Reminder>(id).unwrap();
        reminder.visible_seconds = VISIBLE_SECONDS;
        data.insert_temp(id, reminder);
    });
    update(&mut app, 0);
    assert_eq!(candidate(&app, &ctx), None);
    update(&mut app, 1);
    assert_eq!(candidate(&app, &ctx), Some(2));
}

#[test]
fn failures_keep_banner_priority_without_spending_reminder_lifetime() {
    let mut app = app();
    let ctx = crate::typography::test_context();
    update(&mut app, 0);
    app.load_from(std::path::Path::new("missing-reminder-test.dx"));
    assert!(super::super::pending_feedback(&app).is_some());
    let _ = ctx.run_ui(Default::default(), |ui| {
        super::super::feedback_banner::feedback_banner(&mut app, ui, false);
    });
    let reminder = ctx
        .data(|data| data.get_temp::<Reminder>(key(&app)))
        .unwrap();
    assert!(reminder.last_visible.is_none());
    assert_eq!(reminder.visible_seconds, 0.0);
    assert_eq!(candidate(&app, &ctx), Some(1));
}

#[test]
fn reminder_expires_after_visible_focused_frames_and_waits_while_unfocused() {
    let mut app = app();
    let ctx = crate::typography::test_context();
    update(&mut app, 0);
    let render = |app: &mut PlotiumApp, time: f64, focused: bool| {
        let input = egui::RawInput {
            time: Some(time),
            focused,
            ..Default::default()
        };
        let _ = ctx.run_ui(input, |ui| {
            if let Some(count) = candidate(app, ui.ctx()) {
                show(app, ui, false, count);
            }
        });
    };
    for frame in 0..100 {
        render(&mut app, f64::from(frame) * 0.1, false);
    }
    assert_eq!(candidate(&app, &ctx), Some(1));
    let reminder = ctx
        .data(|data| data.get_temp::<Reminder>(key(&app)))
        .unwrap();
    assert_eq!(reminder.visible_seconds, 0.0);
    for frame in 100..190 {
        render(&mut app, f64::from(frame) * 0.1, true);
    }
    assert_eq!(candidate(&app, &ctx), None);
    assert_eq!(super::super::result_follow::unseen_count(&app), 1);
}

#[test]
fn dismissed_burst_does_not_fall_back_to_generic_success_notification() {
    let mut app = app();
    let ctx = crate::typography::test_context();
    update(&mut app, 0);
    let operation = app.session.background_activity.latest_id();
    assert_eq!(candidate(&app, &ctx), Some(1));
    assert!(!suppress_success(&app, &ctx, operation));
    execute(&mut app, &ctx, CommandId::DismissPausedResults);
    assert!(suppress_success(&app, &ctx, operation));
    update(&mut app, 0);
    assert_eq!(candidate(&app, &ctx), None);
    assert!(suppress_success(
        &app,
        &ctx,
        app.session.background_activity.latest_id()
    ));
    app.session.result_follow.resume();
    assert!(!suppress_success(&app, &ctx, operation));
}
