//! Thin protocol adapter over native page-authoring transactions.
use super::*;
use crate::state::{FigureEditParams, FigureEditPlan, PlotiumApp};
use serde_json::{Value, json};
pub const FIGURE_EDIT: &str = "figure.edit";
pub fn figure_edit_schema() -> Value {
    let mut series_target = document_inspect_schema()["properties"]["target"].clone();
    series_target["required"] = json!(["resource", "component"]);
    series_target["properties"]["component"] = json!({"type":"object","additionalProperties":false,"required":["kind","id"],"properties":{"kind":{"const":"series"},"id":{"type":"integer","minimum":0}}});
    let mut target = document_inspect_schema()["properties"]["target"].clone();
    target["properties"]
        .as_object_mut()
        .expect("schema object")
        .remove("component");
    let number = json!({"type":"number"});
    let arr = |n| json!({"type":"array","minItems":n,"maxItems":n,"items":number});
    let panels = json!({"type":"array","minItems":1,"maxItems":200,"items":target});
    let name = json!({"type":"string","minLength":1,"maxLength":1024});
    let text = json!({"type":"string","maxLength":16384});
    let font = json!({"type":"number","minimum":1,"maximum":72});
    let annotation_text =
        json!({"type":"string","minLength":1,"maxLength":crate::state::MAX_ANNOTATION_TEXT_BYTES});
    let annotation_id = json!({"type":"integer","minimum":0,"description":"Plot-local annotation id from document inspection or an earlier annotation_edits result."});
    let data_point = json!({"type":"array","minItems":2,"maxItems":2,"items":number,"description":"Finite [x,y] anchor in the plot's source data coordinates (the units in its coordinate_units, before any display-unit conversion). The label is centred above the anchor and follows axis range, zoom and pan; anchors outside the visible range are clipped."});
    let variant = |op: &str, required: Vec<&str>, mut properties: Value| {
        properties["op"] = json!({"const":op});
        let required = std::iter::once("op").chain(required).collect::<Vec<_>>();
        json!({"type":"object","additionalProperties":false,"required":required,"properties":properties})
    };
    let mut variants = vec![
        variant(
            "page",
            vec![],
            json!({"size_mm":arr(2),"size_preset":size_preset_schema(),"margin_mm":arr(4),"gutter_mm":{"type":"number","minimum":0}}),
        ),
        variant(
            "panel_frame",
            vec!["target", "frame_pt"],
            json!({"target":target,"frame_pt":arr(4)}),
        ),
        variant(
            "content_frame",
            vec!["target", "frame_pt"],
            json!({"target":target,"frame_pt":{"type":"array","minItems":4,"maxItems":4,"items":number,"description":"Complete finite [x,y,width,height] parent-local pt frame (page pt for loose content), width/height at least 1 pt; edits this item only. Raster preserve_aspect constrains dimensions; read the effective frame in preflight."}}),
        ),
        variant(
            "image_replace",
            vec!["target", "path"],
            json!({"target":target,"path":{"type":"string","description":"Absolute explicitly named PNG/JPEG regular file; frozen at preflight. Replace only the asset reference of a raster at page_index 0; retain all other fields."}}),
        ),
        variant(
            "panel_label",
            vec!["target", "text", "visible"],
            json!({"target":target,"text":name,"visible":{"type":"boolean"}}),
        ),
        variant(
            "align",
            vec!["panels", "edge"],
            json!({"panels":panels,"edge":{"enum":["left","center","right","top","middle","bottom"]}}),
        ),
        variant(
            "distribute",
            vec!["panels", "axis"],
            json!({"panels":panels,"axis":{"enum":["horizontal","vertical"]}}),
        ),
        variant(
            "grid",
            vec!["panels", "rows", "cols"],
            json!({"panels":panels,"rows":{"type":"integer","minimum":1,"maximum":16},"cols":{"type":"integer","minimum":1,"maximum":16}}),
        ),
        variant(
            "text_create",
            vec!["parent", "name", "frame_pt", "text", "font_pt"],
            json!({"parent":target,"name":name,"frame_pt":arr(4),"text":text,"font_pt":font}),
        ),
        variant(
            "text_set",
            vec!["target", "text"],
            json!({"target":target,"text":text,"font_pt":font}),
        ),
        variant(
            "image_create",
            vec!["parent", "name", "frame_pt", "path"],
            json!({"parent":target,"name":name,"frame_pt":arr(4),"path":{"type":"string","description":"Absolute explicitly named local PNG/JPEG regular file; frozen at preflight, no adjacent file lookup."}}),
        ),
        variant(
            "annotation_create",
            vec!["target", "text", "at", "font_pt"],
            json!({"target":target,"text":annotation_text,"at":data_point,"font_pt":font}),
        ),
        {
            let mut set = variant(
                "annotation_set",
                vec!["target", "annotation_id"],
                json!({"target":target,"annotation_id":annotation_id,"text":annotation_text,"at":data_point,"font_pt":font}),
            );
            set["anyOf"] =
                json!([{"required":["text"]},{"required":["at"]},{"required":["font_pt"]}]);
            set
        },
        variant(
            "annotation_delete",
            vec!["target", "annotation_id"],
            json!({"target":target,"annotation_id":annotation_id}),
        ),
        variant("content_delete", vec!["target"], json!({"target":target})),
        variant("panel_delete", vec!["target"], json!({"target":target})),
        variant(
            "series_delete",
            vec!["target"],
            json!({"target":series_target}),
        ),
    ];
    variants[0]["not"] = json!({"required":["size_mm","size_preset"]});
    json!({"type":"object","additionalProperties":false,"required":["canvas","edits"],"properties":{"canvas":target,"edits":{"type":"array","minItems":1,"maxItems":200,"items":{"oneOf":variants}}},"description":"Atomic native authoring on one exact canvas. Panel frames [x,y,w,h] are page pt, resize scales child-local frames. New content frames are parent-local pt. Page size/margins/gutter use mm; margins [top,right,bottom,left]. Grid uses native Frame-basis cells in supplied Panel order, existing page margins/gutter; no page-layout preference change or non-target reflow. Align uses bounding edges/centres, distribution equal centre spacing with endpoints fixed. Panel layout ops require unlocked free-layout Panels. content_frame requires the item and parent unlocked, a free-layout parent and Auto height off; image_replace requires the item and parent unlocked, not a layout mode change. Deletion-only batches accept distinct content_delete, originally empty panel_delete and explicit-column series_delete targets on this canvas. Reject overlaps, last-Series removal, defaults/dynamic displays and mixing deletion with other ops. Preserve empty parents, datasets and assets; preflight lists exact deletions and relationship changes. Hidden exact targets remain addressable. Annotation ops target an exact unlocked plot in an unlocked parent, anchor text in source data coordinates, allow at most 256 per plot and are allowed with Auto height. No top-level ids/targets."})
}
/// Built-in page size presets by id, shared by page edits and new canvases.
pub(super) fn size_preset_schema() -> Value {
    let ids: Vec<_> = crate::state::size_presets().iter().map(|p| p.id).collect();
    json!({"enum":ids,"description":"Built-in page size instead of size_mm. Journal column presets (e.g. nature-1col 89 mm, nature-1p5col 136 mm, nature-2col 183 mm) set the width and keep the height of a page with content; paper and slide presets set both dimensions. Also sets the preset's page margins unless margin_mm is given."})
}
fn invalid(message: impl Into<String>) -> AutomationError {
    AutomationError::InvalidParameters {
        tool_id: FIGURE_EDIT.into(),
        message: message.into(),
    }
}
pub(super) fn prepare(
    app: &PlotiumApp,
    request: &ToolRequest,
) -> Result<FigureEditPlan, AutomationError> {
    if request.targets != (TargetSelector::Explicit { ids: vec![] }) {
        return Err(invalid(
            "Figure edits carry exact targets inside parameters; omit ids/targets.",
        ));
    }
    let p: FigureEditParams = registry::parse(FIGURE_EDIT, request.parameters.clone())?;
    app.plan_figure_edit(request.expected_revision, &p)
        .map_err(invalid)
}
pub(super) fn freeze(request: &ToolRequest) -> Result<FrozenTargetSet, AutomationError> {
    let p: FigureEditParams = registry::parse(FIGURE_EDIT, request.parameters.clone())?;
    Ok(FrozenTargetSet {
        revision: request.expected_revision,
        targets: vec![p.canvas.resource.clone()],
        component_targets: vec![p.canvas],
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
        .figure_plan
        .clone()
        .ok_or_else(|| invalid("Prepare a native figure plan before execution."))?;
    let mut targets = prepared.modified.clone();
    targets.extend(prepared.created.clone());
    targets.extend(prepared.deleted.clone());
    let produced = prepared
        .created
        .iter()
        .map(|t| t.resource.clone())
        .collect();
    let modified = prepared
        .modified
        .iter()
        .map(|t| t.resource.clone())
        .collect();
    let value = prepared.result.clone();
    app.commit_figure_edit(prepared).map_err(invalid)?;
    Ok(ToolResult {
        tool_id: FIGURE_EDIT.into(),
        before_revision: plan.frozen_targets.revision,
        after_revision: DocumentRevision(app.doc.automation_revision),
        targets: targets
            .into_iter()
            .map(|target| TargetResult {
                target,
                outcome: TargetOutcome::Succeeded,
                message: "Applied native figure authoring transaction.".into(),
                skip_reason: None,
                fingerprints: vec![],
            })
            .collect(),
        produced,
        modified,
        diagnostics: vec![],
        verification: vec![],
        value,
    })
}
