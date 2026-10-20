//! Keep the existing synchronous scheme node complete before dependent nodes run.
use super::*;
use crate::state::PlotiumApp;

pub(super) fn execute_node(
    app: &mut PlotiumApp,
    plan: ToolPlan,
    authority: ExecutionAuthority,
    cancellation: &TaskCancellation,
) -> Result<ToolResult, AutomationError> {
    let task = (plan.request.tool_id == "processing.apply_scheme")
        .then(|| plan.task_id.clone())
        .flatten();
    let targets = plan.frozen_targets.targets.clone();
    let mut result = super::tools::execute_planned_tool(app, plan, authority)?;
    let Some(task) = task else {
        return Ok(result);
    };
    // This is a compatibility path for the synchronous executor, not an async
    // DAG scheduler. Use the same worker, cancellation and atomic commit guards
    // as a single operation, and never expose a queued node as successful.
    loop {
        if cancellation.is_cancelled() {
            app.cancel_scientific_task(&task);
        }
        app.poll_compute();
        let receipt = app.session.scientific_tasks.get(&task).ok_or_else(|| {
            AutomationError::Execution("Processing task receipt was lost.".into())
        })?;
        if receipt.status.terminal() {
            if receipt.status != TaskStatus::Succeeded {
                return Err(AutomationError::Execution(
                    receipt.message.clone().unwrap_or_else(|| {
                        format!("Processing task ended with {:?}.", receipt.status)
                    }),
                ));
            }
            result.after_revision = receipt.end_revision.unwrap_or(receipt.start_revision);
            result.produced = receipt.produced.clone();
            result.modified = receipt.modified.clone();
            result.targets = targets
                .into_iter()
                .map(|target| {
                    let applied = receipt.modified.contains(&target);
                    TargetResult {
                        target: TargetRef::resource(target),
                        outcome: if applied {
                            TargetOutcome::Succeeded
                        } else {
                            TargetOutcome::Skipped
                        },
                        message: if applied {
                            "Processing scheme applied."
                        } else {
                            "Incompatible processing target skipped."
                        }
                        .into(),
                        skip_reason: None,
                        fingerprints: Vec::new(),
                    }
                })
                .collect();
            result.value["task"] = serde_json::json!(receipt);
            return Ok(result);
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
