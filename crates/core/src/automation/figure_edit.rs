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
    let data_point = json!({"type":"array","minItems":2,"maxItems":2,"items":number,"description":"Finite [x,y] anchor in the plot's source data coordinates (the units in its coordinate_units, before any display-unit conversion). The label is centred above the anchor, or moved up to three text heights to the nearest spot clear of data, and follows axis range, zoom and pan; anchors outside the visible range are clipped."});
    let orientation = json!({"enum":["vertical","horizontal"],"description":"vertical: constant x, spans the plot's height; horizontal: constant y, spans its width."});
    let line_value = json!({"type":"number","description":"Finite x (vertical) or y (horizontal) in the plot's source data coordinates (coordinate_units). The line spans the plot area, follows axis range, zoom and pan, and is clipped when off-axis."});
    let line_label = json!({"type":"string","maxLength":crate::state::MAX_ANNOTATION_TEXT_BYTES,"description":"Optional label drawn in the line colour at the line's top (vertical) or right end (horizontal), or further along the line when that spot covers data; empty removes it. It uses the tick label size until font_pt sets an explicit size, which then stays until changed."});
    let line_style = json!({"type":"object","additionalProperties":false,"minProperties":1,"properties":{
        "color":{"type":"string","pattern":"^#[0-9a-fA-F]{6}$"},
        "width_pt":{"type":"number","minimum":crate::state::reference_line_width_bound(crate::state::MIN_REFERENCE_LINE_WIDTH_PT),"maximum":crate::state::reference_line_width_bound(crate::state::MAX_REFERENCE_LINE_WIDTH_PT)},
        "style":{"enum":["solid","dashed","dotted","dash_dot"]}},
        "description":"Line stroke; omitted fields keep the current value (new lines default to #000000, 0.75 pt or the theme's supporting-line width, dashed)."});
    let label_offset = json!({"type":"array","minItems":2,"maxItems":2,"items":{"type":"number","minimum":-crate::state::MAX_LABEL_OFFSET_PT,"maximum":crate::state::MAX_LABEL_OFFSET_PT},"description":"Pin the text [dx,dy] pt from at (x right, y up) instead of automatic placement; [0,0] draws it exactly at at."});
    let placement = json!({"enum":["auto"],"description":"Return the label to automatic placement, which keeps it clear of data, the legend and other labels."});
    let arrow_head = json!({"enum":["none","open","filled","double"],"description":"Arrowhead: none (plain line), open V or filled triangle at `to`, or double (filled at both ends). Default filled."});
    let arrow_width = json!({"type":"number","minimum":crate::state::reference_line_width_bound(*crate::state::ARROW_LINE_WIDTH_PT.start()),"maximum":crate::state::reference_line_width_bound(*crate::state::ARROW_LINE_WIDTH_PT.end()),"description":"Shaft stroke width in pt; default 1, or the theme's supporting-line width."});
    let arrow_color = json!({"type":"string","pattern":"^#[0-9a-fA-F]{6}$","description":"'#rrggbb'; default black."});
    let arrow_tail = json!({"type":"array","minItems":2,"maxItems":2,"items":number,"description":"Finite [x,y] arrow tail point in the plot's source data coordinates (the units in its coordinate_units). The arrow follows axis range, zoom and pan and is clipped to the plot area."});
    let arrow_to = json!({"type":"array","minItems":2,"maxItems":2,"items":number,"description":"Finite [x,y] arrow head point in the same source data coordinates as `at` (the tail); must differ from `at`."});
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
            "panel_stack",
            vec!["panel", "shared_x"],
            json!({"panel":target,"shared_x":{"type":"boolean","description":"true stacks the Panel's plots top to bottom in its content order on one shared x axis; false returns it to free layout, keeping the frames."},"gap_pt":{"type":"number","minimum":0,"maximum":144,"description":"Space between stacked plots, only with shared_x true. Defaults to 0 (plots touch) when entering the stack and keeps the current gap otherwise."},"merge":{"type":"array","maxItems":199,"items":target,"description":"Other Panels, e.g. one per plot from plot.create, merged into `panel` first; only with shared_x true. Merged plots stack top to bottom in their page order; without merge the Panel's content order is kept."}}),
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
            json!({"target":target,"kind":{"const":"text"},"text":annotation_text,"at":data_point,"font_pt":font}),
        ),
        variant(
            "annotation_create",
            vec!["target", "kind", "orientation", "value"],
            json!({"target":target,"kind":{"const":"reference_line"},"orientation":orientation,"value":line_value,"label":line_label,"font_pt":font,"line":line_style}),
        ),
        variant(
            "annotation_create",
            vec!["target", "kind", "at", "to"],
            json!({"target":target,"kind":{"const":"arrow"},"at":arrow_tail,"to":arrow_to,"head":arrow_head,"line_width_pt":arrow_width,"color":arrow_color}),
        ),
        {
            let mut set = variant(
                "annotation_set",
                vec!["target", "annotation_id"],
                json!({"target":target,"annotation_id":annotation_id,"text":annotation_text,"at":data_point,"font_pt":font,"orientation":orientation,"value":line_value,"label":line_label,"line":line_style,"to":arrow_to,"head":arrow_head,"line_width_pt":arrow_width,"color":arrow_color,"offset_pt":label_offset,"placement":placement,"override_hand_edits":{"type":"boolean","description":"Also re-place a label the user placed by hand (placement hand). Pass true only when the user asked for it."}}),
            );
            set["anyOf"] = json!(
                [
                    "text",
                    "at",
                    "font_pt",
                    "orientation",
                    "value",
                    "label",
                    "line",
                    "to",
                    "head",
                    "line_width_pt",
                    "color",
                    "offset_pt",
                    "placement"
                ]
                .map(|field| json!({"required":[field]}))
            );
            set["description"] = json!(
                "text, at and offset_pt apply to text annotations; orientation, value, label and line to reference lines; font_pt and placement to both; at, to, head, line_width_pt and color to arrows (at is the tail). Labels are placed clear of data automatically; a label the user dragged keeps its place through other edits, and offset_pt or placement on it needs override_hand_edits."
            );
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
    json!({"type":"object","additionalProperties":false,"required":["canvas","edits"],"properties":{"canvas":target,"edits":{"type":"array","minItems":1,"maxItems":200,"items":{"oneOf":variants}}},"description":"Atomic native authoring on one exact canvas. Panel frames [x,y,w,h] are page pt, resize scales child-local frames. New content frames are parent-local pt. Page size/margins/gutter use mm; margins [top,right,bottom,left]. Grid uses native Frame-basis cells in supplied Panel order, existing page margins/gutter; no page-layout preference change or non-target reflow. Align uses bounding edges/centres, distribution equal centre spacing with endpoints fixed. Panel layout ops require unlocked Panels with free or shared-x stack layout; new text/images need a free-layout parent. panel_stack needs at least two plots in the Panel and its merge Panels: members fill its inner width, heights keep their frame proportions, only the bottom plot draws x tick labels and x title and only the top plot a title, inner boundaries touch with inward x ticks, members get one x range covering all their data (x scales must match) and draw the bottom member's x direction, and x range, scale, display unit and thousands separator edits apply to every member or are rejected. content_frame requires the item and parent unlocked, a free-layout or shared-x stack parent (where only the height counts, as the member's share) and Auto height off; image_replace requires the item and parent unlocked, not a layout mode change. Deletion-only batches accept distinct content_delete, originally empty panel_delete and explicit-column series_delete targets on this canvas. Reject overlaps, last-Series removal, defaults/dynamic displays and mixing deletion with other ops. Preserve empty parents, datasets and assets; preflight lists exact deletions and relationship changes. Hidden exact targets remain addressable. Annotation ops target an exact unlocked plot in an unlocked parent, anchor text, reference lines (kind reference_line) or arrows (kind arrow: tail `at`, head `to`) in source data coordinates, allow at most 256 of any kind per plot and are allowed with Auto height. No top-level ids/targets."})
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
