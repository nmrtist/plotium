//! `dataset.delete`: undoable removal of datasets nothing draws from.
use super::*;
use crate::actions::Action;
use crate::state::PlotiumApp;

pub const DATASET_DELETE: &str = "dataset.delete";

/// One delete per compatible target, highest index first so each recorded
/// index still addresses the document when its step applies. A dataset still
/// used by a plot or derived dataset fails the whole request, naming the user.
pub(super) fn actions(app: &PlotiumApp, plan: &ToolPlan) -> Result<Vec<Action>, AutomationError> {
    let mut indices = tools::compatible_targets(plan)
        .map(|target| {
            app.doc
                .datasets
                .iter()
                .position(|dataset| dataset.resource_id().to_string() == target.id)
                .ok_or_else(|| {
                    AutomationError::Execution(format!("dataset {} no longer exists", target.id))
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if indices.is_empty() {
        return Err(AutomationError::InvalidParameters {
            tool_id: DATASET_DELETE.into(),
            message: "ids must name existing datasets".into(),
        });
    }
    indices.sort_unstable_by(|a, b| b.cmp(a));
    indices.dedup();
    let mut protein = app.doc.protein.clone();
    let mut actions = indices
        .into_iter()
        .map(|index| {
            protein.remove_dataset(app.doc.datasets[index].resource_id());
            app.delete_dataset_base_action(index).map_err(|message| {
                AutomationError::InvalidParameters {
                    tool_id: DATASET_DELETE.into(),
                    message,
                }
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if protein != app.doc.protein {
        actions.insert(0, app.protein_action(protein));
    }
    Ok(actions)
}

pub(super) fn execute(
    app: &mut PlotiumApp,
    plan: &ToolPlan,
) -> Result<ToolResult, AutomationError> {
    let actions = actions(app, plan)?;
    tool_executors::commit_actions(app, plan, actions, "deleted")
}
