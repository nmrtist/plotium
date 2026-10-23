use plotium_core::{automation::*, state::*};
use serde_json::{Value, json};

fn fixture() -> (PlotiumApp, TargetRef) {
    let (mut a, _) = crate::component_tests::fixture();
    a.doc.canvases.truncate(1);
    let page = &mut a.doc.canvases[0];
    let id = page.allocate_object_id();
    page.objects.push(ContentItem {
        id,
        name: "same".into(),
        frame: ObjectFrame::new(4., 5., 40., 20.),
        locked: false,
        visible: true,
        kind: ContentKind::Text(TextBox::label("caption".into())),
    });
    let parent = page.create_panel("Parent".into(), ObjectFrame::new(30., 40., 100., 80.));
    page.panel_mut(parent).unwrap().item_order.push(id);
    let t = TargetRef::resource(canvas_object_ref(page.resource_id, id));
    a.clear_history();
    (a, t)
}
fn args(a: &PlotiumApp, t: &TargetRef) -> Value {
    json!({"expected_project_context":a.session.project_context,"expected_revision":a.doc.automation_revision,"tool_id":FIGURE_EDIT,"parameters":{"canvas":TargetRef::resource(a.doc.canvases[0].resource_id.into()),"edits":[{"op":"content_frame","target":t,"frame_pt":[-8.,-9.,80.,30.]}]}})
}
fn inspect(s: &mut crate::Session, a: &mut PlotiumApp, t: &TargetRef) -> Value {
    let args = json!({"expected_project_context":a.session.project_context,"expected_revision":a.doc.automation_revision,"target":t,"view":"self"});
    s.call(a, "plotium_document_inspect", args).unwrap()["value"]["node"].clone()
}

#[test]
fn content_edit_discovery_preflight_exact_targets_and_single_use_guards() {
    let (mut a, t) = fixture();
    let mut s = crate::Session::default();
    let rev = a.doc.automation_revision;
    let history = a.session.undo_stack.len();
    let before = a.doc.canvases[0].clone();
    let detail = inspect(&mut s, &mut a, &t);
    assert_eq!(detail["frame_space"], "panel_local_pt");
    assert_eq!(detail["parent"]["resource"]["kind"], KIND_PANEL);
    assert_eq!(detail["page_frame"]["x"], 34.);
    assert_eq!(detail["geometry"]["min_size_pt"], json!([1., 1.]));
    assert_eq!(detail["geometry"]["frame_editable"], true);
    assert_eq!(a.doc.automation_revision, rev);
    assert_eq!(a.session.undo_stack.len(), history);
    let p = args(&a, &t);
    assert_eq!(
        s.call(&mut a, "plotium_plan", p).unwrap_err().code,
        "permission_denied"
    );
    s.allow_modify = true;
    let p = args(&a, &t);
    let planned = s.call(&mut a, "plotium_plan", p).unwrap();
    let entry = &planned["preflight"]["figure_plan"]["result"]["content_edits"][0];
    assert_eq!(entry["target"], json!(t));
    assert_eq!(entry["effective_frame_pt"], json!([-8., -9., 80., 30.]));
    assert_eq!(entry["page_frame"]["x"], 22.);
    assert_eq!(a.doc.canvases[0].panels, before.panels);
    assert_eq!(a.doc.automation_revision, rev);
    let result = s
        .call(
            &mut a,
            "plotium_execute",
            json!({"plan_id":planned["plan_id"]}),
        )
        .unwrap();
    assert_eq!(result["modified"].as_array().unwrap().len(), 1);
    assert_eq!(result["value"]["content_edits"][0], *entry);
    assert_eq!(
        s.call(
            &mut a,
            "plotium_execute",
            json!({"plan_id":planned["plan_id"]})
        )
        .unwrap_err()
        .code,
        "expired_plan"
    );
    a.undo();
    assert_eq!(inspect(&mut s, &mut a, &t)["frame"], detail["frame"]);
    a.redo();
    for mode in ["revoke", "busy", "revision", "context"] {
        let p = args(&a, &t);
        let planned = s.call(&mut a, "plotium_plan", p).unwrap();
        let args = json!({"plan_id":planned["plan_id"]});
        let expected = match mode {
            "revoke" => {
                s.allow_modify = false;
                "permission_denied"
            }
            "busy" => "busy",
            "revision" => {
                a.undo();
                "stale_revision"
            }
            _ => {
                a.session.project_context = ProjectContextId::default();
                "project_changed"
            }
        };
        assert_eq!(
            s.call_when_available(&mut a, "plotium_execute", args, mode == "busy")
                .unwrap_err()
                .code,
            expected
        );
        s.allow_modify = true;
    }
}

