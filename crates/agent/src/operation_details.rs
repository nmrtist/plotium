use serde_json::{Map, Value, json};

pub(crate) fn build(contract: &Value) -> Map<String, Value> {
    let schema = &contract["schema"];
    schema["properties"]["edits"]["items"]["oneOf"].as_array().expect("figure edit branches")
        .iter().map(|branch| {
            let op = branch["properties"]["op"]["const"].as_str().expect("edit op");
            let kind = branch["properties"]["kind"]["const"].as_str();
            let suffix = if op == "annotation_create" {
                format!("{op}.{}", kind.expect("annotation kind"))
            } else { op.to_owned() };
            let mut selected = schema.clone();
            selected["properties"]["edits"]["items"] = branch.clone();
            selected["description"] = json!(relevant(schema["description"].as_str().expect("figure edit description"), DESCRIPTION, op));
            let result = relevant(contract["result_contract"].as_str().expect("figure edit result contract"), RESULT, op);
            let purpose = match (op, kind) {
                ("page", _) => "Set page size, margins and gutter",
                ("panel_frame", _) => "Move or resize panels",
                ("content_frame", _) => "Move or resize a plot, text box or image",
                ("image_replace", _) => "Replace an image file in place",
                ("panel_label", _) => "Set a panel letter or label",
                ("panel_stack", _) => "Stack plots as shared-x subplots",
                ("align", _) => "Align panel edges or centres",
                ("distribute", _) => "Space panels evenly",
                ("grid", _) => "Arrange panels in a grid",
                ("text_create", _) => "Add a text box",
                ("text_set", _) => "Change the text of a text box",
                ("image_create", _) => "Add an image from a local file",
                ("annotation_create", Some("arrow")) => "Add an arrow in source data coordinates",
                ("annotation_create", Some("text")) => "Add a text label anchored to data",
                ("annotation_create", Some("reference_line")) => "Add a vertical or horizontal reference line",
                ("annotation_set", _) => "Change or move an existing annotation",
                ("annotation_delete", _) => "Remove an annotation",
                ("content_delete", _) => "Delete a plot, text box or image",
                ("panel_delete", _) => "Delete an empty panel",
                ("series_delete", _) => "Delete curves from a plot",
                _ => "",
            };
            let purpose = if purpose.is_empty() { suffix.replace('_', " ") } else { purpose.to_owned() };
            (format!("figure.edit/{suffix}"), json!({
                "purpose":purpose,"operation":"figure.edit","schema":selected,
                "preflight_permission":contract["preflight_permission"],
                "execute_permission":contract["execute_permission"],
                "result_contract":result,
                "entry":{"tool":"plotium_plan","tool_id":"figure.edit","selectors":"omit ids/targets; supply expected_project_context and expected_revision"},
                "example":{"tool_id":"figure.edit","parameters":example(op,kind)},
                "example_targets":"Replace example canvas, panel and content targets with full stable references from plotium_document_inspect; file paths must name your intended local file."
            }))
        }).collect()
}

const PANEL_LAYOUT: &[&str] = &["panel_frame", "grid", "align", "distribute"];
const NEW_CONTENT: &[&str] = &["text_create", "image_create"];
const DELETION: &[&str] = &["content_delete", "panel_delete", "series_delete"];
const ANNOTATION: &[&str] = &["annotation_create", "annotation_set", "annotation_delete"];
/// Routes a clause to every op.
const ALL: &[&str] = &[];

/// Clauses of the shared figure.edit description routed to the ops they
/// constrain, matched by prefix. Unmatched clauses go to every op.
const DESCRIPTION: &[(&str, &[&str])] = &[
    ("Atomic native authoring", ALL),
    ("Panel frames [x,y,w,h]", PANEL_LAYOUT),
    (
        "New content frames",
        &["text_create", "image_create", "content_frame"],
    ),
    ("Page size/margins/gutter", &["page"]),
    ("margins [top", &["page"]),
    ("Grid uses", &["grid"]),
    ("no page-layout preference change", &["grid"]),
    ("Align uses", &["align", "distribute"]),
    ("Panel layout ops require", PANEL_LAYOUT),
    ("new text/images need", NEW_CONTENT),
    ("panel_stack needs", &["panel_stack"]),
    ("content_frame requires", &["content_frame"]),
    ("image_replace requires", &["image_replace"]),
    ("Deletion-only batches", DELETION),
    ("Reject overlaps", DELETION),
    ("Preserve empty parents", DELETION),
    ("preflight lists exact deletions", DELETION),
    ("Hidden exact targets", ALL),
    ("Annotation ops", ANNOTATION),
    ("No top-level ids/targets", ALL),
];

