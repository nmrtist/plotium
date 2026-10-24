//! Candidate pixels are checked against the same native action, not a second edit model.
use crate::{Session, guide_fixture};
use plotium_core::data::BlockStore;
use plotium_core::{automation::*, project::canvas_to_view, state::*};
use serde_json::{Value, json};

pub(super) fn fixture() -> (PlotiumApp, Session, Vec<String>) {
    let (mut a, s, ids) = guide_fixture::fixture();
    let c = &mut a.doc.canvases[0];
    let id = c.allocate_object_id();
    c.objects.push(ContentItem {
        id,
        name: "Duplicate caption".into(),
        frame: ObjectFrame::new(20., 220., 35., 18.),
        locked: false,
        visible: true,
        kind: ContentKind::Text(TextBox::label("A caption to widen".into())),
    });
    c.create_panel("Empty".into(), ObjectFrame::new(400., 240., 30., 10.));
    a.session.active_canvas = Some(0);
    a.clear_history();
    (a, s, ids)
}
pub(super) fn image_args(a: &PlotiumApp) -> Value {
    json!({"expected_project_context":a.session.project_context,"expected_revision":a.doc.automation_revision,"target":TargetRef::resource(a.doc.canvases[0].resource_id.into()),"max_edge":480})
}
pub(super) fn preview_args(a: &PlotiumApp, plan: &Value) -> Value {
    let mut p = image_args(a);
    p["plan_id"] = plan["plan_id"].clone();
    p
}
pub(super) fn edit_args(a: &PlotiumApp, edits: Value) -> Value {
    json!({"expected_project_context":a.session.project_context,"expected_revision":a.doc.automation_revision,"tool_id":FIGURE_EDIT,"parameters":{"canvas":TargetRef::resource(a.doc.canvases[0].resource_id.into()),"edits":edits}})
}
pub(super) fn target(a: &PlotiumApp, index: usize) -> TargetRef {
    TargetRef::resource(canvas_object_ref(
        a.doc.canvases[0].resource_id,
        a.doc.canvases[0].objects[index].id,
    ))
}
/// Persisted native presentation + non-persisted provenance/geometry + exact data blocks.
/// Persistent audit and save state are checked alongside candidate contents.
pub(super) fn content(a: &PlotiumApp) -> Value {
    json!({"pages":a.doc.canvases.iter().map(|c| {
        json!({"view":canvas_to_view(&a.doc.datasets,c,"test").unwrap(),
            "runtime":c.objects.iter().filter_map(|o|o.plot()).map(|p| format!("{:?}/{:?}",p.figure(),p.derived_axes())).collect::<Vec<_>>(),
            "selected":c.selected_object,"allocators":[c.next_object_id.get(),c.next_group_id,c.next_panel_label_slot]})
    }).collect::<Vec<_>>(),
    "data":a.doc.datasets.iter().map(|d| {let t=d.as_table().unwrap();json!({"id":t.resource_id,"catalog":t.field_catalog,"envelope":t.typed_state.envelope,
        "blocks":t.typed_state.envelope.referenced_blocks().into_iter().map(|h|t.typed_state.store.get(h).unwrap().to_vec()).collect::<Vec<_>>()})}).collect::<Vec<_>>(),
    "assets":a.doc.assets.values().map(|a|format!("{a:?}")).collect::<Vec<_>>(),
    "style":format!("{:?}",a.doc.style_library),"path":a.doc.project_path,"project_revision":a.doc.project_revision})
}
fn state(a: &PlotiumApp) -> Value {
    json!({"content":content(a),"revision":a.doc.automation_revision,"selection":format!("{:?}/{:?}/{:?}/{:?}",a.session.ui.selection,a.session.ui.hierarchical_selection,a.session.ui.selection_anchors,a.session.ui.panel_label_selection),
        "active":a.session.active_canvas,"history":[a.session.undo_stack.len(),a.session.redo_stack.len()],
        "history_storage":[format!("{:p}",a.session.undo_stack.as_ptr()),format!("{:p}",a.session.redo_stack.as_ptr())],
        "generation":a.doc.canvases.iter().flat_map(|c|c.objects.iter()).filter_map(|o|o.plot()).map(|p|p.figure_geometry_generation()).collect::<Vec<_>>()})
}
pub(super) fn roundtrip(a: &mut PlotiumApp, s: &mut Session, request: Value) -> Value {
    let plan = s.call(a, "plotium_plan", request).unwrap();
    let before = state(a);
    let original = content(a);
    let history = a.session.undo_stack.len();
    let runs = a.doc.automation_runs.len();
    let generation = a.doc.edit_generation;
    let dirty = a.doc.dirty;
    let p = preview_args(a, &plan);
    let candidate = s.call(a, "plotium_preview_plan", p.clone()).unwrap();
    assert_eq!(state(a), before);
    assert_eq!(a.doc.dirty, dirty);
    assert_eq!(a.doc.edit_generation, generation);
    assert_eq!(a.doc.automation_runs.len(), runs);
    assert!(
        a.doc
            .automation_runs
            .iter()
            .flat_map(|r| &r.nodes)
            .all(|n| n.result.value.get("png_base64").is_none())
    );
    let v = &candidate["value"];
    assert_eq!(v["candidate"], true);
    assert_eq!(v["plan_id"], plan["plan_id"]);
    assert_eq!(v["basis_project_context"], json!(a.session.project_context));
    assert_eq!(v["basis_revision"], a.doc.automation_revision);
    assert!(v.get("render_revision").is_none());
    assert_eq!(v["width_px"], 480);
    assert_eq!(
        s.call(a, "plotium_preview_plan", p.clone()).unwrap()["value"],
        *v
    );
    assert_eq!(state(a), before);
    let image = crate::protocol::image_result(
        candidate.clone(),
        &serde_json::from_value(json!(1)).unwrap(),
    );
    assert_eq!(
        image
            .content
            .iter()
            .filter(|c| c.as_image().is_some())
            .count(),
        1
    );
    s.call(a, "plotium_execute", json!({"plan_id":plan["plan_id"]}))
        .unwrap();
    let args = image_args(a);
    let committed = s.call(a, "plotium_render_preview", args).unwrap();
    // Same native renderer/font environment gives deterministic PNG encoding; equality is stronger than decoded pixels.
    assert_eq!(v["png_base64"], committed["value"]["png_base64"]);
    let after = content(a);
    assert_eq!(a.session.undo_stack.len(), history + 1);
    a.undo();
    assert_eq!(content(a), original);
    a.redo();
    assert_eq!(content(a), after);
    assert_eq!(
        s.call(a, "plotium_execute", json!({"plan_id":plan["plan_id"]}))
            .unwrap_err()
            .code,
        "expired_plan"
    );
    plan
}
#[test]
fn candidate_all_guide_properties_and_resets_are_readonly_and_match_execution() {
    let (mut a, mut s, ids) = fixture();
    for (key, value) in [
        ("visibility", json!("hide")),
        ("placement", json!("inside")),
        ("layout", json!("horizontal")),
        ("title", json!("Samples")),
        ("position", json!([0.1, 0.9])),
    ] {
        let key = format!("object.guides.{key}");
        let args = guide_fixture::args(&a, &ids, &key, value);
        roundtrip(&mut a, &mut s, args);
        let mut args = guide_fixture::args(&a, &ids, &key, Value::Null);
        args["tool_id"] = json!("properties.reset");
        args["parameters"] = json!({"key":key});
        roundtrip(&mut a, &mut s, args);
    }
}
#[test]
fn candidate_native_content_and_deletion_batches_match_with_automatic_axes() {
    let (mut a, mut s, _) = fixture();
    // Remove authored ranges so rebuild must recompute them after Series deletion.
    let plot = a.doc.canvases[0].objects[0].id;
    let mut o = a.doc.canvases[0].objects[0]
        .plot()
        .unwrap()
        .axis_overrides
        .clone();
    o.x_range = None;
    o.y_range = None;
    a.set_axis_overrides_value(0, plot, &o);
    let t = target(&a, 2);
    let q = edit_args(
        &a,
        json!([{"op":"content_frame","target":t,"frame_pt":[20,220,220,18]},{"op":"text_set","target":t,"text":"Updated native caption"}]),
    );
    roundtrip(&mut a, &mut s, q);
    let q = edit_args(
        &a,
        json!([{"op":"content_frame","target":target(&a,0),"frame_pt":[0,0,230,210]}]),
    );
    roundtrip(&mut a, &mut s, q);
    let series = a.series_targets(0, plot)[5].clone();
    let empty = TargetRef::resource(panel_resource(
        &a.doc.canvases[0],
        &a.doc.canvases[0].panels[2],
    ));
    let q = edit_args(
        &a,
        json!([{"op":"content_delete","target":t},{"op":"series_delete","target":series},{"op":"panel_delete","target":empty}]),
    );
    roundtrip(&mut a, &mut s, q);
}
#[test]
fn candidate_image_replacement_uses_frozen_bytes_and_preserves_shared_asset() {
    let (mut a, mut s, _) = fixture();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("replacement.png");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let q = edit_args(
        &a,
        json!([{"op":"image_create","parent":image_args(&a)["target"],"name":"image","frame_pt":[25,200,40,40],"path":root.join("icon-256.png")},{"op":"image_create","parent":image_args(&a)["target"],"name":"retain","frame_pt":[300,200,40,40],"path":root.join("icon-256.png")} ]),
    );
    let p = s.call(&mut a, "plotium_plan", q).unwrap();
    s.call(&mut a, "plotium_execute", json!({"plan_id":p["plan_id"]}))
        .unwrap();
    std::fs::copy(root.join("icon-256-macos.png"), &path).unwrap();
    let q = edit_args(
        &a,
        json!([{"op":"image_replace","target":target(&a,3),"path":path}]),
    );
    let p = s.call(&mut a, "plotium_plan", q).unwrap();
    std::fs::write(&path, b"bad replacement after preflight").unwrap();
    let before = state(&a);
    let args = preview_args(&a, &p);
    let v = s.call(&mut a, "plotium_preview_plan", args).unwrap();
    assert_eq!(state(&a), before);
    std::fs::remove_file(&path).unwrap();
    s.call(&mut a, "plotium_execute", json!({"plan_id":p["plan_id"]}))
        .unwrap();
    let args = image_args(&a);
    let committed = s.call(&mut a, "plotium_render_preview", args).unwrap();
    assert_eq!(v["value"]["png_base64"], committed["value"]["png_base64"]);
    assert_eq!(a.doc.assets.len(), 2);
    let args = edit_args(
        &a,
        json!([{"op":"content_frame","target":target(&a,3),"frame_pt":[30,195,45,60]}]),
    );
    roundtrip(&mut a, &mut s, args);
}

