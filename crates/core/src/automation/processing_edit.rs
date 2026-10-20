//! Stable StepId edits for the existing NMR pipeline service.
use super::*;
use crate::actions::DatasetProcessingState;
use crate::project::{
    AxisPipelineDto, ProcessingStepDto, StepKindDto, StepSourceDto, pipeline_from_dto,
    pipeline_to_dto,
};
use crate::state::{Dataset, PlotiumApp};
use plotium_processing::StepId;
use serde::Deserialize;
use serde_json::{Value, json};

pub const PROCESSING_EDIT: &str = "processing.edit";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessingEditParams {
    pub edits: Vec<PipelineEdit>,
}
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum PipelineEdit {
    Replace {
        step: StepId,
        kind: StepKindDto,
    },
    Enable {
        step: StepId,
        enabled: bool,
    },
    Add {
        axis: String,
        kind: StepKindDto,
        before: Option<StepId>,
    },
    Remove {
        step: StepId,
    },
    Move {
        step: StepId,
        before: Option<StepId>,
    },
}
fn invalid(message: impl Into<String>) -> AutomationError {
    AutomationError::InvalidParameters {
        tool_id: PROCESSING_EDIT.into(),
        message: message.into(),
    }
}
fn validate_kind(kind: &StepKindDto) -> Result<(), AutomationError> {
    if let StepKindDto::ZeroFill(value) = kind {
        let valid = value == "none"
            || value
                .strip_prefix("factor")
                .is_some_and(|s| s.parse::<u8>().is_ok())
            || value
                .strip_prefix("size")
                .is_some_and(|s| s.parse::<usize>().is_ok());
        if !valid {
            return Err(invalid(
                "ZeroFill must be none, factor followed by a u8, or size followed by a nonnegative integer.",
            ));
        }
    }
    Ok(())
}
fn source<'a>(app: &'a PlotiumApp, plan: &ToolPlan) -> Result<&'a Dataset, AutomationError> {
    let [target] = plan.frozen_targets.targets.as_slice() else {
        return Err(invalid("Select exactly one dataset."));
    };
    if plan
        .targets
        .iter()
        .any(|t| t.status != TargetCompatibility::Compatible)
    {
        return Err(invalid("Dataset has no editable processing pipeline."));
    }
    app.doc
        .datasets
        .iter()
        .find(|d| d.resource_id().to_string() == target.id)
        .ok_or_else(|| invalid("Dataset no longer exists."))
}
type PipelineAxes = Vec<(&'static str, AxisPipelineDto)>;
fn pipelines(dataset: &Dataset) -> Result<(PipelineAxes, u64), AutomationError> {
    match dataset {
        Dataset::Nmr(n) => Ok((
            vec![("direct", pipeline_to_dto(&n.pipeline))],
            n.next_step_id,
        )),
        Dataset::Nmr2D(n) => Ok((
            vec![
                ("f2", pipeline_to_dto(&n.params.f2)),
                ("f1", pipeline_to_dto(&n.params.f1)),
            ],
            n.next_step_id,
        )),
        _ => Err(invalid("Dataset has no editable processing pipeline.")),
    }
}
fn locate(
    pipelines: &[(&str, AxisPipelineDto)],
    id: StepId,
) -> Result<(usize, usize), AutomationError> {
    pipelines
        .iter()
        .enumerate()
        .find_map(|(axis, p)| {
            p.1.steps
                .iter()
                .position(|s| s.id == Some(id.get()))
                .map(|step| (axis, step))
        })
        .ok_or_else(|| invalid(format!("Unknown StepId {}.", id.get())))
}
fn insertion(pipeline: &AxisPipelineDto, before: Option<StepId>) -> Result<usize, AutomationError> {
    before
        .map(|id| {
            pipeline
                .steps
                .iter()
                .position(|s| s.id == Some(id.get()))
                .ok_or_else(|| invalid("The insertion anchor must exist on the same axis."))
        })
        .unwrap_or(Ok(pipeline.steps.len()))
}

pub(super) fn prepare(app: &PlotiumApp, plan: &ToolPlan) -> Result<Value, AutomationError> {
    let dataset = source(app, plan)?;
    let params: ProcessingEditParams =
        super::registry::parse(PROCESSING_EDIT, plan.request.parameters.clone())?;
    if params.edits.is_empty() || params.edits.len() > 200 {
        return Err(invalid("Supply between 1 and 200 pipeline edits."));
    }
    let (mut axes, mut next_id) = pipelines(dataset)?;
    for edit in params.edits {
        match edit {
            PipelineEdit::Add { axis, kind, before } => {
                validate_kind(&kind)?;
                let pipeline = axes
                    .iter_mut()
                    .find(|p| p.0 == axis)
                    .ok_or_else(|| invalid("Axis must be direct, f2 or f1 for this dataset."))?;
                let position = insertion(&pipeline.1, before)?;
                let id = next_id;
                next_id = next_id
                    .checked_add(1)
                    .ok_or_else(|| invalid("StepId space exhausted."))?;
                pipeline.1.steps.insert(
                    position,
                    ProcessingStepDto {
                        id: Some(id),
                        kind,
                        enabled: true,
                        source: StepSourceDto::User,
                    },
                );
            }
            PipelineEdit::Replace { step, kind } => {
                validate_kind(&kind)?;
                let (a, s) = locate(&axes, step)?;
                axes[a].1.steps[s].kind = kind;
                axes[a].1.steps[s].source = StepSourceDto::User;
            }
            PipelineEdit::Enable { step, enabled } => {
                let (a, s) = locate(&axes, step)?;
                axes[a].1.steps[s].enabled = enabled;
            }
            PipelineEdit::Remove { step } => {
                let (a, s) = locate(&axes, step)?;
                axes[a].1.steps.remove(s);
            }
            PipelineEdit::Move { step, before } => {
                let (a, s) = locate(&axes, step)?;
                if before == Some(step) {
                    continue;
                }
                let value = axes[a].1.steps.remove(s);
                let position = insertion(&axes[a].1, before)?;
                axes[a].1.steps.insert(position, value);
            }
        }
    }
    let frozen = json!({"axes":axes.into_iter().map(|(axis,pipeline)|json!({"axis":axis,"pipeline":pipeline})).collect::<Vec<_>>(),"next_step_id":next_id});
    let state = state_from_frozen(dataset, &frozen)?;
    match (dataset, &state) {
        (Dataset::Nmr(n), DatasetProcessingState::Nmr { pipeline, .. }) => {
            pipeline
                .output_domain(n.input_domain())
                .map_err(|e| invalid(e.to_string()))?;
        }
        (Dataset::Nmr2D(n), DatasetProcessingState::Nmr2D { params, .. }) => {
            plotium_processing::nmr_execution::validate_2d_domains(&n.data, params)
                .map_err(invalid)?;
        }
        _ => return Err(invalid("Pipeline type mismatch.")),
    }
    Ok(frozen)
}
pub(super) fn state_from_frozen(
    dataset: &Dataset,
    frozen: &Value,
) -> Result<DatasetProcessingState, AutomationError> {
    let mut state = DatasetProcessingState::from_dataset(dataset);
    let axes = frozen["axes"]
        .as_array()
        .ok_or_else(|| invalid("Missing frozen axes."))?;
    for axis in axes {
        let dto: AxisPipelineDto =
            serde_json::from_value(axis["pipeline"].clone()).map_err(|e| invalid(e.to_string()))?;
        let pipeline = pipeline_from_dto(&dto);
        match (&mut state, axis["axis"].as_str()) {
            (DatasetProcessingState::Nmr { pipeline: p, .. }, Some("direct")) => *p = pipeline,
            (DatasetProcessingState::Nmr2D { params, .. }, Some("f2")) => params.f2 = pipeline,
            (DatasetProcessingState::Nmr2D { params, .. }, Some("f1")) => params.f1 = pipeline,
            _ => return Err(invalid("Frozen axis does not belong to this dataset.")),
        }
    }
    if let Some(value) = frozen["group_delay_correct"].as_bool() {
        match &mut state {
            DatasetProcessingState::Nmr {
                group_delay_correct,
                ..
            }
            | DatasetProcessingState::Nmr2D {
                group_delay_correct,
                ..
            } => *group_delay_correct = value,
            _ => {}
        }
    }
    if let (DatasetProcessingState::Nmr2D { params, .. }, Some(layout)) =
        (&mut state, frozen["layout"].as_str())
    {
        params.layout = crate::project::layout_from_str(layout);
    }
    Ok(state)
}
pub(super) fn execute(
    app: &mut PlotiumApp,
    plan: &ToolPlan,
) -> Result<ToolResult, AutomationError> {
    super::prepared_processing::execute(app, plan)
}

pub fn processing_edit_schema() -> Value {
    json!({"type":"object","required":["edits"],"additionalProperties":false,"properties":{"edits":{"type":"array","minItems":1,"maxItems":200,"items":{"oneOf":[
        edit_schema("replace",json!({"step":{"type":"integer","minimum":0},"kind":super::scientific_schema::step_kind()}),&["step","kind"]),
        edit_schema("enable",json!({"step":{"type":"integer","minimum":0},"enabled":{"type":"boolean"}}),&["step","enabled"]),
        edit_schema("add",json!({"axis":{"enum":["direct","f2","f1"]},"kind":super::scientific_schema::step_kind(),"before":{"type":["integer","null"],"minimum":0}}),&["axis","kind"]),
        edit_schema("remove",json!({"step":{"type":"integer","minimum":0}}),&["step"]),
        edit_schema("move",json!({"step":{"type":"integer","minimum":0},"before":{"type":["integer","null"],"minimum":0}}),&["step"])
    ]}}}})
}
fn edit_schema(op: &str, mut fields: Value, required: &[&str]) -> Value {
    fields["op"] = json!({"const":op});
    let mut keys = vec!["op"];
    keys.extend_from_slice(required);
    json!({"type":"object","properties":fields,"required":keys,"additionalProperties":false})
}
pub(super) fn descriptor() -> ToolDescriptor {
    ToolDescriptor {id:PROCESSING_EDIT.into(),version:1,title:"Edit processing pipeline".into(),description:"Edit parameters, enable, add, remove and reorder steps using stable StepIds. Proposed IDs are frozen in preflight.".into(),parameter_schema:processing_edit_schema(),result_schema:super::scientific_schema::task_result(),target_kinds:vec![ResourceKindId::new(KIND_DATASET)],required_capabilities:vec![CapabilityId::new(CAP_PROCESSING_SCHEME)],effect:EffectLevel::Reversible,undoable:true,deterministic:true,task_kind:"background".into()}
}
