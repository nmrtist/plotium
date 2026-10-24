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
            let purpose = match (op, kind) {
                ("page", _) => "Set page size, margins and gutter",
                ("panel_label", _) => "Set a panel letter or label",
                ("annotation_create", Some("arrow")) => "Add an arrow in source data coordinates",
                ("annotation_create", Some("text")) => "Add a text label anchored to data",
                ("annotation_create", Some("reference_line")) => "Add a vertical or horizontal reference line",
                _ => "",
            };
            let purpose = if purpose.is_empty() { suffix.replace('_', " ") } else { purpose.to_owned() };
            (format!("figure.edit/{suffix}"), json!({
                "purpose":purpose,"operation":"figure.edit","schema":selected,
                "preflight_permission":contract["preflight_permission"],
                "execute_permission":contract["execute_permission"],
                "result_contract":contract["result_contract"],
                "entry":{"tool":"plotium_plan","tool_id":"figure.edit","selectors":"omit ids/targets; supply expected_project_context and expected_revision"},
                "example":{"tool_id":"figure.edit","parameters":example(op,kind)},
                "example_targets":"Replace example canvas, panel and content targets with full stable references from plotium_document_inspect; file paths must name your intended local file."
            }))
        }).collect()
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
