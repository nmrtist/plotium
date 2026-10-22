//! Transport adaptation only; all binding semantics belong to the native service.
use super::*;
use crate::state::{CurveBindingEdit, CurveEditPlan, MaterializeCurveEdit, PlotiumApp};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const CURVE_APPEND: &str = "plot.series.append";
pub const CURVE_BIND: &str = "plot.series.bind";
pub const CURVE_MATERIALIZE: &str = "plot.series.materialize_default";
pub fn is_curve_binding_tool(id: &str) -> bool {
    matches!(id, CURVE_BIND | CURVE_MATERIALIZE | CURVE_APPEND)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurveBindParams {
    pub edits: Vec<CurveBindingEdit>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurveMaterializeParams {
    pub edits: Vec<MaterializeCurveEdit>,
}
fn invalid(tool: &str, e: impl Into<String>) -> AutomationError {
    AutomationError::InvalidParameters {
        tool_id: tool.into(),
        message: e.into(),
    }
}
pub fn curve_append_schema() -> Value {
    let mut plot = document_inspect_schema()["properties"]["target"].clone();
    plot["properties"]
        .as_object_mut()
        .expect("local target schema")
        .remove("component");
    plot["properties"]["resource"]["properties"]["kind"] = json!({"const":KIND_CANVAS_OBJECT});
    let series = plot_create_schema()["properties"]["series"].clone();
    json!({"type":"object","additionalProperties":false,"required":["plot","series"],"properties":{"plot":plot,"series":series},"description":"Append selected explicit curves to one existing explicit-coordinate plot (200 total max). New Series IDs are frozen in binding_plan.appended. Existing curves/styles/coordinates/ranges/frames remain unchanged, except that existing bars regroup to share their slots with appended bars; one Undo removes only the appended curves. Omit top-level ids/targets."})
}
pub fn curve_binding_schema(materialize: bool) -> Value {
    let uuid = json!({"type":"string","format":"uuid"});
    let target = document_inspect_schema()["properties"]["target"].clone();
    let edit = if materialize {
        json!({"type":"object","additionalProperties":false,"required":["target","retain_y_column_id","table_revision"],"properties":{"target":target,"retain_y_column_id":uuid,"table_revision":uuid}})
    } else {
        json!({"type":"object","additionalProperties":false,"required":["target","source"],"properties":{"target":target,"source":{"type":"object","additionalProperties":false,"required":["dataset_id","field_id","table_id","table_revision","x_column_id","y_column_id","y_error_column_id"],"properties":{"dataset_id":uuid,"field_id":{"type":"integer","minimum":0},"table_id":uuid,"table_revision":uuid,"x_column_id":uuid,"y_column_id":uuid,"y_error_column_id":{"type":["string","null"],"format":"uuid"}}}}})
    };
    json!({"type":"object","additionalProperties":false,"required":["edits"],"properties":{"edits":{"type":"array","minItems":1,"maxItems":200,"items":edit}}})
}
pub(super) fn check_source_revision(
    app: &PlotiumApp,
    tool: &str,
    source: &crate::state::TableCurveSource,
) -> Result<(), AutomationError> {
    let actual = app
        .doc
        .dataset_by_id(source.dataset_id)
        .and_then(|d| d.as_table())
        .ok_or_else(|| invalid(tool, "Table dataset is missing."))?
        .typed_state
        .envelope
        .revision
        .id;
    if actual != source.table_revision {
        return Err(AutomationError::StaleTableRevision {
            expected: source.table_revision.to_string(),
            actual: actual.to_string(),
        });
    }
    Ok(())
}
pub(super) fn prepare(
    app: &PlotiumApp,
    request: &ToolRequest,
) -> Result<CurveEditPlan, AutomationError> {
    if request.targets != (TargetSelector::Explicit { ids: Vec::new() }) {
        return Err(invalid(
            &request.tool_id,
            "Use exact targets in edits, without a second ids/targets selector.",
        ));
    }
    let tool = request.tool_id.as_str();
    let check = |dataset: crate::state::DatasetId,
                 expected: plotium_data::RevisionId|
     -> Result<(), AutomationError> {
        let actual = app
            .doc
            .dataset_by_id(dataset)
            .and_then(|d| d.as_table())
            .ok_or_else(|| invalid(tool, "Table dataset is missing."))?
            .typed_state
            .envelope
            .revision
            .id;
        if expected != actual {
            return Err(AutomationError::StaleTableRevision {
                expected: expected.to_string(),
                actual: actual.to_string(),
            });
        }
        Ok(())
    };
    if tool == CURVE_APPEND {
        let p: crate::state::CurveAppendParams = registry::parse(tool, request.parameters.clone())?;
        for curve in &p.series {
            check_source_revision(app, tool, &curve.source)?;
        }
        app.plan_append_curves(request.expected_revision, &p)
            .map_err(|e| invalid(tool, e))
    } else if tool == CURVE_BIND {
        let p: CurveBindParams = registry::parse(tool, request.parameters.clone())?;
        for e in &p.edits {
            check_source_revision(app, tool, &e.source)?;
        }
        app.plan_curve_bindings(request.expected_revision, &p.edits)
            .map_err(|e| invalid(tool, e))
    } else {
        let p: CurveMaterializeParams = registry::parse(tool, request.parameters.clone())?;
        for e in &p.edits {
            let (c, o, s) = app.curve_target(&e.target).map_err(|e| invalid(tool, e))?;
            let plot = app.doc.canvases[app.doc.canvas_index(c).expect("validated canvas")]
                .object(o)
                .and_then(|o| o.plot())
                .expect("validated plot");
            let dataset = plot
                .binding
                .series
                .iter()
                .find(|v| v.id == s)
                .expect("validated Series")
                .source
                .resource;
            check(dataset, e.table_revision)?;
        }
        app.plan_materialize_curves(request.expected_revision, &p.edits)
            .map_err(|e| invalid(tool, e))
    }
}
pub(super) fn freeze(request: &ToolRequest) -> Result<FrozenTargetSet, AutomationError> {
    let targets = if request.tool_id == CURVE_APPEND {
        vec![
            registry::parse::<crate::state::CurveAppendParams>(
                &request.tool_id,
                request.parameters.clone(),
            )?
            .plot,
        ]
    } else if request.tool_id == CURVE_BIND {
        registry::parse::<CurveBindParams>(&request.tool_id, request.parameters.clone())?
            .edits
            .into_iter()
            .map(|e| e.target)
            .collect::<Vec<_>>()
    } else {
        registry::parse::<CurveMaterializeParams>(&request.tool_id, request.parameters.clone())?
            .edits
            .into_iter()
            .map(|e| e.target)
            .collect()
    };
    Ok(FrozenTargetSet {
        revision: request.expected_revision,
        targets: targets.iter().map(|t| t.resource.clone()).collect(),
        total_matches: targets.len(),
        component_targets: targets,
        reasons: vec![],
        truncated: false,
    })
}
pub(super) fn execute(
    app: &mut PlotiumApp,
    plan: &ToolPlan,
) -> Result<ToolResult, AutomationError> {
    let prepared = plan.binding_plan.clone().ok_or_else(|| {
        invalid(
            &plan.request.tool_id,
            "Prepare a new native binding plan before execution.",
        )
    })?;
    // Revalidate all source revisions and identities before consuming native state.
    prepare(app, &plan.request)?;
    let value =
        serde_json::to_value(&prepared).map_err(|e| AutomationError::Execution(e.to_string()))?;
    app.commit_curve_bindings(prepared)
        .map_err(|e| invalid(&plan.request.tool_id, e))?;
    Ok(ToolResult {
        tool_id: plan.request.tool_id.clone(),
        before_revision: plan.frozen_targets.revision,
        after_revision: DocumentRevision(app.doc.automation_revision),
        targets: plan
            .targets
            .iter()
            .map(|t| TargetResult {
                target: t.target.clone(),
                outcome: TargetOutcome::Succeeded,
                message: "Updated native curve binding.".into(),
                skip_reason: None,
                fingerprints: vec![],
            })
            .collect(),
        produced: vec![],
        modified: plan.frozen_targets.targets.clone(),
        diagnostics: vec![],
        verification: vec![],
        value,
    })
}
