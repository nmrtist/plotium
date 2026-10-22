use super::*;
use serde_json::Value;

fn receipt(name: &str) -> TaskReceipt {
    TaskReceipt {
        sources: vec![],
        task_id: TaskId(name.into()),
        plan_id: Some(name.into()),
        operation: "analysis.test".into(),
        project_context: Default::default(),
        targets: vec![],
        start_revision: DocumentRevision(0),
        end_revision: None,
        status: TaskStatus::Queued,
        phase: "queued".into(),
        completed: None,
        total: None,
        failure_code: None,
        message: None,
        produced: vec![],
        modified: vec![],
        summary: Value::Null,
    }
}
#[test]
fn terminal_retention_preserves_active_work_and_never_replays_evicted_ids() {
    let mut tasks = ScientificTasks::default();
    assert!(tasks.register(receipt("active")));
    assert!(tasks.register(receipt("late_completion")));
    for index in 0..130 {
        let name = format!("task_{index}");
        assert!(tasks.register(receipt(&name)));
        tasks.update(
            &TaskId(name),
            TaskStatus::Succeeded,
            1,
            None,
            None,
            Value::Null,
        );
    }
    assert!(tasks.get(&TaskId("active".into())).is_some());
    assert!(tasks.get(&TaskId("task_0".into())).is_none());
    assert!(tasks.was_consumed(&TaskId("task_0".into())));
    assert!(tasks.plan_was_consumed("task_0"));
    assert!(tasks.for_plan("task_0").is_none());
    assert!(!tasks.register(receipt("task_0")));
    tasks.update(
        &TaskId("late_completion".into()),
        TaskStatus::Succeeded,
        2,
        None,
        None,
        Value::Null,
    );
    assert_eq!(tasks.list(0, 1)[0].task_id.0, "late_completion");
    assert_eq!(tasks.list(128, 128)[0].task_id.0, "active");
}
#[test]
fn cancellation_is_terminal_and_project_invalidation_keeps_history() {
    let mut tasks = ScientificTasks::default();
    assert!(tasks.register(receipt("cancelled")));
    tasks.update(
        &TaskId("cancelled".into()),
        TaskStatus::Cancelled,
        1,
        Some("request_cancelled"),
        None,
        Value::Null,
    );
    tasks.update(
        &TaskId("cancelled".into()),
        TaskStatus::Succeeded,
        2,
        None,
        None,
        Value::Null,
    );
    assert_eq!(tasks.list(0, 1)[0].status, TaskStatus::Cancelled);
    assert!(tasks.register(receipt("active")));
    tasks.invalidate_project(3);
    assert_eq!(tasks.list(0, 1)[0].status, TaskStatus::Stale);
    assert_eq!(
        tasks.get(&TaskId("cancelled".into())).unwrap().status,
        TaskStatus::Cancelled
    );
}

#[test]
fn scientific_warning_preserves_successful_execution_without_claiming_valid_science() {
    let mut task = receipt("fit");
    task.status = TaskStatus::Succeeded;
    task.summary = serde_json::json!({"converged":false});
    assert!(
        task.scientific_warning()
            .unwrap()
            .contains("did not converge")
    );
    task.summary = serde_json::json!({"status":"empty","accepted_columns":0});
    assert!(
        task.scientific_warning()
            .unwrap()
            .contains("no accepted result")
    );
    task.summary = serde_json::json!({"converged":true,"status":"available"});
    assert!(task.scientific_warning().is_none());
    task.status = TaskStatus::Failed;
    task.summary = serde_json::json!({"converged":false});
    assert!(task.scientific_warning().is_none());
}
