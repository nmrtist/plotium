//! Shared scientific inspection and reversible input operations.
use super::*;
use crate::state::{Dataset, Nmr2DDataset, PlotiumApp};
use crate::{DiffusionInput, DiffusionInputReport, DiffusionIssue, DiffusionReadiness};
use serde::Deserialize;
use serde_json::{Value, json};

pub const DIFFUSION_INSPECT: &str = "diffusion.inspect";
pub const DIFFUSION_SET_INPUT: &str = "diffusion.set_input";
pub const PROCESSING_INSPECT: &str = "processing.inspect";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiffusionInspectParams {
    #[serde(default)]
    pub input: Option<DiffusionInput>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiffusionSetInputParams {
    pub input: DiffusionInput,
}

/// Select one lifecycle copy before validation. An invalid explicit input must
/// remain visible; falling back would conceal a caller's scientific mistake.
pub fn inspect_diffusion_input(data: &Nmr2DDataset, explicit: Option<&DiffusionInput>) -> Value {
    let (_, resolution_source) = data.diffusion_input_candidate(explicit);
    let (count, report) = input_report(data, explicit);
    json!({"resolution_source": resolution_source, "increment_count": count,
        "input": report, "field_requirement": data.dosy_input_error(),
        "executable": report.executable && data.dosy_input_error().is_none(),
        "acquisition_evidence": data.data.jeol_diffusion.as_ref().map(|facts| json!({"experiment":facts.text("experiment"),"gradient_shape":facts.text("grad_shape"),"gradient_axis":facts.gradient_axis,"full_program_identity":facts.has_full_program_identity(),"timing_seconds":(["delta","delta_large","tau_1","tau_2","diffusion_time","grad_1","tau_e1","tau_e2","st_coef"].into_iter().map(|key| (key, facts.seconds(key))).collect::<std::collections::BTreeMap<_,_>>()) }))})
}

pub(super) fn resolve_input(
    data: &Nmr2DDataset,
    explicit: Option<&DiffusionInput>,
) -> Result<DiffusionInput, AutomationError> {
    data.resolve_diffusion_input(explicit)
        .map_err(|_| AutomationError::DiffusionInput {
            report: Box::new(input_report(data, explicit).1),
        })
}

fn input_report(
    data: &Nmr2DDataset,
    explicit: Option<&DiffusionInput>,
) -> (usize, DiffusionInputReport) {
    let count = match &data.processed {
        plotium_processing::Processed2D::Stack(stack) => stack.traces.len(),
        _ => data.data.rows,
    };
    let (input, _) = data.diffusion_input_candidate(explicit);
    let report = match input {
        Ok(input) => input.inspect(count),
        Err(message) => acquisition_report(data, message),
    };
    (count, report)
}

fn acquisition_report(data: &Nmr2DDataset, message: String) -> DiffusionInputReport {
    let mut issues = Vec::new();
    if let Some(facts) = &data.data.jeol_diffusion {
        for name in ["delta", "delta_large", "tau_1", "tau_2"] {
            if facts.seconds(name).is_none() {
                issues.push(DiffusionIssue {
                    code: "missing_parameters".into(),
                    path: format!("acquisition.{name}"),
                    expected_unit: Some("s".into()),
                    actual: Value::Null,
                    message: format!("JEOL {name} is absent or has unsupported units."),
                    evidence: "JEOL JDF parameter records".into(),
                    remedy: "Supply the duration explicitly in seconds or provide direct b values."
                        .into(),
                });
            }
        }
        for (path, actual, accepted) in [
            (
                "experiment",
                facts.text("experiment"),
                matches!(
                    facts.text("experiment"),
                    Some(
                        "bpp_dste_led_dif"
                            | "bpp_dste_led_diffusion"
                            | "bpp_dste_led_diffusion.jxp"
                    )
                ),
            ),
            (
                "grad_shape",
                facts.text("grad_shape"),
                facts.text("grad_shape") == Some("SQUARE"),
            ),
        ] {
            if !accepted {
                issues.push(DiffusionIssue { code: "evidence_required".into(), path: format!("acquisition.{path}"), expected_unit: None, actual: json!(actual),
                    message: format!("The JEOL {path} evidence does not establish the supported rectangular BPP-DSTE model."), evidence: "JEOL JDF parameter records".into(), remedy: "Review the actual pulse program and confirm an explicit supported model, or provide direct b values.".into() });
            }
        }
    }
    if issues.is_empty() {
        issues.push(DiffusionIssue { code: if data.data.jeol_diffusion.is_some() { "invalid_model_parameters" } else { "missing_parameters" }.into(), path: "input".into(), expected_unit: None, actual: Value::Null,
            message, evidence: "Acquisition metadata".into(), remedy: "Supply valid explicit model parameters and gradients, or direct b values bound to acquired IncrementIds.".into() });
    }
    let status = if issues.iter().any(|i| i.code == "missing_parameters") {
        DiffusionReadiness::MissingParameters
    } else if issues.iter().any(|i| i.code == "invalid_model_parameters") {
        DiffusionReadiness::InvalidInput
    } else {
        DiffusionReadiness::EvidenceRequired
    };
    DiffusionInputReport {
        status,
        executable: false,
        b_unit: "s/m²".into(),
        diffusion_unit: "m²/s".into(),
        input_unit: None,
        conversion_factor: None,
        source: "Acquisition metadata".into(),
        rule_version: 1,
        program_sha256: None,
        vendor_coefficient_seconds: data
            .data
            .jeol_diffusion
            .as_ref()
            .and_then(|facts| facts.seconds("st_coef")),
        b_values: Vec::new(),
        issues,
    }
}

pub(super) fn execute(
    app: &mut PlotiumApp,
    plan: &ToolPlan,
) -> Result<ToolResult, AutomationError> {
    let targets = plan.frozen_targets.targets.iter().collect::<Vec<_>>();
    if targets.len() != 1
        || plan
            .targets
            .iter()
            .any(|t| t.status != TargetCompatibility::Compatible)
    {
        return Err(AutomationError::InvalidSelector(
            "Select exactly one compatible scientific resource.".into(),
        ));
    }
    let target = targets[0];
    let id = if target.kind.0 == KIND_FIELD {
        target
            .parent_id
            .as_deref()
            .ok_or_else(|| AutomationError::InvalidSelector("Field has no owner.".into()))?
    } else {
        &target.id
    };
    let dataset = app
        .doc
        .datasets
        .iter()
        .find(|d| d.resource_id().to_string() == id)
        .ok_or_else(|| {
            AutomationError::InvalidSelector("Scientific target no longer exists.".into())
        })?;
    let mut result = match plan.request.tool_id.as_str() {
        DIFFUSION_INSPECT => {
            let params: DiffusionInspectParams =
                super::registry::parse(DIFFUSION_INSPECT, plan.request.parameters.clone())?;
            let data = dataset.as_nmr2d().ok_or_else(|| {
                AutomationError::InvalidSelector(
                    "Target does not expose diffusion encoding.".into(),
                )
            })?;
            super::tool_executors::readonly_value(
                DIFFUSION_INSPECT,
                plan.frozen_targets.revision,
                inspect_diffusion_input(data, params.input.as_ref()),
            )
        }
        DIFFUSION_SET_INPUT => {
            let params: DiffusionSetInputParams =
                super::registry::parse(DIFFUSION_SET_INPUT, plan.request.parameters.clone())?;
            let data = dataset.as_nmr2d().ok_or_else(|| {
                AutomationError::InvalidSelector(
                    "Target does not expose diffusion encoding.".into(),
                )
            })?;
            let report = params.input.inspect(data.data.rows);
            if !report.executable {
                return Err(AutomationError::DiffusionInput {
                    report: Box::new(report),
                });
            }
            let action = app
                .diffusion_input_action(data.resource_id, Some(params.input))
                .map_err(AutomationError::Execution)?;
            app.try_execute_action(action)
                .map_err(|e| AutomationError::Execution(e.to_string()))?;
            let mut result = super::tool_executors::readonly_value(
                DIFFUSION_SET_INPUT,
                plan.frozen_targets.revision,
                json!(report),
            );
            result.modified.push(target.clone());
            result
        }
        PROCESSING_INSPECT => {
            let axes = match dataset {
                Dataset::Nmr(n) => vec![
                    json!({"axis":"direct", "input_domain":format!("{:?}", n.input_domain()), "output_domain":format!("{:?}",n.output_domain()), "pipeline":crate::project::pipeline_to_dto(&n.pipeline), "valid":n.pipeline.output_domain(n.input_domain()).is_ok(), "pending_apply":false}),
                ],
                Dataset::Nmr2D(n) => [("f2", crate::state::PhaseAxis::F2, &n.params.f2), ("f1", crate::state::PhaseAxis::F1, &n.params.f1)]
                    .into_iter().map(|(axis, phase_axis, pipeline)| {
                        let input = n.input_domain(phase_axis);
                        let output = input.as_ref().map_err(Clone::clone).and_then(|domain|
                            pipeline.output_domain(*domain).map_err(|e| e.to_string()));
                        json!({"axis":axis,"input_domain":input.as_ref().ok().map(|d|format!("{d:?}")),
                            "output_domain":output.as_ref().ok().map(|d|format!("{d:?}")),"valid":output.is_ok(),
                            "invalid_reason":output.err(),"pipeline":crate::project::pipeline_to_dto(pipeline),
                            "pending_apply":n.base_stale || n.params != n.base_params})
                    }).collect(),
                Dataset::Xrd(x) => vec![json!({"axis":"2theta","settings":x.params,"valid":true,
                    "pending_apply":super::processing_edit_spectra::has_pending_edit(app, dataset)})],
                Dataset::Xas(x) => vec![json!({"axis":"energy","signal":x.signal,"settings":x.params,
                    "columns":x.column_summary(),"e0":x.edge_e0(),"edge_step":x.edge_step(),
                    "k_end":x.k_end(),"issues":x.issues(),"checks":x.checks(),
                    "calibration":x.calibration_summary(),"valid":x.has_spectrum(),
                    "pending_apply":super::processing_edit_spectra::has_pending_edit(app, dataset)})],
                Dataset::Xps(x) => x.experiment.regions.iter().map(|region| json!({
                    "axis":region.id.0.to_string(),"region":region.name,
                    "pipeline":{"steps":x.recipe(region.id).map(|recipe| &recipe.steps)},
                    "valid":x.displayed_region(region.id).is_some(),
                    "pending_apply":super::processing_edit_spectra::has_pending_edit(app, dataset)})).collect(),
                _ => {
                    return Err(AutomationError::InvalidSelector(
                        "Target has no editable processing pipeline.".into(),
                    ));
                }
            };
            super::tool_executors::readonly_value(
                PROCESSING_INSPECT,
                plan.frozen_targets.revision,
                json!({"dataset":dataset.resource_id(),"axes":axes}),
            )
        }
        _ => return Err(AutomationError::UnknownTool(plan.request.tool_id.clone())),
    };
    result.after_revision = DocumentRevision(app.doc.automation_revision);
    result.targets.push(TargetResult {
        target: TargetRef::resource(target.clone()),
        outcome: TargetOutcome::Succeeded,
        message: "Scientific operation completed.".into(),
        skip_reason: None,
        fingerprints: Vec::new(),
    });
    Ok(result)
}

pub fn diffusion_input_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["model","rule_version","source","confirmed","increments","gradients","vendor_coefficient_seconds","program_sha256"],"properties":{
        "model":{"oneOf":[
            {"type":"object","required":["model","unit"],"properties":{"model":{"const":"direct_b"},"unit":{"enum":["s/m²","s/cm²","s/mm²"]}},"additionalProperties":false},
            model_schema("rectangular_pgse", &["gamma","delta","big_delta","tau"]),
            model_schema("jeol_bpp_dste", &["gamma","delta","big_delta","tau_1","tau_2"]),
            model_schema("stejskal_tanner", &["gamma","delta","big_delta","tau","shape_factor"])
        ]},
        "rule_version":{"const":1},"source":{"type":"string"},"confirmed":{"type":"boolean"},
        "increments":{"type":"array","description":"b is always normalized to s/m²; id is the acquired IncrementId","items":{"type":"object","required":["id","b"],"properties":{"id":{"type":"integer","minimum":0},"b":{"type":"number","minimum":0}},"additionalProperties":false}},
        "gradients":{"type":["array","null"],"items":{"type":"number"},"description":"T/m in acquired increment order"},
        "vendor_coefficient_seconds":{"type":["number","null"]},"program_sha256":{"type":["string","null"]}
    }})
}
fn model_schema(model: &str, parameters: &[&str]) -> Value {
    let mut properties = serde_json::Map::new();
    properties.insert("model".into(), json!({"const":model}));
    for parameter in parameters {
        properties.insert((*parameter).into(), json!({"type":"number"}));
    }
    let mut required = vec!["model"];
    required.extend_from_slice(parameters);
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}

