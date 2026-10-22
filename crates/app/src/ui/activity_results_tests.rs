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
fn success_notice_expires_and_repeated_update_does_not_reopen_it() {
    let mut app = app();
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
        |_| assert!(notice(&app, &ctx).is_none()),
    );
    assert_eq!(app.session.background_activity.results().len(), 1);
}

#[test]
fn dismissed_notice_stays_dismissed_during_repeated_updates() {
    let mut app = app();
    rename(&mut app, "First update");
    let ctx = crate::typography::test_context();
    let id = notice(&app, &ctx).unwrap().id;
    dismiss_notice(&app, &ctx, id);
    rename(&mut app, "Another update");
    assert!(notice(&app, &ctx).is_none());
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
