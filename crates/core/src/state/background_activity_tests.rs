use super::*;
use crate::automation::{DocumentRevision, ResourceRef};

fn result(resource: ResourceRef, created: bool) -> ToolResult {
    ToolResult {
        tool_id: "resource.rename".into(),
        before_revision: DocumentRevision(0),
        after_revision: DocumentRevision(1),
        targets: Vec::new(),
        produced: if created {
            vec![resource.clone()]
        } else {
            Vec::new()
        },
        modified: vec![resource],
        diagnostics: Vec::new(),
        verification: Vec::new(),
        value: serde_json::Value::Null,
    }
}

fn record(app: &mut PlotiumApp, result: ToolResult) {
    app.record_background_tool(
        app.session.project_context,
        CallerType::Agent,
        "resource.rename",
        Ok(&result),
    );
}

#[test]
fn registered_tool_execution_records_once() {
    use crate::automation::{
        ExecutionAuthority, TargetSelector, ToolRequest, execute_tool, plan_tool,
    };
    let mut app = PlotiumApp::new();
    app.new_table_dataset();
    let resource = ResourceRef::from(app.doc.datasets[0].resource_id());
    let plan = plan_tool(
        &app,
        ToolRequest {
            tool_id: "resource.rename".into(),
            tool_version: 1,
            parameters: serde_json::json!({"name":"Updated result"}),
            targets: TargetSelector::Explicit {
                ids: vec![resource.id.clone()],
            },
            expected_revision: DocumentRevision(app.doc.automation_revision),
            caller: CallerType::Agent,
        },
    )
    .unwrap();
    execute_tool(&mut app, plan, ExecutionAuthority::ReversibleModify).unwrap();
    assert_eq!(app.session.background_activity.operations().len(), 1);
    assert_eq!(app.session.background_activity.results().len(), 1);
    assert_eq!(
        app.session.background_activity.results()[0].resource,
        resource
    );
    assert_eq!(
        app.session.background_activity.results()[0].name,
        "Updated result"
    );
}

#[test]
fn workflow_records_success_once_and_retains_node_planning_failures() {
    use crate::automation::{
        ExecutionAuthority, NodeCondition, NodeFailurePolicy, TargetSelector, TaskCancellation,
        WORKFLOW_SCHEMA, WorkflowDefinition, WorkflowFailurePolicy, WorkflowNode, execute_workflow,
    };
    let mut app = PlotiumApp::new();
    app.new_table_dataset();
    let resource = ResourceRef::from(app.doc.datasets[0].resource_id());
    let nodes = [resource.id.clone(), "missing-resource".into()]
        .into_iter()
        .enumerate()
        .map(|(index, id)| WorkflowNode {
            id: format!("rename-{index}"),
            tool_id: "resource.rename".into(),
            tool_version: 1,
            parameters: serde_json::json!({"name":"Workflow result"}),
            targets: TargetSelector::Explicit { ids: vec![id] },
            dependencies: if index == 0 {
                Vec::new()
            } else {
                vec!["rename-0".into()]
            },
            bindings: Vec::new(),
            condition: NodeCondition::Always,
            failure_policy: NodeFailurePolicy::Inherit,
        })
        .collect();
    let workflow = WorkflowDefinition {
        schema: WORKFLOW_SCHEMA.into(),
        inputs: BTreeMap::new(),
        nodes,
        failure_policy: WorkflowFailurePolicy::ContinueCompatible,
    };
    let manifest = execute_workflow(
        &mut app,
        &workflow,
        CallerType::Workflow,
        ExecutionAuthority::ReversibleModify,
        &TaskCancellation::default(),
        &mut |_| {},
    )
    .unwrap();
    assert_eq!(manifest.errors.len(), 1);
    let activity = &app.session.background_activity;
    assert_eq!(activity.operations().len(), 2);
    assert_eq!(activity.results().len(), 1);
    assert_eq!(activity.results()[0].updates, 1);
    assert_eq!(activity.results()[0].resource, resource);
    assert_eq!(activity.operations()[0].outcome, OperationOutcome::Success);
    assert_eq!(activity.operations()[1].outcome, OperationOutcome::Failure);
    assert!(activity.operations()[1].diagnostics[0].contains("missing-resource"));
}