#[test]
fn content_edit_rejects_series_incomplete_frame_extra_fields_and_wrong_owner() {
    let (mut a, t) = fixture();
    let mut s = crate::Session::default();
    s.allow_modify = true;
    for change in [
        json!({"frame_pt":[1,2,3]}),
        json!({"frame_pt":[1,2,0.5,3]}),
        json!({"dx":3}),
        json!({"target":{"resource":t.resource,"component":{"kind":"series","id":0}}}),
        json!({"target":{"resource":{"id":t.resource.id,"kind":t.resource.kind,"parent_id":"other","local_id":t.resource.local_id}}}),
    ] {
        let mut p = args(&a, &t);
        for (key, value) in change.as_object().unwrap() {
            p["parameters"]["edits"][0][key] = value.clone();
        }
        assert_eq!(
            s.call(&mut a, "plotium_plan", p).unwrap_err().code,
            "invalid_parameters"
        );
        assert!(!a.can_undo());
    }
    a.doc.canvases[0].panels[0].locked = true;
    let detail = inspect(&mut s, &mut a, &t);
    assert_eq!(detail["geometry"]["frame_editable"], false);
    assert!(
        detail["geometry"]["frame_edit_error"]
            .as_str()
            .unwrap()
            .contains("locked")
    );
}

#[test]
fn content_image_replacement_uses_public_schema_and_discloses_native_attributes() {
    let (mut a, t) = fixture();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/icon-256.png");
    // image_create exercises the same native I/O/freeze path, without a new test dependency.
    let mut s = crate::Session::default();
    s.allow_modify = true;
    let mut request = args(&a, &t);
    request["parameters"]["edits"] = json!([{"op":"image_create","parent":request["parameters"]["canvas"],"name":"image","frame_pt":[10,20,40,20],"path":path}]);
    let p = s.call(&mut a, "plotium_plan", request).unwrap();
    let r = s
        .call(&mut a, "plotium_execute", json!({"plan_id":p["plan_id"]}))
        .unwrap();
    let image: TargetRef = serde_json::from_value(r["value"]["created"][0].clone()).unwrap();
    let before = inspect(&mut s, &mut a, &image);
    assert_eq!(before["parent"]["resource"]["kind"], KIND_CANVAS);
    assert_eq!(before["image"]["preserve_aspect"], true);
    assert_eq!(before["image"]["fit"], "contain");
    assert_eq!(before["image"]["replaceable"], true);
    let mut request = args(&a, &t);
    request["parameters"]["edits"] = json!([{"op":"image_replace","target":image,"path":path}]);
    let p = s.call(&mut a, "plotium_plan", request).unwrap();
    let entry = &p["preflight"]["figure_plan"]["result"]["content_edits"][0];
    assert_eq!(
        entry["asset_change"]["before"],
        entry["asset_change"]["after"]
    );
    s.call(&mut a, "plotium_execute", json!({"plan_id":p["plan_id"]}))
        .unwrap();
    assert_eq!(inspect(&mut s, &mut a, &image), before);
    let state = s
        .call(
            &mut a,
            "plotium_capabilities_get",
            json!({"section":"operation_constraints","key":FIGURE_EDIT}),
        )
        .unwrap();
    let ops = state["contract"]["schema"]["properties"]["edits"]["items"]["oneOf"]
        .as_array()
        .unwrap();
    for op in ["content_frame", "image_replace"] {
        assert!(ops.iter().any(|v| v["properties"]["op"]["const"] == op));
    }
}

#[test]
fn content_and_guide_capabilities_remain_discoverable_together() {
    let (mut a, _) = fixture();
    for allow_modify in [false, true] {
        let mut s = crate::Session::default();
        s.allow_modify = allow_modify;
        let mut section = |section: &str, key: Option<&str>| {
            let mut args = json!({ "section": section });
            if let Some(key) = key {
                args["key"] = json!(key);
            }
            s.call(&mut a, "plotium_capabilities_get", args).unwrap()["contract"].take()
        };
        let edit = &section("operation_constraints", Some(FIGURE_EDIT));
        let guide = section("guide_contract", None);
        assert!(edit.get("currently_permitted").is_none());
        assert_eq!(edit["schema"], figure_edit_schema());
        let ops = edit["schema"]["properties"]["edits"]["items"]["oneOf"]
            .as_array()
            .unwrap();
        for op in ["content_frame", "image_replace"] {
            assert!(ops.iter().any(|v| v["properties"]["op"]["const"] == op));
            assert!(edit["result_contract"].as_str().unwrap().contains(op));
        }
        assert_eq!(guide["limits"]["targets"], 200);
        assert_eq!(guide["limits"]["title_utf8_bytes"], 16384);
        assert!(
            guide["coordinates"]
                .as_str()
                .unwrap()
                .contains("plot_size - legend_size")
        );
        for key in ["visibility", "placement", "layout", "title", "position"] {
            let id = format!("object.guides.{key}");
            assert!(crate::session::PROPERTIES.iter().any(|p| p.as_str() == id));
        }
    }
}
