use super::*;
use crate::automation::*;
use plotium_figure::{LineStyle, ReferenceOrientation};
use serde_json::{Value, json};

fn fixture() -> (PlotiumApp, TargetRef) {
    let mut a = super::curve_tests::fixture(false);
    let page = &a.doc.canvases[0];
    let plot = page
        .objects
        .iter()
        .find(|o| o.plot().is_some())
        .expect("fixture plot");
    let target = TargetRef::resource(canvas_object_ref(page.resource_id, plot.id));
    a.clear_history();
    (a, target)
}

fn try_plan(a: &PlotiumApp, edits: Value) -> Result<FigureEditPlan, String> {
    let canvas = TargetRef::resource(a.doc.canvases[0].resource_id.into());
    let params = serde_json::from_value(json!({"canvas":canvas,"edits":edits}))
        .map_err(|e| e.to_string())?;
    a.plan_figure_edit(DocumentRevision(a.doc.automation_revision), &params)
}

fn commit(a: &mut PlotiumApp, edits: Value) -> Value {
    let plan = try_plan(a, edits).unwrap();
    let result = plan.result.clone();
    a.commit_figure_edit(plan).unwrap();
    result
}

fn plot(a: &PlotiumApp) -> &PlotObject {
    a.doc.canvases[0]
        .objects
        .iter()
        .find_map(ContentItem::plot)
        .unwrap()
}

