//! MCP/core adapter for the shared native curve planner and commit service.
use super::*;
use crate::state::{
    CreatedCurveDefaults, CurveCoordinates, CurveEditPlan, PlotCreateParams, PlotCreationSummary,
    PlotiumApp,
};
use serde::Serialize;
use serde_json::{Value, json};

pub const PLOT_CREATE: &str = "plot.create";
pub fn is_native_curve_tool(id: &str) -> bool {
    is_curve_binding_tool(id) || id == PLOT_CREATE
}
pub fn plot_create_schema() -> Value {
    let target = document_inspect_schema()["properties"]["target"].clone();
    let source =
        curve_binding_schema(false)["properties"]["edits"]["items"]["properties"]["source"].clone();
    let name = json!({"type":"string","minLength":1,"maxLength":1024,"description":"Nonblank name, at most 1024 UTF-8 bytes; names are labels, not identity."});
    let mut parent = target;
    parent["description"] = json!(
        "Exact whole document, empty canvas without panels, or its sole empty visible unlocked free-layout panel. Creation never uses selection. An explicit panel_frame_pt on an existing canvas creates an additional Panel without moving existing content."
    );
    parent["properties"]["resource"]["properties"]["kind"] =
        json!({"enum":[KIND_DOCUMENT,KIND_CANVAS,KIND_PANEL]});
    parent["properties"]
        .as_object_mut()
        .expect("local schema")
        .remove("component");
    json!({"type":"object","additionalProperties":false,"required":["parent","name","series"],"properties":{
        "parent":parent,"name":name,
        "canvas":{"type":"object","additionalProperties":false,"required":["name"],"oneOf":[{"required":["size_mm"]},{"required":["size_preset"]}],"description":"Only for document parent. Omit to use the plot name and native default size, frozen in preflight.","properties":{"name":name,"size_mm":{"type":"array","minItems":2,"maxItems":2,"items":{"type":"number","minimum":10,"maximum":1000},"description":"[width,height] in mm."},"size_preset":super::figure_edit::size_preset_schema()}},
        "panel_frame_pt":{"type":"array","minItems":4,"maxItems":4,"items":{"type":"number"},"description":"New Panel [x,y,width,height] in page-relative pt, finite positive width/height. Required to add a second Panel to an existing canvas. Omit for existing empty-Panel parent and native empty-page placement."},
        "panel_name":{"type":"string","minLength":1,"maxLength":1024,"description":"Only for a newly created panel; defaults to plot name."},
        "series":{"type":"array","minItems":1,"maxItems":200,"items":{"type":"object","additionalProperties":false,"required":["kind","source"],"properties":{"kind":{"enum":["line","scatter"]},"source":source}}}
    }})
}
pub(super) fn prepare(
    app: &PlotiumApp,
    request: &ToolRequest,
) -> Result<CurveEditPlan, AutomationError> {
    if request.targets != (TargetSelector::Explicit { ids: vec![] }) {
        return Err(invalid(
            "Creation carries its own exact parent; omit top-level ids/targets.",
        ));
    }
    let p: PlotCreateParams = registry::parse(PLOT_CREATE, request.parameters.clone())?;
    for curve in &p.series {
        super::curve_bindings::check_source_revision(app, PLOT_CREATE, &curve.source)?;
    }
    app.plan_plot_creation(request.expected_revision, &p)
        .map_err(invalid)
}
fn invalid(message: impl Into<String>) -> AutomationError {
    AutomationError::InvalidParameters {
        tool_id: PLOT_CREATE.into(),
        message: message.into(),
    }
}
pub(super) fn freeze(request: &ToolRequest) -> Result<FrozenTargetSet, AutomationError> {
    let p: PlotCreateParams = registry::parse(PLOT_CREATE, request.parameters.clone())?;
    Ok(FrozenTargetSet {
        revision: request.expected_revision,
        targets: vec![p.parent.resource.clone()],
        component_targets: vec![p.parent],
        total_matches: 1,
        reasons: vec![],
        truncated: false,
    })
}
pub(super) fn execute(
    app: &mut PlotiumApp,
    plan: &ToolPlan,
) -> Result<ToolResult, AutomationError> {
    let prepared = plan
        .binding_plan
        .clone()
        .ok_or_else(|| invalid("Prepare a new native creation plan before execution."))?;
    let summary = prepared
        .creation
        .as_ref()
        .ok_or_else(|| invalid("Plan is not a native creation."))?;
    let value = serde_json::to_value(CreatedPlot::new(&prepared, summary))
        .map_err(|e| AutomationError::Execution(e.to_string()))?;
    let produced = summary
        .created
        .iter()
        .filter(|t| t.component.is_none())
        .map(|t| t.resource.clone())
        .collect();
    let targets = summary
        .created
        .iter()
        .map(|t| TargetResult {
            target: t.clone(),
            outcome: TargetOutcome::Succeeded,
            message: "Created native plot content.".into(),
            skip_reason: None,
            fingerprints: vec![],
        })
        .collect();
    for binding in &prepared.bindings {
        super::curve_bindings::check_source_revision(app, PLOT_CREATE, &binding.source)?;
    }
    // Do not replan: the prepared candidate owns the identities returned by preflight.
    app.commit_curve_bindings(prepared).map_err(invalid)?;
    Ok(ToolResult {
        tool_id: PLOT_CREATE.into(),
        before_revision: plan.frozen_targets.revision,
        after_revision: DocumentRevision(app.doc.automation_revision),
        targets,
        produced,
        modified: plan.frozen_targets.targets.clone(),
        diagnostics: vec![],
        verification: vec![],
        value,
    })
}

