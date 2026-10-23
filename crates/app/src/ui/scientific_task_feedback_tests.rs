use super::*;
use plotium_core::automation::{
    CallerType, DocumentRevision, ExecutionAuthority, TargetSelector, ToolRequest, execute_tool,
    plan_tool,
};

fn stale_task() -> (PlotiumApp, ActivityId) {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/jcamp-ppm.dx");
    app.load_from(&path);
    for (tool, parameters) in [
        (
            "processing.edit",
            serde_json::json!({"edits": [{"op": "add", "axis": "direct", "kind": "Invert"}]}),
        ),
        (
            "resource.rename",
            serde_json::json!({"name": "Edited while computing"}),
        ),
    ] {
        let plan = plan_tool(
            &app,
            ToolRequest {
                tool_id: tool.into(),
                tool_version: 1,
                parameters,
                targets: TargetSelector::Explicit {
                    ids: vec![app.doc.datasets[0].resource_id().to_string()],
                },
                expected_revision: DocumentRevision(app.doc.automation_revision),
                caller: CallerType::Human,
            },
        )
        .unwrap();
        execute_tool(&mut app, plan, ExecutionAuthority::ReversibleModify).unwrap();
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.compute_busy() && std::time::Instant::now() < deadline {
        app.poll_compute();
        std::thread::yield_now();
    }
    app.poll_compute();
    assert!(!app.compute_busy());
    let id = app
        .session
        .background_activity
        .operations()
        .iter()
        .find(|operation| operation.task_id.is_some())
        .unwrap()
        .id;
    (app, id)
}

#[test]
fn stale_task_has_unapplied_explanation_and_dismissible_warning() {
    let (app, id) = stale_task();
    let task = receipt(&app, id).unwrap();
    assert_eq!(task.status, TaskStatus::Stale);
    assert!(
        notice_summary(&app, id)
            .unwrap()
            .contains(task.status.label())
    );
    assert!(
        task.message
            .as_ref()
            .is_some_and(|message| !message.is_empty())
    );
    let ctx = crate::typography::test_context();
    let notice = super::super::activity_results::notice(&app, &ctx).unwrap();
    assert_eq!(notice.id, id);
    assert!(super::super::activity_results::has_pending_warning(
        &app, &ctx
    ));
    super::super::activity_results::dismiss_notice(&app, &ctx, id);
    assert!(super::super::activity_results::notice(&app, &ctx).is_none());
    assert!(!super::super::activity_results::has_pending_warning(
        &app, &ctx
    ));
}

#[test]
fn recompute_is_searchable_routes_by_stable_activity_and_preserves_navigation() {
    let (mut app, id) = stale_task();
    let command = CommandId::RecomputeScientificTask(id);
    assert!(
        commands::catalog(&app)
            .iter()
            .any(|item| item.id == command && item.enabled)
    );
    let original = receipt(&app, id).unwrap().task_id.clone();
    let selected = app.active_dataset();
    let view = app.session.view;
    let history_open = app.session.ui.diagnostics_open;
    let ctx = crate::typography::test_context();
    commands::execute_without_clipboard(command, &mut app, &ctx);
    assert_eq!(receipt(&app, id).unwrap().task_id, original);
    assert!(
        app.session
            .scientific_tasks
            .list(0, 128)
            .iter()
            .any(|task| task.task_id != original && !task.status.terminal())
    );
    assert!(!commands::describe(&app, command).enabled);
    assert_eq!(app.active_dataset(), selected);
    assert!(app.session.view == view);
    assert_eq!(app.session.ui.diagnostics_open, history_open);
}

#[test]
fn cleared_or_foreign_task_cannot_be_recomputed_by_stale_command() {
    let (mut app, id) = stale_task();
    app.session.project_context = Default::default();
    let command = CommandId::RecomputeScientificTask(id);
    assert!(!commands::describe(&app, command).enabled);
    let before = app.session.scientific_tasks.list(0, 128);
    commands::execute_without_clipboard(command, &mut app, &egui::Context::default());
    assert_eq!(app.session.scientific_tasks.list(0, 128), before);
    app.session.clear_operation_history();
    app.poll_compute();
    assert!(app.session.background_activity.operations().is_empty());
    assert!(!commands::describe(&app, command).enabled);
}