#[test]
fn submitted_task_records_only_after_terminal_commit_and_only_once() {
    let mut app = PlotiumApp::new();
    app.new_table_dataset();
    let resource = ResourceRef::from(app.doc.datasets[0].resource_id());
    let receipt = TaskReceipt {
        sources: Vec::new(),
        task_id: TaskId("processing-1".into()),
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
        modified: vec![resource.clone()],
        summary: serde_json::Value::Null,
    };
    app.session.scientific_tasks.register(receipt.clone());
    let mut queued = result(resource, false);
    queued.value = serde_json::json!({"task":receipt});
    app.record_background_tool(
        app.session.project_context,
        CallerType::Agent,
        "processing.edit",
        Ok(&queued),
    );
    assert!(app.session.background_activity.operations().is_empty());
    app.session.clear_operation_history();
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
    assert_eq!(app.session.background_activity.operations().len(), 1);
    assert_eq!(app.session.background_activity.results().len(), 1);
}

#[test]
fn repeated_writes_coalesce_without_losing_created_identity_or_navigation() {
    let mut app = PlotiumApp::new();
    app.new_table_dataset();
    let resource = ResourceRef::from(app.doc.datasets[0].resource_id());
    let selected = app.active_dataset();
    let view = app.session.view;
    record(&mut app, result(resource.clone(), true));
    record(&mut app, result(resource.clone(), false));
    let history = &app.session.background_activity;
    assert_eq!(history.operations().len(), 2);
    assert_eq!(history.results().len(), 1);
    let row = history.results().back().unwrap();
    assert_eq!(row.change, ActivityChange::Created);
    assert_eq!(row.updates, 2);
    assert_eq!(row.resource, resource);
    assert_eq!(app.active_dataset(), selected);
    assert!(app.session.view == view);
}

#[test]
fn removed_result_does_not_retarget_replacement_at_same_index() {
    let mut app = PlotiumApp::new();
    app.new_table_dataset();
    let resource = ResourceRef::from(app.doc.datasets[0].resource_id());
    record(&mut app, result(resource.clone(), true));
    let name = app.session.background_activity.results()[0].name.clone();
    app.doc.datasets.clear();
    app.new_table_dataset();
    assert!(!app.activity_result_available(&resource));
    assert_eq!(
        app.session.background_activity.results()[0].resource,
        resource
    );
    assert_eq!(app.session.background_activity.results()[0].name, name);
}

#[test]
fn readonly_and_human_calls_do_not_create_activity() {
    let mut app = PlotiumApp::new();
    app.new_table_dataset();
    let value = result(ResourceRef::from(app.doc.datasets[0].resource_id()), false);
    for (caller, tool) in [
        (CallerType::Human, "resource.rename"),
        (CallerType::Agent, "document.inspect"),
    ] {
        app.record_background_tool(app.session.project_context, caller, tool, Ok(&value));
    }
    assert!(app.session.background_activity.operations().is_empty());
}

#[test]
fn failures_are_retained_without_inventing_results() {
    let mut app = PlotiumApp::new();
    app.record_background_tool(
        app.session.project_context,
        CallerType::Agent,
        "resource.rename",
        Err(&AutomationError::Execution(
            "Target no longer exists".into(),
        )),
    );
    let operation = app.session.background_activity.operations().back().unwrap();
    assert_eq!(operation.outcome, OperationOutcome::Failure);
    assert_eq!(operation.failed, 1);
    assert_eq!(operation.diagnostics.len(), 1);
    assert!(operation.diagnostics[0].contains("Target no longer exists"));
    assert!(app.session.background_activity.results().is_empty());
}

#[test]
fn old_project_completions_are_ignored() {
    let mut app = PlotiumApp::new();
    let context = app.session.project_context;
    app.session.project_context = ProjectContextId::default();
    app.record_background_tool(
        context,
        CallerType::Agent,
        "resource.rename",
        Err(&AutomationError::Execution("Old completion".into())),
    );
    assert!(app.session.background_activity.operations().is_empty());
}

#[test]
fn history_is_bounded_and_new_context_discards_old_rows() {
    let mut history = BackgroundActivity::default();
    let context = ProjectContextId::default();
    for index in 0..BACKGROUND_ACTIVITY_CAPACITY + 1 {
        let resource = ResourceRef::from(super::super::DatasetId::new());
        history.push(
            context,
            ActivityOperation {
                id: ActivityId(0),
                title: format!("Write {index}"),
                tool_id: "resource.rename".into(),
                outcome: OperationOutcome::Success,
                created: 1,
                updated: 0,
                failed: 0,
                skipped: 0,
                diagnostics: Vec::new(),
            },
            vec![(resource, format!("Result {index}"), ActivityChange::Created)],
        );
    }
    assert_eq!(history.results().len(), BACKGROUND_ACTIVITY_CAPACITY);
    assert_eq!(history.operations().len(), BACKGROUND_ACTIVITY_CAPACITY);
    history.push(
        ProjectContextId::default(),
        history.operations()[0].clone(),
        Vec::new(),
    );
    assert!(history.results().is_empty());
    assert_eq!(history.operations().len(), 1);
}