#[test]
fn candidate_multi_canvas_batch_and_auto_height_use_native_dependencies() {
    let (mut a, mut s, _) = fixture();
    let mut other = a.doc.canvases[0].clone();
    other.resource_id = CanvasId::new();
    other.auto_height = true;
    other.size_mm[1] = 200.;
    a.doc.canvases.push(other);
    let ids: Vec<_> = a
        .doc
        .canvases
        .iter()
        .map(|c| canvas_object_ref(c.resource_id, c.objects[0].id).id)
        .collect();
    let q = guide_fixture::args(&a, &ids, "object.guides.position", json!([0., 0.]));
    let p = s.call(&mut a, "plotium_plan", q).unwrap();
    let before = state(&a);
    let mut candidates = vec![];
    for ci in 0..2 {
        let mut q = preview_args(&a, &p);
        q["target"] = json!(TargetRef::resource(a.doc.canvases[ci].resource_id.into()));
        candidates.push(s.call(&mut a, "plotium_preview_plan", q).unwrap());
        assert_eq!(state(&a), before);
    }
    s.call(&mut a, "plotium_execute", json!({"plan_id":p["plan_id"]}))
        .unwrap();
    a.reconcile_page_fit();
    for (ci, candidate) in candidates.iter().enumerate() {
        let mut q = image_args(&a);
        q["target"] = json!(TargetRef::resource(a.doc.canvases[ci].resource_id.into()));
        let r = s.call(&mut a, "plotium_render_preview", q).unwrap();
        assert_eq!(candidate["value"]["png_base64"], r["value"]["png_base64"]);
        assert_eq!(
            candidate["value"]["output"]["size_mm"],
            json!(a.doc.canvases[ci].size_mm)
        );
    }
    assert_eq!(a.session.undo_stack.len(), 1);
}

