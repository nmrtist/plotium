//! One call that plans and commits a reversible edit. It reuses the plan and
//! execute paths unchanged, so preflight, permissions, revision checks, Undo
//! and audit records are exactly those of the two-step form.
use super::*;

/// Reversible edits that write no files and start no background work.
/// Deletions keep the two-step form so callers review them first.
pub(crate) const APPLY_OPERATIONS: [&str; 10] = [
    TOOL_SET,
    TOOL_SET_MANY,
    TOOL_RESET,
    FIGURE_EDIT,
    PLOT_CREATE,
    "plot.series.append",
    "plot.series.bind",
    "plot.series.materialize_default",
    "resource.rename",
    "figure.apply_theme",
];

const DELETIONS: [&str; 3] = ["content_delete", "panel_delete", "series_delete"];

impl Session {
    pub(super) fn apply(
        &mut self,
        app: &mut PlotiumApp,
        arguments: Value,
        blocked: bool,
    ) -> Result<Value, Failure> {
        let tool_id = arguments.get("tool_id").and_then(Value::as_str);
        if !tool_id.is_some_and(|id| APPLY_OPERATIONS.contains(&id)) {
            return Err(Failure::new(
                "unsupported_operation",
                "plotium_apply accepts reversible edits only; plan this operation with plotium_plan and submit it with plotium_execute",
            )
            .at_field("tool_id"));
        }
        let deletes = arguments
            .pointer("/parameters/edits")
            .and_then(Value::as_array)
            .is_some_and(|edits| {
                edits
                    .iter()
                    .any(|e| e["op"].as_str().is_some_and(|op| DELETIONS.contains(&op)))
            });
        if tool_id == Some(FIGURE_EDIT) && deletes {
            return Err(Failure::new(
                "unsupported_operation",
                "Deletions are reviewed before they run; use plotium_plan, then plotium_execute",
            )
            .at_field("parameters.edits"));
        }
        if !self.allow_modify {
            return Err(Failure::new(
                "permission_denied",
                "Switch to Read and modify before applying edits",
            ));
        }
        let plan = self.call_inner(app, "plotium_plan", arguments, blocked)?;
        let id = plan["plan_id"]
            .as_str()
            .expect("plans have an id")
            .to_owned();
        self.call_inner(app, "plotium_execute", json!({ "plan_id": id }), blocked)
    }
}

#[cfg(test)]
#[path = "apply_tests.rs"]
mod tests;