#[test]
fn created_reference_line_uses_defaults_and_renders() {
    let (mut a, target) = fixture();
    let result = commit(
        &mut a,
        json!([{"op":"annotation_create","target":target,"kind":"reference_line","orientation":"vertical","value":1.0,"label":"onset"}]),
    );
    let edit = &result["annotation_edits"][0];
    assert_eq!(edit["op"], "created");
    assert_eq!(edit["annotation"]["kind"], "reference_line");
    assert_eq!(edit["annotation"]["orientation"], "vertical");
    assert_eq!(
        edit["annotation"]["line"],
        json!({"color":"#000000","width_pt":0.75,"style":"dashed"})
    );
    assert_eq!(edit["annotation"]["font_pt"], Value::Null);
    let thin = commit(
        &mut a,
        json!([{"op":"annotation_set","target":target,"annotation_id":0,"line":{"width_pt":0.3}}]),
    );
    assert_eq!(
        thin["annotation_edits"][0]["annotation"]["line"]["width_pt"],
        json!(0.3)
    );
    let figure = plot(&a).figure();
    assert_eq!(figure.reference_lines.len(), 1);
    assert_eq!(figure.reference_lines[0].line_style, LineStyle::Dashed);
    let svg = plotium_render::svg::export(figure);
    assert!(svg.contains(r#"class="reference-line""#));
    assert!(svg.contains("onset"));
}

#[test]
fn set_restyles_lines_and_ids_are_shared_with_text() {
    let (mut a, target) = fixture();
    commit(
        &mut a,
        json!([
            {"op":"annotation_create","target":target,"text":"peak","at":[1.0,1.0],"font_pt":8},
            {"op":"annotation_create","target":target,"kind":"reference_line","orientation":"horizontal","value":0.5,"label":"base"}
        ]),
    );
    let result = commit(
        &mut a,
        json!([{"op":"annotation_set","target":target,"annotation_id":1,"orientation":"vertical","value":2.0,"label":"","font_pt":9,
            "line":{"color":"#CC2211","width_pt":1.5,"style":"solid"}}]),
    );
    assert_eq!(result["annotation_edits"][0]["annotation"]["label"], "");
    let annotations = &plot(&a).axis_overrides.annotations;
    assert_eq!(annotations.items.len(), 1);
    let line = annotations
        .reference_line(AnnotationId::new(1))
        .expect("line");
    assert_eq!(line.orientation, ReferenceOrientation::Vertical);
    assert_eq!(line.value, 2.0);
    assert_eq!(line.color, plotium_figure::Color::rgb(0xcc, 0x22, 0x11));
    assert_eq!(line.width, 1.5);
    assert_eq!(line.line_style, LineStyle::Solid);
    assert_eq!(line.font_size, Some(9.0));
    assert!(line.label.is_empty());
    assert_eq!(plot(&a).figure().reference_lines[0].value, 2.0);

    // The text annotation keeps id 0; a new line takes the shared next id.
    let result = commit(
        &mut a,
        json!([{"op":"annotation_create","target":target,"kind":"reference_line","orientation":"vertical","value":3.0}]),
    );
    assert_eq!(result["annotation_edits"][0]["annotation_id"], 2);
    commit(
        &mut a,
        json!([{"op":"annotation_delete","target":target,"annotation_id":1}]),
    );
    assert_eq!(plot(&a).figure().reference_lines.len(), 1);
    assert_eq!(plot(&a).figure().author_annotations.len(), 1);

    a.undo();
    assert_eq!(plot(&a).figure().reference_lines.len(), 2);
    let request = ToolRequest {
        tool_id: DOCUMENT_INSPECT.into(),
        tool_version: 1,
        parameters: json!({"target":target,"view":"self"}),
        targets: TargetSelector::Explicit {
            ids: vec![target.resource.id],
        },
        expected_revision: DocumentRevision(a.doc.automation_revision),
        caller: CallerType::Agent,
    };
    let plan = plan_tool(&a, request).expect("inspect plan");
    let inspected = execute_tool(&mut a, plan, ExecutionAuthority::Read).expect("inspect");
    let kinds: Vec<_> = inspected.value["node"]["annotations"]
        .as_array()
        .expect("annotations")
        .iter()
        .map(|a| a["kind"].clone())
        .collect();
    assert_eq!(
        kinds,
        [
            json!("text"),
            json!("reference_line"),
            json!("reference_line")
        ]
    );
}

#[test]
fn invalid_reference_line_edits_reject_the_whole_batch() {
    let (mut a, target) = fixture();
    commit(
        &mut a,
        json!([
            {"op":"annotation_create","target":target,"text":"peak","at":[1.0,1.0],"font_pt":8},
            {"op":"annotation_create","target":target,"kind":"reference_line","orientation":"vertical","value":1.0}
        ]),
    );
    let line = |extra: Value| {
        let mut edit = json!({"op":"annotation_create","target":target,"kind":"reference_line","orientation":"vertical","value":1.0});
        edit.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        edit
    };
    for edit in [
        json!({"op":"annotation_create","target":target,"kind":"reference_line","value":1.0}),
        json!({"op":"annotation_create","target":target,"kind":"reference_line","orientation":"vertical"}),
        line(json!({"orientation":"diagonal"})),
        line(json!({"line":{"width_pt":0.0}})),
        line(json!({"line":{"color":"red"}})),
        line(json!({"line":{"dash":"dashed"}})),
        line(json!({"text":"x"})),
        line(json!({"at":[0.0,0.0]})),
        line(json!({"font_pt":100})),
        json!({"op":"annotation_create","target":target,"text":"t","at":[0.0,0.0],"font_pt":8,"value":1.0}),
        json!({"op":"annotation_create","target":target,"text":"t","at":[0.0,0.0]}),
        json!({"op":"annotation_set","target":target,"annotation_id":0,"value":2.0}),
        json!({"op":"annotation_set","target":target,"annotation_id":1,"text":"x"}),
        json!({"op":"annotation_set","target":target,"annotation_id":1}),
        json!({"op":"annotation_set","target":target,"annotation_id":9,"value":2.0}),
        json!({"op":"annotation_delete","target":target,"annotation_id":9}),
    ] {
        let edits = json!([
            {"op":"annotation_create","target":target,"kind":"reference_line","orientation":"horizontal","value":0.2},
            edit.clone()
        ]);
        assert!(try_plan(&a, edits).is_err(), "{edit}");
    }
    assert_eq!(plot(&a).axis_overrides.annotations.reference_lines.len(), 1);
}

#[test]
fn reference_lines_round_trip_through_the_project_file() {
    let (mut a, target) = fixture();
    commit(
        &mut a,
        json!([
            {"op":"annotation_create","target":target,"kind":"reference_line","orientation":"horizontal","value":0.25,"label":"noise","font_pt":7,
                "line":{"color":"#0f4d92","width_pt":0.5,"style":"dash_dot"}},
            {"op":"annotation_create","target":target,"kind":"reference_line","orientation":"vertical","value":1.5}
        ]),
    );
    let dir = std::env::temp_dir().join(format!("plotium-reference-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("reference.plotium");
    crate::project::save_project(&a, &path, false).unwrap();
    let loaded = crate::project::load_project(&path).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
    assert_eq!(
        plot(&loaded).axis_overrides.annotations,
        plot(&a).axis_overrides.annotations
    );
    assert_eq!(
        plot(&loaded).figure().reference_lines,
        plot(&a).figure().reference_lines
    );
}

#[test]
fn reference_lines_follow_the_axis_range_and_clip_off_axis() {
    let (mut a, target) = fixture();
    commit(
        &mut a,
        json!([{"op":"annotation_create","target":target,"kind":"reference_line","orientation":"vertical","value":1.0}]),
    );
    let object = a.doc.canvases[0]
        .objects
        .iter()
        .find(|o| o.plot().is_some())
        .unwrap()
        .id;
    let mut overrides = plot(&a).axis_overrides.clone();
    overrides.x_range = Some(AxisRange { min: 2.0, max: 4.0 });
    a.set_axis_overrides_value(0, object, &overrides);
    let figure = plot(&a).figure();
    assert_eq!(figure.reference_lines.len(), 1);
    assert!(!plotium_render::svg::export(figure).contains("reference-line"));
}

#[test]
fn schema_separates_the_kinds_and_states_exact_width_bounds() {
    let schema = figure_edit_schema();
    let creates: Vec<&Value> = schema["properties"]["edits"]["items"]["oneOf"]
        .as_array()
        .expect("variants")
        .iter()
        .filter(|v| v["properties"]["op"]["const"] == "annotation_create")
        .collect();
    assert_eq!(creates.len(), 3);
    let kinds: Vec<_> = creates
        .iter()
        .map(|v| v["properties"]["kind"]["const"].clone())
        .collect();
    assert_eq!(
        kinds,
        [json!("text"), json!("reference_line"), json!("arrow")]
    );
    // Only the reference-line and arrow variants require kind, so legacy text payloads
    // without it still match exactly one variant.
    let requires_kind = |v: &Value| {
        v["required"]
            .as_array()
            .is_some_and(|r| r.contains(&json!("kind")))
    };
    assert!(!requires_kind(creates[0]) && requires_kind(creates[1]) && requires_kind(creates[2]));
    let width = &creates[1]["properties"]["line"]["properties"]["width_pt"];
    assert_eq!(width["minimum"], json!(0.1));
    assert_eq!(width["maximum"], json!(20.0));
    let width = &creates[2]["properties"]["line_width_pt"];
    assert_eq!(width["minimum"], json!(0.1));
    assert_eq!(width["maximum"], json!(20.0));
}
