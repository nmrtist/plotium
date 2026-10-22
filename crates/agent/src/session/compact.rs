//! Shorter MCP responses for property writes. The stored plan and the core
//! result stay complete; only what the connection returns is reduced to the
//! rows a caller has to act on.
use plotium_core::automation::{
    TOOL_RESET, TOOL_SET, TOOL_SET_MANY, TargetCompatibility, TargetOutcome, ToolPlan, ToolResult,
};
use serde_json::{Map, Value, json};

pub(super) fn is_property_write(tool_id: &str) -> bool {
    matches!(tool_id, TOOL_SET | TOOL_SET_MANY | TOOL_RESET)
}

/// Preflight without the echoed request and frozen targets: counts, and only
/// the targets that will not be written.
pub(super) fn plan_response(id: &str, plan: &ToolPlan) -> Value {
    let count = |status| plan.targets.iter().filter(|t| t.status == status).count();
    let rows: Vec<_> = plan
        .targets
        .iter()
        .filter(|t| t.status != TargetCompatibility::Compatible)
        .collect();
    json!({
        "plan_id": id,
        "preflight": {
            "required_authority": plan.required_authority,
            "estimated_changes": plan.estimated_changes,
            "summary": {
                "compatible": count(TargetCompatibility::Compatible),
                "skipped": count(TargetCompatibility::Skipped),
                "failed": count(TargetCompatibility::Failed),
            },
            "targets": rows,
        }
    })
}

/// Execution result with a summary; succeeded rows are dropped from `targets`
/// and from a batch's `value.writes`.
pub(super) fn execute_response(result: ToolResult) -> Value {
    let mut by_reason = Map::new();
    let (mut succeeded, mut skipped, mut failed) = (0, 0, 0);
    for target in &result.targets {
        match target.outcome {
            TargetOutcome::Succeeded => succeeded += 1,
            TargetOutcome::Skipped => {
                skipped += 1;
                // Plan-time skips carry prose only; core leaves their reason empty.
                let reason = target.skip_reason.as_deref().unwrap_or("unspecified");
                let entry = by_reason.entry(reason).or_insert(json!(0));
                *entry = json!(entry.as_u64().unwrap_or(0) + 1);
            }
            TargetOutcome::Failed => failed += 1,
        }
    }
    let writes = result.value.get("writes").and_then(Value::as_array);
    // A batch reports one target row per write, in write order.
    let aligned = writes.is_some_and(|w| w.len() == result.targets.len());
    let writes: Option<Vec<Value>> = writes.map(|writes| {
        writes
            .iter()
            .enumerate()
            .filter(|(_, w)| w["outcome"] != "succeeded")
            .map(|(i, w)| {
                let mut w = w.clone();
                if aligned && let Some(reason) = &result.targets[i].skip_reason {
                    w["skip_reason"] = json!(reason);
                }
                w
            })
            .collect()
    });
    let targets: Vec<_> = result
        .targets
        .iter()
        .filter(|t| t.outcome != TargetOutcome::Succeeded)
        .collect();
    let mut value = json!({
        "tool_id": result.tool_id,
        "before_revision": result.before_revision,
        "after_revision": result.after_revision,
        "summary": {
            "succeeded": succeeded,
            "skipped": skipped,
            "failed": failed,
            "by_reason": by_reason,
        },
        "targets": targets,
    });
    if !result.modified.is_empty() {
        value["modified"] = json!(result.modified);
    }
    if !result.diagnostics.is_empty() {
        value["diagnostics"] = json!(result.diagnostics);
    }
    if !result.verification.is_empty() {
        value["verification"] = json!(result.verification);
    }
    if let Some(writes) = writes {
        value["value"] = json!({ "writes": writes });
    }
    value
}