/// The committed view of a creation plan. Preflight already returned the
/// parent, the source mappings and sample affected rows; repeating them here
/// only grows every creation response.
#[derive(Serialize)]
struct CreatedPlot<'a> {
    creation: CreatedLayout<'a>,
    bindings: Vec<CreatedBinding<'a>>,
}
#[derive(Serialize)]
struct CreatedLayout<'a> {
    canvas: &'a TargetRef,
    panel: &'a TargetRef,
    plot: &'a TargetRef,
    size_mm: [f32; 2],
    #[serde(skip_serializing_if = "Option::is_none")]
    size_preset: Option<&'a str>,
    frame_pt: [f32; 4],
    panel_frame_pt: [f32; 4],
    coordinate_units: &'a CurveCoordinates,
    series_defaults: &'a [CreatedCurveDefaults],
}
#[derive(Serialize)]
struct CreatedBinding<'a> {
    target: &'a TargetRef,
    diagnostics: RowCounts,
}
#[derive(Serialize)]
struct RowCounts {
    total_rows: usize,
    plotted_rows: usize,
    missing_xy: usize,
    non_finite_xy: usize,
    missing_error: usize,
}
impl<'a> CreatedPlot<'a> {
    fn new(plan: &'a CurveEditPlan, summary: &'a PlotCreationSummary) -> Self {
        Self {
            creation: CreatedLayout {
                canvas: &summary.canvas,
                panel: &summary.panel,
                plot: &summary.plot,
                size_mm: summary.size_mm,
                size_preset: summary.size_preset.as_deref(),
                frame_pt: summary.frame_pt,
                panel_frame_pt: summary.panel_frame_pt,
                coordinate_units: &summary.coordinate_units,
                series_defaults: &summary.series_defaults,
            },
            bindings: plan
                .bindings
                .iter()
                .map(|binding| {
                    let d = &binding.diagnostics;
                    CreatedBinding {
                        target: &binding.target,
                        diagnostics: RowCounts {
                            total_rows: d.total_rows,
                            plotted_rows: d.plotted_rows,
                            missing_xy: d.missing_xy,
                            non_finite_xy: d.non_finite_xy,
                            missing_error: d.missing_error,
                        },
                    }
                })
                .collect(),
        }
    }
}
