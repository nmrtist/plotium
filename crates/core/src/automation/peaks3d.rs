//! Shared CLI/MCP operations on complete three-axis peak observations.
use super::*;
use crate::state::Peak3DId;
use serde::Deserialize;
use serde_json::json;
pub const PEAKS_3D_LIST: &str = "nmr.peaks3d.list";
pub const PEAKS_3D: &str = "nmr.peaks3d";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    #[serde(default)]
    pub operation: String,
    pub id: Option<Peak3DId>,
    pub ppm: Option<[f64; 3]>,
    pub note: Option<String>,
}
pub(super) fn execute(
    app: &mut crate::state::PlotiumApp,
    plan: &ToolPlan,
) -> Result<ToolResult, AutomationError> {
    let [target] = plan.frozen_targets.targets.as_slice() else {
        return Err(AutomationError::InvalidSelector(
            "Select one 3D spectrum".into(),
        ));
    };
    let di = app
        .doc
        .datasets
        .iter()
        .position(|d| d.resource_id().to_string() == target.id)
        .ok_or_else(|| AutomationError::InvalidSelector("Select an existing spectrum".into()))?;
    let dataset = app.doc.datasets[di].resource_id();
    let mut p: Params = super::registry::parse(PEAKS_3D, plan.request.parameters.clone())?;
    if plan.request.tool_id == PEAKS_3D_LIST {
        p.operation = "list".into();
    }
    let fail = |s: &str| AutomationError::Execution(s.into());
    let mut modified = false;
    match p.operation.as_str() {
        "list" => {}
        "add" => {
            app.add_peak_3d(
                dataset,
                p.ppm.ok_or_else(|| fail("Supply three ppm coordinates"))?,
            )
            .map_err(AutomationError::Execution)?;
            modified = true;
        }
        "edit" => {
            let id = p.id.ok_or_else(|| fail("Supply a peak ID"))?;
            let old = app.doc.datasets[di]
                .as_nmr2d()
                .and_then(|n| n.volume.as_ref())
                .and_then(|v| v.peaks.marks.iter().find(|m| m.id == id))
                .cloned()
                .ok_or_else(|| fail("Select an existing 3D peak"))?;
            app.edit_peak_3d(
                dataset,
                id,
                p.ppm.unwrap_or(old.ppm),
                p.note.unwrap_or(old.note),
            )
            .map_err(AutomationError::Execution)?;
            modified = true;
        }
        "delete" => {
            app.delete_peak_3d(dataset, p.id.ok_or_else(|| fail("Supply a peak ID"))?)
                .map_err(AutomationError::Execution)?;
            modified = true;
        }
        "locate" => {
            app.locate_peak_3d(dataset, p.id.ok_or_else(|| fail("Supply a peak ID"))?)
                .map_err(AutomationError::Execution)?;
            modified = true;
        }
        _ => {
            return Err(fail("Choose list, add, edit, delete or locate"));
        }
    }
    let mut result = super::tool_executors::readonly_value(
        &plan.request.tool_id,
        plan.frozen_targets.revision,
        app.peak_table_3d(dataset)
            .map_err(AutomationError::Execution)?,
    );
    if modified {
        result.modified.push(target.clone());
    }
    result.after_revision = DocumentRevision(app.doc.automation_revision);
    Ok(result)
}
pub(super) fn descriptors() -> Vec<ToolDescriptor> {
    let mut edit = ToolDescriptor{id:PEAKS_3D.into(),version:1,title:"Edit or inspect 3D peaks".into(),description:"Persistent peak coordinates in acquisition-axis order; list returns an exportable peak table with axis identities.".into(),parameter_schema:json!({"type":"object","additionalProperties":false,"required":["operation"],"properties":{"operation":{"enum":["list","add","edit","delete","locate"]},"id":{"type":"integer","minimum":0},"ppm":{"type":"array","minItems":3,"maxItems":3,"items":{"type":"number"}},"note":{"type":"string"}}}),result_schema:json!({"type":"object"}),target_kinds:vec![ResourceKindId::new(KIND_DATASET)],required_capabilities:vec![CapabilityId::new(CAP_PROCESSING_SCHEME)],effect:EffectLevel::Reversible,undoable:true,deterministic:true,task_kind:"synchronous".into()};
    let mut list = edit.clone();
    list.id = PEAKS_3D_LIST.into();
    list.title = "List and export 3D peaks".into();
    list.effect = EffectLevel::ReadOnly;
    list.undoable = false;
    list.parameter_schema = json!({"type":"object","additionalProperties":false});
    edit.description
        .push_str(" Use nmr.peaks3d.list for read-only export.");
    vec![list, edit]
}

#[cfg(test)]
#[path = "peaks3d_tests.rs"]
mod tests;