#[test]
fn candidate_bad_assets_and_snapshot_budget_fail_without_partial_state() {
    let (mut a, mut s, _) = fixture();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/icon-256.png");
    let q = edit_args(
        &a,
        json!([{"op":"image_create","parent":image_args(&a)["target"],"name":"bad image","frame_pt":[25,200,40,40],"path":source}]),
    );
    let p = s.call(&mut a, "plotium_plan", q).unwrap();
    s.call(&mut a, "plotium_execute", json!({"plan_id":p["plan_id"]}))
        .unwrap();
    a.doc.assets.values_mut().next().unwrap().bytes = b"not an image".to_vec();
    let q = edit_args(
        &a,
        json!([{"op":"text_set","target":target(&a,2),"text":"New"}]),
    );
    let p = s.call(&mut a, "plotium_plan", q).unwrap();
    let before = state(&a);
    let q = preview_args(&a, &p);
    assert_eq!(
        s.call(&mut a, "plotium_preview_plan", q).unwrap_err().code,
        "preview_asset_unavailable"
    );
    assert_eq!(state(&a), before);
    // Large authored strings are rejected before constructing a native candidate.
    a.doc.canvases[0].objects[2].text_mut().unwrap().text =
        "x".repeat(CANDIDATE_SNAPSHOT_BYTES + 1);
    let q = preview_args(&a, &p);
    let revision = a.doc.automation_revision;
    let history = a.session.undo_stack.len();
    assert_eq!(
        s.call(&mut a, "plotium_preview_plan", q).unwrap_err().code,
        "preview_budget_exceeded"
    );
    assert_eq!(a.doc.automation_revision, revision);
    assert_eq!(a.session.undo_stack.len(), history);
}

