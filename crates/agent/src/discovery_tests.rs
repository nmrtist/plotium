use crate::{
    capabilities,
    session::{Session, property_catalog},
};
use plotium_core::{automation::*, properties, state::*};
use serde_json::{Value, json};

fn fixture() -> PlotiumApp {
    let (mut app, _) = crate::component_tests::fixture();
    app.session.project_present = true;
    app.doc.canvases.truncate(1);
    let c = &mut app.doc.canvases[0];
    let mut object = c.objects[0].clone();
    object.id = c.allocate_object_id();
    c.objects.push(object);
    let ids: Vec<_> = c.objects.iter().map(|o| o.id).collect();
    for id in ids {
        c.create_panel_for_plot(id).unwrap();
    }
    app
}
fn inspect(
    s: &mut Session,
    app: &mut PlotiumApp,
    target: Value,
    view: &str,
) -> Result<Value, crate::Failure> {
    let context = app.session.project_context;
    s.call(
        app,
        "plotium_document_inspect",
        json!({"expected_project_context":context,"expected_revision":app.doc.automation_revision,"target":target,"view":view}),
    )
}
#[test]
fn session_discovery_walks_to_exact_series_and_preserves_original_tools() {
    let mut app = fixture();
    let context = app.session.project_context;
    let mut s = Session::default();
    let before = app.doc.automation_revision;
    let runs = app.doc.automation_runs.len();
    let root = inspect(&mut s, &mut app, json!(document_root_target()), "children").unwrap();
    let canvas = root["value"]["items"][0]["target"].clone();
    let panels = inspect(&mut s, &mut app, canvas, "children").unwrap();
    assert_eq!(panels["value"]["total"], 2);
    let p = panels["value"]["items"][1]["target"].clone();
    let objects = inspect(&mut s, &mut app, p.clone(), "children").unwrap();
    let o = objects["value"]["items"][0]["target"].clone();
    let curves = inspect(&mut s, &mut app, o, "children").unwrap();
    let t = curves["value"]["items"][1]["target"].clone();
    let detail = inspect(&mut s, &mut app, t.clone(), "self").unwrap();
    assert_eq!(detail["value"]["node"]["ancestors"][2], p);
    assert_eq!(detail["value"]["node"]["source"]["status"], "resolved");
    let property = s
        .call(
            &mut app,
            "plotium_properties_inspect",
            json!({"expected_project_context":context,"expected_revision":before,"targets":[t],"key":"series.line.stroke_width"}),
        )
        .unwrap();
    assert_eq!(property["value"]["readings"][0]["target"], t);
    assert_eq!(app.doc.automation_revision, before);
    assert!(app.doc.automation_runs.len() > runs);
    let state = s.call(&mut app, "plotium_get_state", json!({})).unwrap();
    assert_eq!(
        state["connection_capabilities"]["tools"]
            .as_array()
            .unwrap()
            .len(),
        15
    );
    let execute = state["connection_capabilities"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "plotium_execute")
        .unwrap();
    assert_eq!(execute["currently_permitted"], true);
    assert_eq!(
        execute["required_permission"],
        "operation_specific: reversible_modify or per_request_desktop_save/export_authorization"
    );
    s.allow_modify = true;
    let state = s.call(&mut app, "plotium_get_state", json!({})).unwrap();
    assert_eq!(
        state["connection_capabilities"]["tools"][6]["currently_permitted"],
        true
    );
}
#[test]
fn new_queries_reject_forgery_stale_selection_and_invalid_shape() {
    let mut app = fixture();
    let context = app.session.project_context;
    let mut s = Session::default();
    let target = json!(app.series_targets(0, app.doc.canvases[0].objects[0].id)[0]);
    for (key, value) in [
        (
            "resource",
            json!({"id":"document","kind":"plotium.document"}),
        ),
        ("component", json!({"kind":"series","id":999})),
    ] {
        let mut forged = target.clone();
        forged[key] = value;
        assert_eq!(
            inspect(&mut s, &mut app, forged, "self").unwrap_err().code,
            "invalid_parameters"
        );
    }
    for args in [
        json!({"expected_project_context":context,"expected_revision":app.doc.automation_revision,"view":"self"}),
        json!({"expected_project_context":context,"expected_revision":app.doc.automation_revision,"target":target,"view":"self","extra":true}),
        json!({"expected_project_context":context,"expected_revision":app.doc.automation_revision,"target":target,"view":"children","limit":201}),
        json!({"expected_project_context":context,"expected_revision":app.doc.automation_revision,"target":target,"view":"self","offset":1}),
    ] {
        assert_eq!(
            s.call(&mut app, "plotium_document_inspect", args)
                .unwrap_err()
                .code,
            "invalid_parameters"
        );
    }
    let old = app.doc.automation_revision;
    app.doc.automation_revision += 1;
    assert_eq!(
        s.call(
            &mut app,
            "plotium_document_inspect",
            json!({"expected_project_context":context,"expected_revision":old,"target":target,"view":"children"})
        )
        .unwrap_err()
        .code,
        "stale_revision"
    );
    assert_eq!(
        s.call_when_available(
            &mut app,
            "plotium_document_inspect",
            json!({"expected_project_context":context,"expected_revision":old+1,"target":target,"view":"self"}),
            true
        )
        .unwrap_err()
        .code,
        "busy"
    );
    app.doc.canvases[0].objects[0].name = "x".repeat(DOCUMENT_RESPONSE_BYTES);
    let t = json!(TargetRef::resource(canvas_object_ref(
        app.doc.canvases[0].resource_id,
        app.doc.canvases[0].objects[0].id
    )));
    assert_eq!(
        inspect(&mut s, &mut app, t, "self").unwrap_err().code,
        "response_too_large"
    );
}
#[test]
fn static_definitions_and_connection_gate_are_the_same_supported_subset() {
    let catalog = property_catalog();
    assert_eq!(
        catalog["properties"].as_array().unwrap().len(),
        crate::session::PROPERTIES.len()
    );
    for (i, id) in crate::session::PROPERTIES.iter().enumerate() {
        let row = &catalog["properties"][i];
        let d = properties::definition(*id).unwrap();
        assert_eq!(row["id"], id.as_str());
        assert_eq!(row["label"], d.canonical_label);
        assert_eq!(row["scope"], format!("{:?}", d.scope_kind));
        assert_eq!(
            row["applicability"]["component"],
            d.applicability.component.as_str()
        );
        assert!(row.get("value").is_none());
        assert!(row.get("availability").is_none());
        if matches!(
            d.value_schema,
            properties::ValueSchema::Float { .. } | properties::ValueSchema::Int { .. }
        ) {
            assert!(row["schema"]["min"].is_number());
            assert!(row["schema"]["max"].is_number());
        }
    }
    assert_eq!(catalog["properties"][0]["schema"]["unit"], "pt");
    assert_eq!(catalog["properties"][0]["schema"]["min"], 0.05);
    assert_eq!(catalog["properties"][2]["schema"]["type"], "int");
    assert_eq!(catalog["properties"][2]["schema"]["max"], 256);
    let manifest = capabilities::manifest(false);
    let plan = manifest["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "plotium_plan")
        .unwrap();
    assert_eq!(
        plan["input_schema"]["properties"]["tool_id"]["enum"],
        json!(capabilities::EDIT_OPERATIONS)
    );
    assert_eq!(plan["currently_permitted"], true);
    let mut app = fixture();
    let context = app.session.project_context;
    let mut s = Session::default();
    let revision = app.doc.automation_revision;
    for operation in ["render.preview", "document.inspect"] {
        assert_eq!(s.call(&mut app,"plotium_plan",json!({"expected_project_context":context,"expected_revision":revision,"ids":["document"],"tool_id":operation,"parameters":{}})).unwrap_err().code,"unsupported_operation");
    }
}
