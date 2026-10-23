use super::*;
use crate::automation::{
    DocumentRevision, PreparedToolRun, TargetSelector, ToolRequest, plan_tool,
};
use crate::state::ActivitySourceId;

fn result(resource: ResourceRef) -> ToolResult {
    ToolResult {
        tool_id: "resource.rename".into(),
        before_revision: DocumentRevision(0),
        after_revision: DocumentRevision(1),
        targets: Vec::new(),
        produced: Vec::new(),
        modified: vec![resource],
        diagnostics: Vec::new(),
        verification: Vec::new(),
        value: serde_json::Value::Null,
    }
}

#[test]
fn asynchronous_tool_audit_keeps_original_source_after_connection_changes() {
    let mut app = PlotiumApp::new();
    app.new_table_dataset();
    let resource = ResourceRef::from(app.doc.datasets[0].resource_id());
    let source = ActivitySourceId::default();
    app.session.background_activity.execution_source = Some(source);
    let plan = plan_tool(
        &app,
        ToolRequest {
            tool_id: "resource.rename".into(),
            tool_version: 1,
            parameters: serde_json::json!({"name":"Result"}),
            targets: TargetSelector::Explicit {
                ids: vec![resource.id.clone()],
            },
            expected_revision: DocumentRevision(app.doc.automation_revision),
            caller: CallerType::Agent,
        },
    )
    .unwrap();
    let run = PreparedToolRun::begin(&app, &plan);
    let replacement = ActivitySourceId::default();
    app.session.background_activity.execution_source = Some(replacement);
    run.finish(&mut app, &Ok(result(resource.clone()))).unwrap();
    assert_eq!(
        app.session.background_activity.results()[0].source,
        Some(source)
    );
    assert_eq!(
        app.session.background_activity.operations()[0].source,
        Some(source)
    );
    app.record_background_tool(
        app.session.project_context,
        CallerType::Agent,
        "resource.rename",
        Ok(&result(resource)),
    );
    let history = &app.session.background_activity;
    assert_eq!(history.results().len(), 1);
    assert_eq!(history.results()[0].source, Some(replacement));
    assert_eq!(history.results()[0].operation, history.latest_id());
}

#[test]
fn scientific_completion_uses_submission_source_instead_of_current_connection() {
    let mut app = PlotiumApp::new();
    app.new_table_dataset();
    let resource = ResourceRef::from(app.doc.datasets[0].resource_id());
    let source = ActivitySourceId::default();
    app.session.background_activity.execution_source = Some(source);
    let receipt = TaskReceipt {
        sources: Vec::new(),
        task_id: TaskId("source-task".into()),
        plan_id: None,
        operation: "processing.edit".into(),
        project_context: app.session.project_context,
        targets: vec![resource.clone()],
        start_revision: DocumentRevision(0),
        end_revision: None,
        status: TaskStatus::Queued,
        phase: "queued".into(),
        completed: None,
        total: None,
        failure_code: None,
        message: None,
        produced: Vec::new(),
        modified: vec![resource],
        summary: serde_json::Value::Null,
    };
    app.register_background_task(&receipt);
    app.session.scientific_tasks.register(receipt.clone());
    app.session.background_activity.execution_source = Some(ActivitySourceId::default());
    app.poll_background_activity();
    assert!(app.session.background_activity.results().is_empty());
    app.session.scientific_tasks.update(
        &receipt.task_id,
        TaskStatus::Succeeded,
        1,
        None,
        None,
        serde_json::Value::Null,
    );
    app.poll_background_activity();
    app.poll_background_activity();
    assert_eq!(app.session.background_activity.results().len(), 1);
    assert_eq!(
        app.session.background_activity.results()[0].source,
        Some(source)
    );
    assert_eq!(
        app.session.background_activity.operations()[0].task_id,
        Some(receipt.task_id)
    );
}

#[test]
fn source_does_not_make_read_only_or_old_project_results_followable() {
    let mut app = PlotiumApp::new();
    app.new_table_dataset();
    let resource = ResourceRef::from(app.doc.datasets[0].resource_id());
    app.session.background_activity.execution_source = Some(ActivitySourceId::default());
    let value = result(resource);
    app.record_background_tool(
        ProjectContextId::default(),
        CallerType::Agent,
        "resource.rename",
        Ok(&value),
    );
    app.record_background_tool(
        app.session.project_context,
        CallerType::Agent,
        "document.inspect",
        Ok(&value),
    );
    app.record_background_tool(
        app.session.project_context,
        CallerType::Human,
        "resource.rename",
        Ok(&value),
    );
    assert!(app.session.background_activity.results().is_empty());
}
