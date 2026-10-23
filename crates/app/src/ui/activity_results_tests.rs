use super::*;
use plotium_core::automation::{
    CallerType, DocumentRevision, ExecutionAuthority, TargetSelector, ToolRequest, execute_tool,
    plan_tool,
};

fn app() -> PlotiumApp {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    app.new_table_dataset();
    app
}

fn rename(app: &mut PlotiumApp, name: &str) {
    let plan = rename_plan(app, name);
    execute_tool(app, plan, ExecutionAuthority::ReversibleModify).unwrap();
}

fn rename_plan(app: &PlotiumApp, name: &str) -> plotium_core::automation::ToolPlan {
    let request = ToolRequest {
        tool_id: "resource.rename".into(),
        tool_version: 1,
        parameters: serde_json::json!({"name": name}),
        targets: TargetSelector::Explicit {
            ids: vec![app.doc.datasets[0].resource_id().to_string()],
        },
        expected_revision: DocumentRevision(app.doc.automation_revision),
        caller: CallerType::Agent,
    };
    plan_tool(app, request).unwrap()
}

#[test]
fn background_failure_remains_visible_during_successful_import_until_dismissed() {
    let mut app = app();
    let plan = rename_plan(&app, "Denied update");
    assert!(execute_tool(&mut app, plan, ExecutionAuthority::Read).is_err());
    let dataset = app.doc.datasets[0].resource_id();
    app.record_completed_user_import(
        "data.csv".into(),
        vec![plotium_core::state::ImportItem {
            path: "data.csv".into(),
            result: Ok(plotium_core::state::ImportTarget {
                dataset,
                canvas: None,
            }),
        }],
    );
    let ctx = crate::typography::test_context();
    assert!(super::super::import_feedback::notice(&app, &ctx).is_some());
    let Some(BannerNotice::Activity(activity)) = banner_notice(&app, &ctx) else {
        panic!("background failure must take precedence over import success");
    };
    assert_eq!(activity.outcome, OperationOutcome::Failure);
    assert!(has_pending_warning(&app, &ctx));
    dismiss_notice(&app, &ctx, activity.id);
    assert!(!has_pending_warning(&app, &ctx));
    assert!(matches!(
        banner_notice(&app, &ctx),
        Some(BannerNotice::Import(_))
    ));
}

#[test]
fn result_link_resolves_stable_identity_after_reordering_and_rejects_deletion() {
    let mut app = app();
    let resource = ResourceRef::from(app.doc.datasets[0].resource_id());
    app.new_table_dataset();
    app.doc.datasets.swap(0, 1);
    let ctx = crate::typography::test_context();
    let _ = ctx.run_ui(Default::default(), |ui| {
        assert!(open_result(&mut app, ui, &resource))
    });
    assert_eq!(app.session.ui.sheet_open, Some(1));
    app.doc.datasets.remove(1);
    app.new_table_dataset();
    let selected = app.active_dataset();
    let _ = ctx.run_ui(Default::default(), |ui| {
        assert!(!open_result(&mut app, ui, &resource))
    });
    assert_eq!(app.active_dataset(), selected);
}

#[test]
fn current_object_update_is_visible_while_paused_and_expires() {
    let mut app = app();
    app.session.result_follow.pause("Inspecting data");
    rename(&mut app, "First update");
    let ctx = crate::typography::test_context();
    let _ = ctx.run_ui(
        egui::RawInput {
            time: Some(0.0),
            ..Default::default()
        },
        |_| assert!(notice(&app, &ctx).is_some()),
    );
    let _ = ctx.run_ui(
        egui::RawInput {
            time: Some(4.0),
            ..Default::default()
        },
        |_| assert!(notice(&app, &ctx).is_none()),
    );
    rename(&mut app, "Another update");
    let _ = ctx.run_ui(
        egui::RawInput {
            time: Some(5.0),
            ..Default::default()
        },
        |_| {
            assert!(
                notice(&app, &ctx)
                    .unwrap()
                    .summary
                    .contains("Another update")
            )
        },
    );
    let _ = ctx.run_ui(
        egui::RawInput {
            time: Some(9.0),
            ..Default::default()
        },
        |_| assert!(notice(&app, &ctx).is_none()),
    );
    assert_eq!(app.session.background_activity.results().len(), 1);
}