#[test]
fn candidate_preserves_existing_redo_and_selected_objects() {
    let (mut a, mut s, _) = fixture();
    let before_name = a.doc.canvases[0].name.clone();
    a.execute_action(plotium_core::actions::Action::rename_canvas(
        0,
        before_name,
        "Manual rename".into(),
    ));
    a.undo();
    let id = a.doc.canvases[0].objects[2].id;
    a.doc.canvases[0].selected_object = Some(id);
    a.session.ui.selection = Selection::Objects(vec![id]);
    let q = edit_args(&a, json!([{"op":"content_delete","target":target(&a,2)}]));
    let p = s.call(&mut a, "plotium_plan", q).unwrap();
    let before = state(&a);
    let q = preview_args(&a, &p);
    s.call(&mut a, "plotium_preview_plan", q).unwrap();
    assert_eq!(state(&a), before);
    a.redo();
    assert_eq!(a.doc.canvases[0].name, "Manual rename");
    assert_eq!(a.doc.canvases[0].selected_object, Some(id));
    assert_eq!(
        s.call(&mut a, "plotium_execute", json!({"plan_id":p["plan_id"]}))
            .unwrap_err()
            .code,
        "stale_revision"
    );
}

#[test]
fn candidate_deletion_preserves_selection_context_on_an_unaffected_active_canvas() {
    let (mut a, mut s, _) = fixture();
    a.doc.canvases.push(CanvasDocument::new(
        "Other active page".into(),
        [100., 100.],
    ));
    let id = a.doc.canvases[0].objects[2].id;
    a.select_content(0, id);
    a.session.active_canvas = Some(1);
    let q = edit_args(&a, json!([{"op":"content_delete","target":target(&a,2)}]));
    roundtrip(&mut a, &mut s, q);
    assert_eq!(a.session.active_canvas, Some(1));
}

