//! `xas.merge`: the desktop Merge Scans command for agents.
use super::*;
use crate::actions::Action;
use crate::state::{PlotiumApp, XasMergeAlignment};
use serde_json::json;

pub const XAS_MERGE: &str = "xas.merge";

fn invalid(message: impl Into<String>) -> AutomationError {
    AutomationError::InvalidParameters {
        tool_id: XAS_MERGE.into(),
        message: message.into(),
    }
}

fn merge_action(
    app: &PlotiumApp,
    plan: &ToolPlan,
) -> Result<(Action, XasMergeAlignment), AutomationError> {
    let scans = tools::compatible_targets(plan)
        .map(|target| {
            app.doc
                .datasets
                .iter()
                .position(|dataset| dataset.resource_id().to_string() == target.id)
                .ok_or_else(|| invalid(format!("dataset {} no longer exists", target.id)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut distinct = scans.clone();
    distinct.sort_unstable();
    distinct.dedup();
    if distinct.len() != scans.len() {
        return Err(invalid("ids must name each scan once"));
    }
    app.xas_merge_action(&scans).map_err(invalid)
}

/// Refuses what the merge would refuse, and states an unaligned result up front.
pub(super) fn preflight(app: &PlotiumApp, plan: &ToolPlan) -> Result<Vec<String>, AutomationError> {
    let (_, alignment) = merge_action(app, plan)?;
    Ok(match alignment {
        XasMergeAlignment::Aligned => Vec::new(),
        XasMergeAlignment::Unaligned(reason) => vec![format!("Scans stay unaligned: {reason}.")],
    })
}

pub(super) fn execute(
    app: &mut PlotiumApp,
    plan: &ToolPlan,
) -> Result<ToolResult, AutomationError> {
    let (action, alignment) = merge_action(app, plan)?;
    let mut result = tool_executors::commit_actions(app, plan, vec![action], "merged")?;
    result.value = match alignment {
        XasMergeAlignment::Aligned => json!({"alignment":"aligned"}),
        XasMergeAlignment::Unaligned(reason) => json!({"alignment":"unaligned","reason":reason}),
    };
    if let Some(dataset) = app.doc.datasets.last() {
        result.produced.push(dataset.resource_id().into());
    }
    if let Some(canvas) = app.doc.canvases.last() {
        result.produced.push(ResourceRef {
            id: canvas.resource_id.to_string(),
            kind: ResourceKindId::new(KIND_CANVAS),
            parent_id: None,
            local_id: None,
        });
    }
    Ok(result)
}
