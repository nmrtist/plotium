//! Shortened MCP responses for edits and queries. Stored plans and core
//! results stay complete; only what the connection returns is reduced.
use super::*;
use plotium_core::automation::{TargetCompatibility, TargetOutcome};
use serde_json::Map;

/// Edits whose plan and result otherwise come back whole. Property writes and
/// plot creation have their own shapes.
fn is_summarized_edit(tool_id: &str) -> bool {
    matches!(
        tool_id,
        FIGURE_EDIT
            | DATASET_DELETE
            | "resource.rename"
            | "figure.apply_theme"
            | "plot.series.append"
            | "plot.series.bind"
            | "plot.series.materialize_default"
            | "plot.field.create"
            | "analysis.diffusion.plot"
    )
}

pub(super) fn plan_response(id: &str, plan: &ToolPlan, app: &PlotiumApp) -> Value {
    let tool_id = plan.request.tool_id.as_str();
    if compact::is_property_write(tool_id) {
        return compact::plan_response(id, plan);
    }
    if tool_id == PLOT_CREATE {
        return creation_response::plan_response(id, plan, app);
    }
    if !is_summarized_edit(tool_id) {
        return json!({"plan_id":id,"preflight":plan});
    }
    let count = |status| plan.targets.iter().filter(|t| t.status == status).count();
    let mut preflight = json!(plan);
    let fields = preflight
        .as_object_mut()
        .expect("plan serializes as an object");
    // The caller sent the request; frozen identities stay in the stored plan.
    fields.remove("request");
    fields.remove("frozen_targets");
    fields.insert(
        "summary".into(),
        json!({
            "compatible": count(TargetCompatibility::Compatible),
            "skipped": count(TargetCompatibility::Skipped),
            "failed": count(TargetCompatibility::Failed),
        }),
    );
    fields.insert(
        "targets".into(),
        json!(
            plan.targets
                .iter()
                .filter(|t| t.status != TargetCompatibility::Compatible)
                .collect::<Vec<_>>()
        ),
    );
    if let Some(result) = preflight.pointer_mut("/figure_plan/result") {
        drop_frame_contract(result);
    }
    json!({
        "plan_id":id,"project_context":app.session.project_context,
        "basis_revision":plan.frozen_targets.revision,"preflight":preflight
    })
}

pub(super) fn execute_response(mut result: ToolResult) -> Value {
    if compact::is_property_write(&result.tool_id) {
        return compact::execute_response(result);
    }
    if result.tool_id == PLOT_CREATE {
        return creation_response::execute_response(result);
    }
    if !is_summarized_edit(&result.tool_id) {
        return json!(result);
    }
    let count = |outcome| {
        result
            .targets
            .iter()
            .filter(|t| t.outcome == outcome)
            .count()
    };
    let summary = json!({
        "succeeded": count(TargetOutcome::Succeeded),
        "skipped": count(TargetOutcome::Skipped),
        "failed": count(TargetOutcome::Failed),
    });
    // Succeeded rows repeat `produced` and `modified`.
    result
        .targets
        .retain(|t| t.outcome != TargetOutcome::Succeeded);
    drop_frame_contract(&mut result.value);
    let mut value = json!(result);
    value["summary"] = summary;
    value
}

/// Static coordinate prose; the figure.edit result contract states the same rules.
fn drop_frame_contract(value: &mut Value) {
    if let Some(fields) = value.as_object_mut() {
        fields.remove("frame_contract");
    }
}

/// One reading per target repeats the property's schema and default; lift
/// them out when every reading shares them, and drop succeeded target rows.
pub(super) fn inspect_response(mut result: ToolResult) -> Value {
    result
        .targets
        .retain(|t| t.outcome != TargetOutcome::Succeeded);
    if let Some(readings) = result
        .value
        .get_mut("readings")
        .and_then(Value::as_array_mut)
    {
        let mut shared = Map::new();
        for key in ["schema", "default_value"] {
            let first = readings.first().and_then(|r| r.get(key)).cloned();
            if let Some(first) = first
                && readings.len() > 1
                && readings.iter().all(|r| r.get(key) == Some(&first))
            {
                for reading in readings.iter_mut() {
                    reading.as_object_mut().map(|r| r.remove(key));
                }
                shared.insert(key.into(), first);
            }
        }
        if let Some(value) = result.value.as_object_mut() {
            value.extend(shared);
        }
    }
    json!(result)
}

/// An unfiltered search explains every match with the same reason; state it once.
pub(super) fn query_response(mut result: ToolResult) -> Value {
    if result.tool_id == "resources.search"
        && let Some(reasons) = result.value.get("reasons").and_then(Value::as_array)
        && let Some(first) = reasons.first().map(|r| r["reason"].clone())
        && reasons.iter().all(|r| r["reason"] == first)
        && let Some(value) = result.value.as_object_mut()
    {
        value.remove("reasons");
        value.insert("reason".into(), first);
    }
    json!(result)
}

/// Compact diagnostic pages keep findings and coverage status; budgets, page
/// geometry prose, the preview template and static check lists are full detail.
pub(crate) fn diagnostic_page(value: &mut Value) {
    let Some(fields) = value.as_object_mut() else {
        return;
    };
    for key in ["budget", "preview_request", "read_policy"] {
        fields.remove(key);
    }
    if let Some(output) = fields.get_mut("output").and_then(Value::as_object_mut) {
        output.retain(|key, _| matches!(key.as_str(), "size_mm" | "size_pt"));
    }
    if let Some(coverage) = fields.get_mut("coverage").and_then(Value::as_object_mut) {
        for key in ["requested_checks", "supported_checks", "unsupported_checks"] {
            coverage.remove(key);
        }
    }
}

#[cfg(test)]
#[path = "summaries_tests.rs"]
mod tests;