pub(super) fn descriptors() -> Vec<ToolDescriptor> {
    [(DIFFUSION_INSPECT,"Inspect diffusion encoding",false),(DIFFUSION_SET_INPUT,"Set diffusion encoding",true),(PROCESSING_INSPECT,"Inspect processing pipeline",false)].into_iter().map(|(id,title,write)| {
        let parameter_schema = if id == PROCESSING_INSPECT { json!({"type":"object","additionalProperties":false}) }
        else { json!({"type":"object","properties":{"input":diffusion_input_schema()},"required":if write {vec!["input"]}else{vec![]},"additionalProperties":false}) };
        ToolDescriptor { id:id.into(),version:1,title:title.into(),description:title.into(),parameter_schema,
            result_schema:json!({"type":"object","description":"ToolResult; value contains the scientific report","required":["tool_id","before_revision","after_revision","targets","value"]}),
            target_kinds: if write || id == PROCESSING_INSPECT { vec![ResourceKindId::new(KIND_DATASET)] } else { vec![ResourceKindId::new(KIND_DATASET),ResourceKindId::new(KIND_FIELD)] },
            required_capabilities:vec![CapabilityId::new(if id == PROCESSING_INSPECT {CAP_PROCESSING_SCHEME}else{CAP_DIFFUSION_INPUT})],
            effect:if write {EffectLevel::Reversible}else{EffectLevel::ReadOnly},undoable:write,deterministic:true,task_kind:"synchronous".into() }
    }).collect()
}

pub(super) fn validate_input_update(
    app: &PlotiumApp,
    plan: &ToolPlan,
) -> Result<(), AutomationError> {
    let [target] = plan.frozen_targets.targets.as_slice() else {
        return Err(AutomationError::InvalidSelector(
            "Select one diffusion dataset".into(),
        ));
    };
    let data = app
        .doc
        .datasets
        .iter()
        .find(|d| d.resource_id().to_string() == target.id)
        .and_then(|d| d.as_nmr2d())
        .ok_or_else(|| {
            AutomationError::InvalidSelector("Select a dataset with diffusion inputs".into())
        })?;
    let params: DiffusionSetInputParams =
        super::registry::parse(DIFFUSION_SET_INPUT, plan.request.parameters.clone())?;
    let count = match &data.processed {
        plotium_processing::Processed2D::Stack(stack) => stack.traces.len(),
        _ => data.data.rows,
    };
    let report = params.input.inspect(count);
    if !report.executable {
        return Err(AutomationError::DiffusionInput {
            report: Box::new(report),
        });
    }
    Ok(())
}
