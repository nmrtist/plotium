//! Frozen region extraction and the native b-value decay model.
use super::*;
use crate::actions::Action;
use crate::state::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const REGIONS_EXTRACT: &str = "analysis.regions.extract";
pub const DIFFUSION_FIT: &str = "analysis.diffusion.fit";

#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "source", rename_all = "snake_case", deny_unknown_fields)]
pub enum RegionSpec {
    Existing {
        id: RegionId,
    },
    Explicit {
        bounds: [f64; 2],
        metric: RegionMetric,
        name: Option<String>,
    },
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegionExtractParams {
    pub regions: Vec<RegionSpec>,
    pub input: Option<crate::DiffusionInput>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiffusionFitParams {
    pub response_column: plotium_data::ColumnId,
    pub options: Option<plotium_analysis::fit_model::FitOptions>,
}
fn invalid(plan: &ToolPlan, message: impl Into<String>) -> AutomationError {
    AutomationError::InvalidParameters {
        tool_id: plan.request.tool_id.clone(),
        message: message.into(),
    }
}
fn owner<'a>(app: &'a PlotiumApp, plan: &ToolPlan) -> Result<&'a Dataset, AutomationError> {
    let [target] = plan.frozen_targets.targets.as_slice() else {
        return Err(invalid(plan, "Select exactly one scientific source."));
    };
    if plan
        .targets
        .iter()
        .any(|t| t.status != TargetCompatibility::Compatible)
    {
        return Err(invalid(
            plan,
            "The source lacks the required scientific capability.",
        ));
    }
    let id = target.parent_id.as_deref().unwrap_or(&target.id);
    app.doc
        .datasets
        .iter()
        .find(|d| d.resource_id().to_string() == id)
        .ok_or_else(|| invalid(plan, "The source no longer exists."))
}
fn region_snapshot(app: &PlotiumApp, plan: &ToolPlan) -> Result<Dataset, AutomationError> {
    let source = owner(app, plan)?;
    let local = plan.frozen_targets.targets[0].local_id.as_deref();
    let field = source
        .field_descriptors()
        .into_iter()
        .find(|d| Some(d.local_id.as_str()) == local)
        .ok_or_else(|| invalid(plan, "Select an explicit region source field."))?;
    let mut snapshot = source
        .region_snapshot(field.id)
        .map_err(|e| invalid(plan, e))?;
    let params: RegionExtractParams =
        super::registry::parse(REGIONS_EXTRACT, plan.request.parameters.clone())?;
    if let Some(input) = params.input {
        let data = snapshot.as_nmr2d_mut().ok_or_else(|| {
            invalid(
                plan,
                "This region source does not consume diffusion inputs.",
            )
        })?;
        let input = super::science::resolve_input(data, Some(&input))?;
        // Explicit input takes priority over a selected result's provenance.
        data.dosy_provenance = None;
        data.ilt_provenance = None;
        data.diffusion_input = Some(input);
    }
    Ok(snapshot)
}
pub(super) fn prepare(app: &PlotiumApp, plan: &ToolPlan) -> Result<Value, AutomationError> {
    if plan.request.tool_id == REGIONS_EXTRACT {
        let mut snapshot = region_snapshot(app, plan)?;
        let params: RegionExtractParams =
            super::registry::parse(REGIONS_EXTRACT, plan.request.parameters.clone())?;
        if params.regions.is_empty() || params.regions.len() > 200 {
            return Err(invalid(plan, "Specify 1..=200 regions."));
        }
        let state = snapshot
            .region_analysis_mut()
            .ok_or_else(|| invalid(plan, "The source cannot extract regions."))?;
        let existing = state.regions.clone();
        let mut regions = Vec::new();
        for spec in params.regions {
            regions.push(match spec {
                RegionSpec::Existing { id } => existing
                    .iter()
                    .find(|r| r.id == id)
                    .cloned()
                    .ok_or_else(|| invalid(plan, format!("Unknown RegionId {id}")))?,
                RegionSpec::Explicit {
                    bounds,
                    metric,
                    name,
                } => Region {
                    id: state
                        .allocate_region_id()
                        .ok_or_else(|| invalid(plan, "Region identity space exhausted."))?,
                    lo: bounds[0],
                    hi: bounds[1],
                    name: name.unwrap_or_default(),
                    metric: Some(metric),
                    label_position: None,
                    color: [31, 111, 235],
                },
            });
        }
        state.regions = regions;
        state.validate().map_err(|e| invalid(plan, e))?;
        Ok(json!({"regions":state.regions,"output_id":DatasetId::new()}))
    } else {
        let source = owner(app, plan)?;
        let table = source
            .as_table()
            .ok_or_else(|| invalid(plan, "Select a native table."))?;
        let params: DiffusionFitParams =
            super::registry::parse(DIFFUSION_FIT, plan.request.parameters.clone())?;
        if !table
            .series_bindings
            .iter()
            .any(|s| s.value_column == params.response_column)
        {
            return Err(invalid(
                plan,
                "The response ColumnId is not a native table series.",
            ));
        }
        let x = table
            .x_binding
            .ok_or_else(|| invalid(plan, "Bind a b-value input column."))?;
        let unit = table
            .typed_state
            .envelope
            .revision
            .snapshot
            .schema
            .columns
            .iter()
            .find(|c| c.id == x)
            .and_then(|c| c.unit.as_ref());
        if !unit.is_some_and(|u| u.display_unit == "s/m²") {
            return Err(invalid(plan, "The b-value input column must use s/m²."));
        }
        let options = params.options.unwrap_or_default();
        if options.max_iterations == 0
            || options.max_iterations > 10000
            || options.multi_start == 0
            || options.multi_start > 100
        {
            return Err(invalid(
                plan,
                "Fit iterations must be 1..=10000 and multi_start 1..=100.",
            ));
        }
        for (name, value) in [
            ("tolerance", options.tolerance),
            ("relative_tolerance", options.relative_tolerance),
            ("absolute_tolerance", options.absolute_tolerance),
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(invalid(
                    plan,
                    format!("{name} must be finite and positive."),
                ));
            }
        }
        Ok(json!({"options":options}))
    }
}
pub(super) fn execute(
    app: &mut PlotiumApp,
    plan: &ToolPlan,
) -> Result<ToolResult, AutomationError> {
    let source = owner(app, plan)?;
    let dataset = source.resource_id();
    let settings = crate::settings::Settings::default();
    let prepared = plan
        .scientific_preparation
        .clone()
        .ok_or_else(|| invalid(plan, "Scientific preflight is missing."))?;
    let revision = plan.frozen_targets.revision;
    let desktop_region = plan.desktop_region;
    let operation = plan.request.tool_id.clone();
    let work: crate::state::ScientificWork = if operation == REGIONS_EXTRACT {
        let mut snapshot = region_snapshot(app, plan)?;
        let regions = serde_json::from_value(prepared["regions"].clone())
            .map_err(|e| invalid(plan, format!("Invalid frozen regions: {e}")))?;
        snapshot
            .region_analysis_mut()
            .ok_or_else(|| invalid(plan, "Source capability changed."))?
            .regions = regions;
        let output_id: DatasetId = serde_json::from_value(prepared["output_id"].clone())
            .map_err(|e| invalid(plan, e.to_string()))?;
        let local_id = plan.frozen_targets.targets[0].local_id.as_deref();
        let field = snapshot
            .field_descriptors()
            .into_iter()
            .find(|f| Some(f.local_id.as_str()) == local_id)
            .ok_or_else(|| invalid(plan, "Extraction source field no longer exists"))?
            .id;
        let version = app
            .session
            .compute
            .field_version_for(FieldRef {
                resource: dataset,
                field,
            })
            .map_err(|e| invalid(plan, format!("Cannot freeze source field version: {e:?}")))?;
        let source_version = TaskFieldVersion {
            dataset,
            field,
            version: version.0,
        };
        Box::new(move |token| {
            if token.is_cancelled() {
                return Err("Extraction cancelled".into());
            }
            let mut worker = PlotiumApp::with_compute(settings, ComputeService::inactive());
            worker.doc.datasets.push(snapshot);
            let mut table = worker.build_region_table(0, None)?;
            let evidence = table
                .provenance
                .take()
                .ok_or("Extraction evidence was not generated")?;
            let input_fingerprint = table.region_extraction_fingerprint(&evidence.regions)?;
            table.meta.region_extraction = Some(RegionExtractionProvenance {
                source_version: Some(source_version),
                input_fingerprint: Some(input_fingerprint),
                source: evidence.clone(),
                document_revision: revision.0,
                algorithm_version: 1,
            });
            table.resource_id = output_id;
            table.name = Some(format!(
                "{} — regions (frozen)",
                worker.doc.datasets[0].display_name()
            ));
            table.lineage = Some(DatasetLineage::new(
                DerivationKind::FrozenRegionTable,
                [dataset],
            ));
            if desktop_region == Some(true) {
                table.provenance = Some(evidence.clone());
                table.lineage = Some(DatasetLineage::new(
                    DerivationKind::LiveRegionTable,
                    [dataset],
                ));
                table.name = Some(format!(
                    "{} — regions",
                    worker.doc.datasets[0].display_name()
                ));
            }
            let resource = ResourceRef::from(output_id);
            let summary = json!({"status":"available","table":resource,"columns":evidence.regions,"source":table.meta.region_extraction,"diffusion_input":table.meta.diffusion_input,"increment_ids":table.meta.diffusion_input.as_ref().map(|i|i.increments.iter().map(|v|v.id).collect::<Vec<_>>())});
            Ok(crate::state::ScientificOutput {
                create_canvas: desktop_region.is_some(),
                action: Box::new(Action::InsertTableData {
                    dataset: Box::new(Dataset::Table(Box::new(table))),
                }),
                summary,
                produced: vec![resource],
            })
        })
    } else {
        let snapshot = source.clone();
        let params: DiffusionFitParams =
            super::registry::parse(DIFFUSION_FIT, plan.request.parameters.clone())?;
        let options = serde_json::from_value(prepared["options"].clone())
            .map_err(|e| invalid(plan, e.to_string()))?;
        Box::new(move |token| {
            if token.is_cancelled() {
                return Err("Fitting cancelled".into());
            }
            let mut worker = PlotiumApp::with_compute(settings, ComputeService::inactive());
            worker.doc.datasets.push(snapshot);
            let before = fit_state(worker.doc.datasets[0].as_table().ok_or("Source changed")?);
            worker.fit_table_columns(
                0,
                plotium_analysis::models::DIFFUSION_B_ID,
                false,
                params.response_column,
                false,
                options,
            );
            let table = worker.doc.datasets[0].as_table().ok_or("Source changed")?;
            let after = fit_state(table);
            if before == after {
                return Err(worker.session.status.clone());
            }
            let analysis = table
                .curve_fit_analyses
                .last()
                .ok_or("No fit result was generated")?;
            let analysis_resource = diffusion_analysis_ref(dataset, analysis.id);
            let summary = json!({"status":"available","table":ResourceRef::from(dataset),"analysis":analysis_resource,"analysis_id":analysis.id,"model":analysis.result.model,"parameters":analysis.result.parameters,"quality":analysis.result.statistics,"converged":analysis.result.converged,"notices":analysis.result.notices,"selection":analysis.selection,"diffusion_unit":"m²/s","b_unit":"s/m²"});
            Ok(crate::state::ScientificOutput {
                create_canvas: false,
                action: Box::new(Action::set_curve_fit_analyses(dataset, before, after)),
                summary,
                produced: vec![analysis_resource],
            })
        })
    };
    let task_id = plan
        .task_id
        .clone()
        .ok_or_else(|| invalid(plan, "TaskId is missing."))?;
    let receipt = TaskReceipt {
        sources: Vec::new(),
        task_id,
        plan_id: plan.submitted_plan_id.clone(),
        operation,
        project_context: app.session.project_context,
        targets: plan.frozen_targets.targets.clone(),
        start_revision: revision,
        end_revision: None,
        status: TaskStatus::Queued,
        phase: "queued".into(),
        completed: None,
        total: None,
        failure_code: None,
        message: None,
        produced: vec![],
        modified: if plan.request.tool_id == DIFFUSION_FIT {
            vec![ResourceRef::from(dataset)]
        } else {
            vec![]
        },
        summary: Value::Null,
    };
    let receipt = app
        .start_prepared_scientific(dataset, receipt, work)
        .map_err(|e| invalid(plan, e))?;
    Ok(super::tool_executors::readonly_value(
        &plan.request.tool_id,
        revision,
        json!({"task":receipt}),
    ))
}
type FitState = (Vec<Option<CurveFitReference>>, Vec<StoredCurveFitAnalysis>);
fn fit_state(table: &TableDataset) -> FitState {
    (
        table
            .series_bindings
            .iter()
            .map(|s| s.fit.clone())
            .collect(),
        table.curve_fit_analyses.clone(),
    )
}

