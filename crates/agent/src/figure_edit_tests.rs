use plotium_core::{automation::*, state::*};
use serde_json::{Value, json};
fn args(a: &PlotiumApp) -> Value {
    json!({"expected_project_context":a.session.project_context,"expected_revision":a.doc.automation_revision,"tool_id":FIGURE_EDIT,"parameters":{"canvas":TargetRef::resource(a.doc.canvases[0].resource_id.into()),"edits":[{"op":"page","size_mm":[180,90]},{"op":"text_create","parent":TargetRef::resource(a.doc.canvases[0].resource_id.into()),"name":"Native caption","frame_pt":[10,220,200,20],"text":"R7","font_pt":12}]}})
}
#[test]
fn figure_edit_permissions_discovery_single_use_revision_context_and_busy_are_enforced() {
    let mut a = PlotiumApp::new();
    a.doc
        .canvases
        .push(CanvasDocument::new("R7".into(), [120., 80.]));
    let mut s = crate::Session::default();
    let p = args(&a);
    assert_eq!(
        s.call(&mut a, "plotium_plan", p).unwrap_err().code,
        "permission_denied"
    );
    s.allow_modify = true;
    let state = s.call(&mut a, "plotium_get_state", json!({})).unwrap();
    assert_eq!(
        state["connection_capabilities"]["operation_constraints"][FIGURE_EDIT]["schema"],
        figure_edit_schema()
    );
    let tool = crate::protocol::definitions()
        .into_iter()
        .find(|t| t.name == "plotium_plan")
        .unwrap();
    let alternatives = &tool.input_schema["oneOf"];
    assert!(
        alternatives
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["properties"]["tool_id"]["const"] == FIGURE_EDIT)
    );
    let p = args(&a);
    let planned = s.call(&mut a, "plotium_plan", p).unwrap();
    assert!(a.doc.canvases[0].objects.is_empty());
    s.allow_modify = false;
    let execute = json!({"plan_id":planned["plan_id"]});
    assert_eq!(
        s.call(&mut a, "plotium_execute", execute.clone())
            .unwrap_err()
            .code,
        "permission_denied"
    );
    assert_eq!(
        s.call(&mut a, "plotium_execute", execute).unwrap_err().code,
        "expired_plan"
    );
    s.allow_modify = true;
    let p = args(&a);
    let planned = s.call(&mut a, "plotium_plan", p).unwrap();
    let execute = json!({"plan_id":planned["plan_id"]});
    let result = s.call(&mut a, "plotium_execute", execute.clone()).unwrap();
    assert_eq!(result["produced"].as_array().unwrap().len(), 1);
    assert_eq!(a.doc.canvases[0].size_mm, [180., 90.]);
    assert_eq!(
        s.call(&mut a, "plotium_execute", execute).unwrap_err().code,
        "expired_plan"
    );
    a.undo();
    assert!(a.doc.canvases[0].objects.is_empty());
    a.redo();
    let p = args(&a);
    let planned = s.call(&mut a, "plotium_plan", p).unwrap();
    a.undo();
    assert_eq!(
        s.call(
            &mut a,
            "plotium_execute",
            json!({"plan_id":planned["plan_id"]})
        )
        .unwrap_err()
        .code,
        "stale_revision"
    );
    let p = args(&a);
    let planned = s.call(&mut a, "plotium_plan", p).unwrap();
    assert_eq!(
        s.call_when_available(
            &mut a,
            "plotium_execute",
            json!({"plan_id":planned["plan_id"]}),
            true
        )
        .unwrap_err()
        .code,
        "busy"
    );
    let p = args(&a);
    let planned = s.call(&mut a, "plotium_plan", p).unwrap();
    a.session.project_context = ProjectContextId::default();
    assert_eq!(
        s.call(
            &mut a,
            "plotium_execute",
            json!({"plan_id":planned["plan_id"]})
        )
        .unwrap_err()
        .code,
        "project_changed"
    );
}
