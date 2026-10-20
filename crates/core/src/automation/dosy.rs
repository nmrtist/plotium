//! Background DOSY submission and bounded result observation.
use super::*;
use crate::state::{Nmr2DDataset, PlotiumApp};
use crate::{DiffusionInput, IltParams};
use serde::Deserialize;
use serde_json::json;

pub const DOSY_BUILD: &str = "analysis.dosy.build";
pub const ANALYSIS_RESULTS_INSPECT: &str = "analysis.results.inspect";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DosyBuildParams {
    pub method: String,
    pub params: Option<IltParams>,
    pub input: Option<DiffusionInput>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisResultsParams {
    pub method: Option<String>,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "page_size")]
    pub limit: usize,
}
fn page_size() -> usize {
    100
}
fn error(message: impl Into<String>) -> AutomationError {
    AutomationError::InvalidParameters {
        tool_id: DOSY_BUILD.into(),
        message: message.into(),
    }
}
fn source<'a>(app: &'a PlotiumApp, plan: &ToolPlan) -> Result<&'a Nmr2DDataset, AutomationError> {
    let [target] = plan.frozen_targets.targets.as_slice() else {
        return Err(error("Select exactly one diffusion source field."));
    };
    if plan
        .targets
        .iter()
        .any(|t| t.status != TargetCompatibility::Compatible)
    {
        return Err(error(
            "Target does not expose the required scientific capability.",
        ));
    }
    let owner = target.parent_id.as_deref().unwrap_or(&target.id);
    app.doc
        .datasets
        .iter()
        .find(|d| d.resource_id().to_string() == owner)
        .and_then(|d| d.as_nmr2d())
        .ok_or_else(|| error("The diffusion source no longer exists."))
}
pub(super) fn resolved_ilt_params(
    app: &PlotiumApp,
    plan: &ToolPlan,
) -> Result<Option<IltParams>, AutomationError> {
    let params: DosyBuildParams =
        super::registry::parse(DOSY_BUILD, plan.request.parameters.clone())?;
    if params.method != "ilt" {
        return Ok(None);
    }
    let data = source(app, plan)?;
    let index = app
        .doc
        .dataset_index(data.resource_id)
        .ok_or_else(|| error("Source no longer exists."))?;
    let resolved = app.resolve_ilt_params_for(index, params.params);
    crate::state::validate_ilt_params(resolved).map_err(error)?;
    Ok(Some(resolved))
}
pub(super) fn prepare(
    app: &PlotiumApp,
    plan: &ToolPlan,
) -> Result<DiffusionInput, AutomationError> {
    let params: DosyBuildParams =
        super::registry::parse(DOSY_BUILD, plan.request.parameters.clone())?;
    let data = source(app, plan)?;
    if let Some(reason) = data.dosy_input_error() {
        return Err(error(reason));
    }
    if app
        .session
        .compute
        .blocking_work_for(data.resource_id)
        .is_some()
    {
        return Err(error(
            "Wait for the active source computation before planning DOSY.",
        ));
    }
    match params.method.as_str() {
        "mono_exp" if params.params.is_none() => {}
        "ilt" => {
            resolved_ilt_params(app, plan)?;
        }
        _ => {
            return Err(error(
                "Use mono_exp without ILT parameters, or ilt with optional explicit parameters.",
            ));
        }
    }
    super::science::resolve_input(data, params.input.as_ref())
}
pub(super) fn execute(
    app: &mut PlotiumApp,
    plan: &ToolPlan,
) -> Result<ToolResult, AutomationError> {
    let params: DosyBuildParams =
        super::registry::parse(DOSY_BUILD, plan.request.parameters.clone())?;
    let data = source(app, plan)?;
    let id = data.resource_id;
    let task_id = plan
        .task_id
        .clone()
        .ok_or_else(|| error("The preflight has no TaskId."))?;
    let input = plan
        .scientific_input
        .clone()
        .ok_or_else(|| error("Diffusion input was not frozen during preflight."))?;
    let params = if params.method == "ilt" {
        Some(
            plan.scientific_ilt_params
                .ok_or_else(|| error("ILT parameters were not frozen during preflight."))?,
        )
    } else {
        None
    };
    let receipt = app
        .start_dosy_task(
            id,
            task_id,
            plan.submitted_plan_id.clone(),
            params,
            Some(input),
        )
        .map_err(error)?;
    Ok(super::tool_executors::readonly_value(
        DOSY_BUILD,
        plan.frozen_targets.revision,
        json!({"task":receipt}),
    ))
}
pub(super) fn inspect(app: &PlotiumApp, plan: &ToolPlan) -> Result<ToolResult, AutomationError> {
    let params: AnalysisResultsParams =
        super::registry::parse(ANALYSIS_RESULTS_INSPECT, plan.request.parameters.clone())?;
    if params.limit == 0 || params.limit > 100 {
        return Err(error("Result pages require limit 1..=100."));
    }
    if let Some(value) = super::region_fit::inspect_table(app, plan, &params)? {
        return Ok(super::tool_executors::readonly_value(
            ANALYSIS_RESULTS_INSPECT,
            plan.frozen_targets.revision,
            value,
        ));
    }
    let data = source(app, plan)?;
    let target_method = match plan
        .frozen_targets
        .targets
        .first()
        .and_then(|target| target.local_id.as_deref())
    {
        Some("nmr.dosy_map") => "mono_exp",
        Some("nmr.ilt_map") => "ilt",
        _ => return Err(error("Select a DOSY result field.")),
    };
    if params
        .method
        .as_deref()
        .is_some_and(|method| method != target_method)
    {
        return Err(error(
            "The requested method conflicts with the result field identity.",
        ));
    }
    let method = target_method;
    let value = match method {
        "mono_exp" => {
            let map = data
                .dosy_map
                .as_ref()
                .ok_or_else(|| error("No mono-exponential result is installed."))?;
            let accepted = map.d.iter().filter(|v| v.is_finite()).count();
            let diagnostics = data
                .dosy_provenance
                .as_ref()
                .map(|p| p.columns.as_slice())
                .unwrap_or_default();
            let columns=map.d.iter().enumerate().skip(params.offset).take(params.limit).map(|(index,d)|json!({"column":index,"ppm":map.ppm.get(index),"d":d.is_finite().then_some(d),"map_amplitude":map.amp.get(index),"diagnostic":diagnostics.get(index),"diagnostic_unavailable_reason":diagnostics.get(index).is_none().then_some("No column diagnostic was recorded for this result.")})).collect::<Vec<_>>();
            json!({"method":method,"status":if accepted==0{"empty"}else{"available"},"accepted_columns":accepted,"rejected_columns":map.d.len()-accepted,"rejection_reasons":rejection_reasons(diagnostics),"columns":columns,"total":map.d.len(),"provenance":provenance_summary(data.dosy_provenance.as_ref())})
        }
        "ilt" => {
            let map = data
                .ilt_map
                .as_ref()
                .ok_or_else(|| error("No ILT result is installed."))?;
            let accepted = map
                .amp
                .iter()
                .filter(|column| column.iter().any(|v| *v > 0.))
                .count();
            let diagnostics = data
                .ilt_provenance
                .as_ref()
                .map(|p| p.columns.as_slice())
                .unwrap_or_default();
            let columns=map.amp.iter().enumerate().skip(params.offset).take(params.limit).map(|(index,amp)|json!({"column":index,"ppm":map.ppm.get(index),"nonnegative_amplitudes":amp,"diagnostic":diagnostics.get(index),"diagnostic_unavailable_reason":diagnostics.get(index).is_none().then_some("No column diagnostic was recorded for this result.")})).collect::<Vec<_>>();
            json!({"method":method,"status":if accepted==0{"empty"}else{"available"},"accepted_columns":accepted,"rejected_columns":map.amp.len()-accepted,"rejection_reasons":rejection_reasons(diagnostics),"d_grid":map.d_grid,"columns":columns,"total":map.amp.len(),"provenance":provenance_summary(data.ilt_provenance.as_ref())})
        }
        _ => return Err(error("Result method must be mono_exp or ilt.")),
    };
    let provenance = if method == "mono_exp" {
        data.dosy_provenance.as_ref()
    } else {
        data.ilt_provenance.as_ref()
    };
    let valid = match (
        &data.processed,
        provenance.and_then(|p| p.encoding.as_ref()),
        provenance,
    ) {
        (plotium_processing::Processed2D::Stack(stack), Some(input), Some(p)) => {
            crate::state::dosy_data_fingerprint(stack, input) == p.data_fingerprint
        }
        _ => false,
    };
    let mut value = value;
    value["valid"] = json!(valid);
    value["diffusion_unit"] = json!("m²/s");
    value["offset"] = json!(params.offset);
    value["limit"] = json!(params.limit);
    Ok(super::tool_executors::readonly_value(
        ANALYSIS_RESULTS_INSPECT,
        plan.frozen_targets.revision,
        value,
    ))
}
pub(super) fn descriptors() -> Vec<ToolDescriptor> {
    [(DOSY_BUILD,true),(ANALYSIS_RESULTS_INSPECT,false)].into_iter().map(|(id,write)|ToolDescriptor {
        id:id.into(),version:1,title:id.into(),description:if write{"Compute a mono-exponential or ILT DOSY map in the shared worker pool; returns a retained task receipt."}else{"Read bounded pages of native DOSY results and provenance."}.into(),
        parameter_schema:if write{json!({"type":"object","required":["method"],"additionalProperties":false,"properties":{"method":{"enum":["mono_exp","ilt"]},"params":{"type":"object","properties":{"d_min":{"type":"number"},"d_max":{"type":"number"},"n_grid":{"type":"integer"},"lambda":{"type":"number"}},"required":["d_min","d_max","n_grid","lambda"],"additionalProperties":false},"input":diffusion_input_schema()}})}else{json!({"type":"object","additionalProperties":false,"properties":{"method":{"enum":["mono_exp","ilt"]},"offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":100}}})},
        result_schema:if write { super::scientific_schema::task_result() } else { json!({"type":"object","required":["value"],"properties":{"value":{"type":"object","required":["method","status","valid","offset","limit"],"properties":{"method":{"enum":["mono_exp","ilt"]},"status":{"enum":["available","empty"]},"valid":{"type":"boolean"},"columns":{"type":"array"},"analyses":{"type":"array"},"accepted_columns":{"type":"integer"},"rejected_columns":{"type":"integer"},"rejection_reasons":{"type":"object"},"provenance":{"type":["object","null"]},"offset":{"type":"integer"},"limit":{"type":"integer"}}}}}) },
        target_kinds:if write{vec![ResourceKindId::new(KIND_FIELD)]}else{vec![ResourceKindId::new(KIND_FIELD),ResourceKindId::new(KIND_DATASET),ResourceKindId::new(KIND_ANALYSIS)]},required_capabilities:vec![CapabilityId::new(if write{CAP_DIFFUSION_INPUT}else{CAP_DIFFUSION_RESULT})],effect:if write{EffectLevel::Reversible}else{EffectLevel::ReadOnly},undoable:write,deterministic:true,task_kind:if write{"background"}else{"synchronous"}.into(),
    }).collect()
}

fn provenance_summary(provenance: Option<&crate::DosyResultProvenance>) -> serde_json::Value {
    provenance.map_or(serde_json::Value::Null,|p|json!({"algorithm":p.algorithm,"version":p.version,"source_fields":p.source_fields,"input":p.input,"data_fingerprint":p.data_fingerprint,"encoding":p.encoding}))
}
fn rejection_reasons(
    columns: &[plotium_analysis::column_diagnostics::ColumnDiagnostic],
) -> std::collections::BTreeMap<&str, usize> {
    let mut reasons = std::collections::BTreeMap::new();
    for column in columns.iter().filter(|c| !c.accepted) {
        *reasons
            .entry(column.reason.as_deref().unwrap_or("unspecified"))
            .or_default() += 1;
    }
    reasons
}