#[test]
fn dismissing_one_update_does_not_hide_future_changes_to_current_object() {
    let mut app = app();
    rename(&mut app, "First update");
    let ctx = crate::typography::test_context();
    let id = notice(&app, &ctx).unwrap().id;
    dismiss_notice(&app, &ctx, id);
    assert!(notice(&app, &ctx).is_none());
    rename(&mut app, "Another update");
    assert_ne!(notice(&app, &ctx).unwrap().id, id);
}

#[test]
fn offscreen_repeated_update_does_not_notify_when_later_opened() {
    let mut app = app();
    rename(&mut app, "First update");
    let ctx = crate::typography::test_context();
    let id = notice(&app, &ctx).unwrap().id;
    dismiss_notice(&app, &ctx, id);
    app.new_table_dataset();
    rename(&mut app, "Offscreen update");
    assert!(notice(&app, &ctx).is_none());
    app.focus_single(0);
    assert!(notice(&app, &ctx).is_none());
}

#[test]
fn failure_keeps_priority_over_repeated_current_object_update() {
    let mut app = app();
    rename(&mut app, "First update");
    let plan = rename_plan(&app, "Denied update");
    assert!(execute_tool(&mut app, plan, ExecutionAuthority::Read).is_err());
    rename(&mut app, "Updated while following is off");
    app.session.result_follow.stop();
    let ctx = crate::typography::test_context();
    let failure = notice(&app, &ctx).unwrap();
    assert_eq!(failure.outcome, OperationOutcome::Failure);
    dismiss_notice(&app, &ctx, failure.id);
    assert!(
        notice(&app, &ctx)
            .unwrap()
            .summary
            .contains("Updated while following is off")
    );
}

#[test]
fn late_update_from_previous_connection_does_not_reopen_success_notice() {
    let mut app = app();
    let source = plotium_core::state::ActivitySourceId::default();
    app.session
        .result_follow
        .synchronize(app.session.project_context, Some(source));
    app.session.background_activity.execution_source = Some(source);
    rename(&mut app, "First update");
    let ctx = crate::typography::test_context();
    let id = notice(&app, &ctx).unwrap().id;
    dismiss_notice(&app, &ctx, id);
    app.session.result_follow.synchronize(
        app.session.project_context,
        Some(plotium_core::state::ActivitySourceId::default()),
    );
    rename(&mut app, "Old connection update");
    assert!(notice(&app, &ctx).is_none());
}

#[test]
fn current_canvas_updates_notify_without_changing_view_or_follow_mode() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    app.load_from(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../io/tests/fixtures/nmr/jcamp-ppm.dx"),
    );
    app.session.result_follow.stop();
    let canvas = app.doc.canvases[app.session.active_canvas.unwrap()].resource_id;
    let ctx = crate::typography::test_context();
    for name in ["First canvas change", "Repeated canvas change"] {
        let plan = plan_tool(
            &app,
            ToolRequest {
                tool_id: "resource.rename".into(),
                tool_version: 1,
                parameters: serde_json::json!({"name": name}),
                targets: TargetSelector::Explicit {
                    ids: vec![canvas.to_string()],
                },
                expected_revision: DocumentRevision(app.doc.automation_revision),
                caller: CallerType::Agent,
            },
        )
        .unwrap();
        execute_tool(&mut app, plan, ExecutionAuthority::ReversibleModify).unwrap();
        let notice = notice(&app, &ctx).unwrap();
        assert!(notice.summary.contains(name));
        dismiss_notice(&app, &ctx, notice.id);
    }
    assert!(app.session.view == PrimaryView::Canvas);
    assert_eq!(
        app.session.result_follow.mode(),
        plotium_core::state::ResultFollowMode::Off
    );
}

#[test]
fn rendering_result_rows_does_not_navigate_or_open_panels() {
    let mut app = app();
    rename(&mut app, "Updated data");
    app.session.primary_sidebar_visible = false;
    let selected = app.active_dataset();
    let view = app.session.view;
    let sheet = app.session.ui.sheet_open;
    let ctx = crate::typography::test_context();
    let _ = ctx.run_ui(Default::default(), |ui| results(&mut app, ui));
    assert!(!app.session.primary_sidebar_visible);
    assert_eq!(app.active_dataset(), selected);
    assert!(app.session.view == view);
    assert_eq!(app.session.ui.sheet_open, sheet);
}
