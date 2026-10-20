//! Freeze recipes, compute isolated candidates, then install one undo snapshot.
use super::*;
use crate::actions::{Action, DatasetProcessingState};
use crate::state::*;
use serde_json::{Value, json};

fn invalid(message: impl Into<String>) -> AutomationError {
    AutomationError::Execution(message.into())
}
fn freeze_state(state: &DatasetProcessingState) -> Result<Value, AutomationError> {
    match state {
        DatasetProcessingState::Nmr {
            pipeline,
            group_delay_correct,
        } => Ok(
            json!({"axes":[{"axis":"direct","pipeline":crate::project::pipeline_to_dto(pipeline)}],"group_delay_correct":group_delay_correct}),
        ),
        DatasetProcessingState::Nmr2D {
            params,
            group_delay_correct,
            ..
        } => Ok(
            json!({"axes":[{"axis":"f2","pipeline":crate::project::pipeline_to_dto(&params.f2)},{"axis":"f1","pipeline":crate::project::pipeline_to_dto(&params.f1)}],"group_delay_correct":group_delay_correct,"layout":crate::project::layout_to_str(params.layout)}),
        ),
        _ => Err(invalid("This scheme has no native processing adapter.")),
    }
}
pub(super) fn prepare_scheme(app: &PlotiumApp, plan: &ToolPlan) -> Result<Value, AutomationError> {
    let params: super::registry::SchemeParams =
        super::registry::parse("processing.apply_scheme", plan.request.parameters.clone())?;
    let frozen = plan
        .frozen_scheme
        .as_ref()
        .ok_or_else(|| invalid("Missing frozen scheme."))?;
    let scheme = frozen.decode()?;
    let mut targets = Vec::new();
    let mut skipped = Vec::new();
    let mut incompatible_scheme = false;
    for target in &plan.frozen_targets.targets {
        if let Some(compatibility) = plan.targets.iter().find(|t| t.target.resource == *target)
            && compatibility.status != TargetCompatibility::Compatible
        {
            skipped.push(json!({"target":target,"reason":compatibility.reason}));
            continue;
        }
        let Some(source) = app
            .doc
            .datasets
            .iter()
            .find(|d| d.resource_id().to_string() == target.id)
        else {
            return Err(invalid("The processing target no longer exists."));
        };
        let next = match source {
            Dataset::Nmr(n) => n.next_step_id,
            Dataset::Nmr2D(n) => n.next_step_id,
            _ => 0,
        };
        let count = scheme
            .pipelines
            .iter()
            .map(|p| p.steps.len() as u64)
            .sum::<u64>();
        if next.checked_add(count).is_none() {
            return Err(invalid("Processing step identity space exhausted."));
        }
        match crate::project::apply_scheme(&scheme, source) {
            Ok(state) => targets.push(
                json!({"dataset":source.resource_id(),"configuration":freeze_state(&state)?}),
            ),
            Err(error) => {
                incompatible_scheme = true;
                skipped.push(json!({"target":target,"reason":error.to_string()}));
            }
        }
    }
    if targets.is_empty() || (!params.compatible_only && incompatible_scheme) {
        return Err(invalid(
            "No compatible processing targets, or strict policy rejected the selection.",
        ));
    }
    Ok(
        json!({"targets":targets,"skipped":skipped,"scheme_sha256":frozen.sha256,"scheme":frozen.content}),
    )
}
pub(super) fn execute(
    app: &mut PlotiumApp,
    plan: &ToolPlan,
) -> Result<ToolResult, AutomationError> {
    let preparation = if plan.request.tool_id == PROCESSING_EDIT {
        let target = plan
            .frozen_targets
            .targets
            .first()
            .ok_or_else(|| invalid("Missing target."))?;
        json!({"targets":[{"dataset":target.id,"configuration":plan.processing_edit.as_ref().ok_or_else(||invalid("Missing frozen pipeline."))?}]})
    } else {
        plan.scientific_preparation
            .clone()
            .ok_or_else(|| invalid("Missing frozen processing targets."))?
    };
    let targets = preparation["targets"]
        .as_array()
        .ok_or_else(|| invalid("Invalid frozen target list."))?;
    let mut recipes = Vec::new();
    let mut snapshots = Vec::new();
    for target in targets {
        let id: DatasetId = serde_json::from_value(target["dataset"].clone())
            .map_err(|e| invalid(e.to_string()))?;
        let source = app
            .doc
            .dataset_index(id)
            .map(|i| &app.doc.datasets[i])
            .ok_or_else(|| invalid("The processing target no longer exists."))?;
        if let Some(kind) = app.session.compute.blocking_work_for(id) {
            return Err(invalid(format!("{} is already running.", kind.label())));
        }
        let recipe = super::processing_edit::state_from_frozen(source, &target["configuration"])?;
        recipes.push((id, recipe));
        snapshots.push(source.clone());
        if let Some(table) = app.doc.datasets.iter().find(|d| {
            d.as_table()
                .and_then(|t| t.provenance.as_ref())
                .is_some_and(|p| p.source_resource == id.to_string())
        }) {
            snapshots.push(table.clone());
        }
    }
    let dataset = recipes
        .first()
        .ok_or_else(|| invalid("No processing targets."))?
        .0;
    let settings = app.settings.clone();
    let summary = preparation.clone();
    let work: ScientificWork = Box::new(move |token| {
        let before = snapshots.clone();
        let mut worker = PlotiumApp::with_compute(settings, ComputeService::inactive());
        worker.doc.datasets = snapshots;
        for (id, recipe) in recipes {
            if token.is_cancelled() {
                return Err("Processing cancelled".into());
            }
            let index = worker
                .doc
                .dataset_index(id)
                .ok_or("Processing source missing")?;
            recipe
                .apply_to(&mut worker.doc.datasets[index])
                .map_err(|e| e.to_string())?;
            if let Some(data) = worker.doc.datasets[index].as_nmr2d_mut() {
                data.invalidate_dosy_results("Processing changed the source field");
            }
            worker.sync_region_table(index);
        }
        let after = worker.doc.datasets.to_vec();
        Ok(ScientificOutput {
            create_canvas: false,
            action: Box::new(Action::InstallScientificState { before, after }),
            summary,
            produced: vec![],
        })
    });
    let receipt = TaskReceipt {
        sources: Vec::new(),
        task_id: plan
            .task_id
            .clone()
            .ok_or_else(|| invalid("TaskId is missing."))?,
        plan_id: plan.submitted_plan_id.clone(),
        operation: plan.request.tool_id.clone(),
        project_context: app.session.project_context,
        targets: plan.frozen_targets.targets.clone(),
        start_revision: plan.frozen_targets.revision,
        end_revision: None,
        status: TaskStatus::Queued,
        phase: "queued".into(),
        completed: None,
        total: None,
        failure_code: None,
        message: None,
        produced: vec![],
        modified: targets
            .iter()
            .map(|t| {
                serde_json::from_value::<DatasetId>(t["dataset"].clone()).map(ResourceRef::from)
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| invalid(e.to_string()))?,
        summary: Value::Null,
    };
    let receipt = app
        .start_prepared_scientific(dataset, receipt, work)
        .map_err(invalid)?;
    Ok(super::tool_executors::readonly_value(
        &plan.request.tool_id,
        plan.frozen_targets.revision,
        json!({"task":receipt,"preparation":preparation}),
    ))
}