pub(super) fn inspect_table(
    app: &PlotiumApp,
    plan: &ToolPlan,
    params: &AnalysisResultsParams,
) -> Result<Option<Value>, AutomationError> {
    let source = owner(app, plan)?;
    let Some(table) = source.as_table() else {
        return Ok(None);
    };
    if params.method.as_deref().is_some_and(|m| m != "mono_exp") {
        return Err(invalid(
            plan,
            "Native diffusion decay results use mono_exp.",
        ));
    }
    let target = &plan.frozen_targets.targets[0];
    let selected = if target.kind.0 == KIND_ANALYSIS {
        Some(
            target
                .local_id
                .as_deref()
                .and_then(|id| id.strip_prefix("diffusion_fit:"))
                .and_then(|id| id.parse::<u64>().ok())
                .ok_or_else(|| invalid(plan, "Invalid diffusion analysis identity"))?,
        )
    } else {
        None
    };
    let analyses=table.curve_fit_analyses.iter().filter(|a|a.result.model.id==plotium_analysis::models::DIFFUSION_B_ID && selected.is_none_or(|id|id==a.id)).map(|analysis|{
        let result=&analysis.result;
        json!({"analysis":diffusion_analysis_ref(table.resource_id,analysis.id),"analysis_id":analysis.id,"model":result.model,"parameters":result.parameters,"quality":result.statistics,"converged":result.converged,"termination":result.attempts,"notices":result.notices,"selection":analysis.selection,"valid":analysis.selection.as_ref().is_some_and(|s|s.source_revision==table.typed_state.envelope.revision.id),"diagnostics":result.points.iter().skip(params.offset).take(params.limit).collect::<Vec<_>>(),"total":result.points.len(),"unavailable_number_reason":"Non-finite numerical estimates are serialized as null."})
    }).collect::<Vec<_>>();
    let valid = !analyses.is_empty() && analyses.iter().all(|a| a["valid"] == true);
    Ok(Some(
        json!({"method":"mono_exp","status":if analyses.is_empty(){"empty"}else{"available"},"valid":valid,"analyses":analyses,"source":table.meta.region_extraction,"diffusion_input":table.meta.diffusion_input,"b_unit":"s/m²","diffusion_unit":"m²/s","offset":params.offset,"limit":params.limit}),
    ))
}