/// Clauses of the figure.edit result contract, routed like [`DESCRIPTION`].
const RESULT: &[(&str, &[&str])] = &[
    ("ToolPlan.figure_plan/result", ALL),
    ("annotation_edits list", ANNOTATION),
    ("panel_edits list", &["panel_stack"]),
    ("content_frame edits only", &["content_frame"]),
    ("image_replace changes only", &["image_replace"]),
    ("Frozen native assets", &["image_create", "image_replace"]),
    ("one Undo/Redo restores", ALL),
    ("Deletion-only plans report", DELETION),
    ("Reject duplicates", DELETION),
    ("Source datasets/assets remain", DELETION),
    ("empty parents remain", DELETION),
    ("Selection-effect changes", DELETION),
    ("No automatic reflow", ALL),
];

fn sentences(text: &str) -> impl Iterator<Item = &str> {
    text.trim_end_matches('.').split(". ")
}

#[cfg(test)]
fn clauses(text: &str) -> impl Iterator<Item = &str> {
    sentences(text).flat_map(|sentence| sentence.split("; "))
}

fn applies(clause: &str, rules: &[(&str, &[&str])], op: &str) -> bool {
    rules
        .iter()
        .find(|(prefix, _)| clause.starts_with(prefix))
        .is_none_or(|(_, ops)| ops.is_empty() || ops.contains(&op))
}

/// The clauses of `text` that apply to `op`, so a detail does not repeat
/// rules for every other edit.
fn relevant(text: &str, rules: &[(&str, &[&str])], op: &str) -> String {
    let kept: Vec<String> = sentences(text)
        .filter_map(|sentence| {
            let parts: Vec<_> = sentence.split("; ").collect();
            let kept: Vec<_> = parts
                .iter()
                .copied()
                .filter(|c| applies(c, rules, op))
                .collect();
            let first = *kept.first()?;
            let mut sentence = kept.join("; ");
            // A dropped leading clause leaves a lowercase start; operation names stay as written.
            let word = first.split(' ').next().unwrap_or_default();
            if first != parts[0] && !word.contains('_') {
                sentence[..1].make_ascii_uppercase();
            }
            Some(sentence)
        })
        .collect();
    format!("{}.", kept.join(". "))
}

fn target(kind: &str, suffix: &str) -> Value {
    const CANVAS: &str = "00000000-0000-0000-0000-000000000001";
    let mut resource = json!({"id":format!("{CANVAS}{suffix}"),"kind":kind});
    if !suffix.is_empty() {
        resource["parent_id"] = json!(CANVAS);
        resource["local_id"] = json!(suffix.rsplit('/').next().expect("local suffix"));
    }
    json!({"resource":resource})
}

fn example(op: &str, kind: Option<&str>) -> Value {
    let canvas = target(plotium_core::automation::KIND_CANVAS, "");
    let panel = target(plotium_core::automation::KIND_PANEL, "/panels/1");
    let other = target(plotium_core::automation::KIND_PANEL, "/panels/2");
    let content = target(plotium_core::automation::KIND_CANVAS_OBJECT, "/1");
    let mut edit = match op {
        "page" => json!({"size_mm":[180,90]}),
        "panel_frame" => json!({"target":panel,"frame_pt":[10,10,200,120]}),
        "content_frame" => json!({"target":content,"frame_pt":[10,10,200,120]}),
        "image_replace" => json!({"target":content,"path":"/absolute/figure.png"}),
        "panel_label" => json!({"target":panel,"text":"a","visible":true}),
        "panel_stack" => json!({"panel":panel,"shared_x":true}),
        "align" => json!({"panels":[panel,other],"edge":"left"}),
        "distribute" => json!({"panels":[panel,other],"axis":"vertical"}),
        "grid" => json!({"panels":[panel,other],"rows":1,"cols":2}),
        "text_create" => {
            json!({"parent":canvas,"name":"Caption","frame_pt":[10,10,200,20],"text":"Caption","font_pt":12})
        }
        "text_set" => json!({"target":content,"text":"Caption"}),
        "image_create" => {
            json!({"parent":canvas,"name":"Image","frame_pt":[10,10,200,120],"path":"/absolute/figure.png"})
        }
        "annotation_create" => match kind.expect("annotation branch") {
            "text" => {
                json!({"target":content,"kind":"text","text":"Label","at":[1,2],"font_pt":10})
            }
            "reference_line" => {
                json!({"target":content,"kind":"reference_line","orientation":"vertical","value":1})
            }
            "arrow" => json!({"target":content,"kind":"arrow","at":[1,2],"to":[2,3]}),
            _ => unreachable!("known annotation kinds"),
        },
        "annotation_set" => json!({"target":content,"annotation_id":0,"text":"Label"}),
        "annotation_delete" => json!({"target":content,"annotation_id":0}),
        "content_delete" => json!({"target":content}),
        "panel_delete" => json!({"target":panel}),
        "series_delete" => {
            let mut series = content;
            series["component"] = json!({"kind":"series","id":0});
            json!({"target":series})
        }
        _ => unreachable!("every figure edit needs a discovery example"),
    };
    edit["op"] = json!(op);
    json!({"canvas":canvas,"edits":[edit]})
}

#[cfg(test)]
#[path = "operation_details_tests.rs"]
mod tests;