/// One `properties.set_many` plan per style variant over the first plot.
fn variant_args(a: &PlotiumApp, writes: impl Fn(TargetRef, usize) -> Vec<Value>) -> Value {
    let plot = &a.doc.canvases[0].objects[0];
    let writes: Vec<Value> = plot
        .plot()
        .unwrap()
        .binding
        .series
        .iter()
        .enumerate()
        .flat_map(|(i, s)| writes(a.series_target(0, plot.id, s.id).unwrap(), i))
        .collect();
    json!({"expected_project_context":a.session.project_context,"expected_revision":a.doc.automation_revision,
        "tool_id":"properties.set_many","parameters":{"writes":writes}})
}

#[test]
fn candidate_style_variants_preview_exactly_what_each_would_commit() {
    let (mut a, mut s, _) = fixture();
    let palettes = [
        [
            "#0072B2", "#E69F00", "#009E73", "#CC79A7", "#56B4E9", "#D55E00",
        ],
        [
            "#1B9E77", "#D95F02", "#7570B3", "#E7298A", "#66A61E", "#E6AB02",
        ],
        [
            "#000000", "#555555", "#888888", "#AAAAAA", "#333333", "#777777",
        ],
    ];
    for palette in palettes {
        let args = variant_args(&a, |target, i| {
            vec![
                json!({"target":target,"key":"series.line.color","value":palette[i]}),
                json!({"target":target,"key":"series.line.stroke_width","value":0.5 + i as f64 * 0.25}),
            ]
        });
        roundtrip(&mut a, &mut s, args);
    }
    // Plot-level and document-wide display properties preview too.
    let plot = target(&a, 0);
    let document = TargetRef::resource(plotium_core::automation::document_root_target().resource);
    let args = json!({"expected_project_context":a.session.project_context,"expected_revision":a.doc.automation_revision,
        "tool_id":"properties.set_many","parameters":{"writes":[
            {"target":plot,"key":"object.axes.y_label","value":"Current (mA)"},
            {"target":document,"key":"document.figure.typography.tick_pt","value":9.0}]}});
    roundtrip(&mut a, &mut s, args);
}

#[test]
fn candidate_variants_are_compared_while_pending_and_only_the_pick_commits() {
    let (mut a, mut s, _) = fixture();
    let plans: Vec<Value> = [0.5, 1.0, 1.5]
        .into_iter()
        .map(|width| {
            let args = variant_args(&a, |target, _| {
                vec![json!({"target":target,"key":"series.line.stroke_width","value":width})]
            });
            s.call(&mut a, "plotium_plan", args).unwrap()
        })
        .collect();
    let before = content(&a);
    let images: Vec<Value> = plans
        .iter()
        .map(|p| {
            let q = preview_args(&a, p);
            s.call(&mut a, "plotium_preview_plan", q).unwrap()["value"]["png_base64"].clone()
        })
        .collect();
    assert_eq!(content(&a), before);
    assert!(images[0] != images[1] && images[1] != images[2]);
    s.call(
        &mut a,
        "plotium_execute",
        json!({"plan_id":plans[1]["plan_id"]}),
    )
    .unwrap();
    let args = image_args(&a);
    let committed = s.call(&mut a, "plotium_render_preview", args).unwrap();
    assert_eq!(committed["value"]["png_base64"], images[1]);
    let q = preview_args(&a, &plans[0]);
    assert_eq!(
        s.call(&mut a, "plotium_preview_plan", q).unwrap_err().code,
        "stale_revision"
    );
}