pub(super) fn descriptors() -> Vec<ToolDescriptor> {
    [REGIONS_EXTRACT,DIFFUSION_FIT].into_iter().map(|id|ToolDescriptor {
        id:id.into(),version:1,title:id.into(),description:if id==REGIONS_EXTRACT{"Extract explicit regions into a native frozen table, without creating a plot."}else{"Fit one native table response to the existing b-value mono-exponential model."}.into(),
        parameter_schema:if id==REGIONS_EXTRACT{json!({"type":"object","additionalProperties":false,"required":["regions"],"properties":{"regions":{"type":"array","minItems":1,"maxItems":200,"items":{"oneOf":[{"type":"object","required":["source","id"],"additionalProperties":false,"properties":{"source":{"const":"existing"},"id":{"type":"integer","minimum":0}}},{"type":"object","required":["source","bounds","metric"],"additionalProperties":false,"properties":{"source":{"const":"explicit"},"bounds":{"type":"array","items":{"type":"number"},"minItems":2,"maxItems":2},"metric":{"enum":["Height","Area","Mean","Max","Min"]},"name":{"type":"string"}}}]}},"input":diffusion_input_schema()}})}else{json!({"type":"object","additionalProperties":false,"required":["response_column"],"properties":{"response_column":{"type":"string"},"options":super::scientific_schema::fit_options()}})},
        result_schema:super::scientific_schema::task_result(),
        target_kinds:vec![ResourceKindId::new(if id==REGIONS_EXTRACT{KIND_FIELD}else{KIND_DATASET})],required_capabilities:vec![CapabilityId::new(if id==REGIONS_EXTRACT{CAP_FIELD_REGION_SERIES}else{CAP_DIFFUSION_FIT})],effect:EffectLevel::Reversible,undoable:true,deterministic:true,task_kind:"background".into(),
    }).collect()
}
