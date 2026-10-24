use super::*;

pub(super) fn plan_response(id: &str, plan: &ToolPlan, app: &PlotiumApp) -> Value {
    let binding = plan.binding_plan.as_ref().expect("native creation plan");
    let creation = binding.creation.as_ref().expect("native creation summary");
    let diagnostics: Vec<_> = binding
        .bindings
        .iter()
        .filter_map(|b| {
            let d = &b.diagnostics;
            (d.missing_xy != 0
                || d.non_finite_xy != 0
                || d.missing_error != 0
                || !d.affected_rows.is_empty())
            .then(|| json!({"target":b.target,"diagnostics":d}))
        })
        .collect();
    let exceptional: Vec<_> = plan
        .targets
        .iter()
        .filter(|t| t.status != TargetCompatibility::Compatible)
        .collect();
    json!({
        "plan_id":id,"project_context":app.session.project_context,
        "basis_revision":plan.frozen_targets.revision,
        "preflight":{
            "required_authority":plan.required_authority,
            "created_target_count":creation.created.len(),
            "targets":exceptional,"outputs":plan.outputs,
            "binding_plan":{
                "creation":creation,"binding_count":binding.bindings.len(),
                "row_counts":{
                    "total_rows":binding.bindings.iter().map(|b| b.diagnostics.total_rows).sum::<usize>(),
                    "plotted_rows":binding.bindings.iter().map(|b| b.diagnostics.plotted_rows).sum::<usize>()
                },
                "bindings":diagnostics
            }
        }
    })
}

pub(super) fn execute_response(mut result: ToolResult) -> Value {
    let succeeded = result
        .targets
        .iter()
        .filter(|t| t.outcome == TargetOutcome::Succeeded)
        .count();
    let skipped = result
        .targets
        .iter()
        .filter(|t| t.outcome == TargetOutcome::Skipped)
        .count();
    let failed = result
        .targets
        .iter()
        .filter(|t| t.outcome == TargetOutcome::Failed)
        .count();
    result
        .targets
        .retain(|t| t.outcome != TargetOutcome::Succeeded);
    // Defaults are already disclosed before execution; bindings retain every Series target.
    if let Some(creation) = result
        .value
        .get_mut("creation")
        .and_then(Value::as_object_mut)
    {
        creation.remove("series_defaults");
    }
    let mut value = json!(result);
    value["summary"] = json!({"succeeded":succeeded,"skipped":skipped,"failed":failed});
    value
}

#[cfg(test)]
#[path = "creation_response_tests.rs"]
mod tests;
