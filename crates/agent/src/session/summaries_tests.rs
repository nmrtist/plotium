use super::*;
use crate::Session;
use plotium_core::state::PlotiumApp;

fn fixture() -> (PlotiumApp, Session, Vec<String>) {
    crate::guide_fixture::fixture()
}

fn context(app: &PlotiumApp) -> Value {
    json!({"expected_project_context":app.session.project_context,"expected_revision":app.doc.automation_revision})
}

#[test]
fn figure_edits_return_summaries_without_echoes_or_static_prose() {
    let (mut app, mut session, _) = fixture();
    session.allow_modify = true;
    let canvas = &app.doc.canvases[0];
    let panel = TargetRef::resource(panel_resource(canvas, &canvas.panels[0]));
    let mut args = context(&app);
    args["tool_id"] = json!(FIGURE_EDIT);
    args["parameters"] = json!({"canvas":TargetRef::resource(canvas.resource_id.into()),
        "edits":[{"op":"panel_label","target":panel,"text":"a","visible":true}]});
    let plan = session.call(&mut app, "plotium_plan", args).unwrap();
    let stored = session.plans[plan["plan_id"].as_str().unwrap()].clone();
    let preflight = &plan["preflight"];
    assert!(preflight.get("request").is_none());
    assert!(preflight.get("frozen_targets").is_none());
    assert!(preflight["targets"].as_array().unwrap().is_empty());
    assert_eq!(preflight["summary"]["compatible"], stored.targets.len());
    assert_eq!(
        plan["basis_revision"],
        json!(stored.frozen_targets.revision)
    );
    assert_eq!(plan["project_context"], json!(app.session.project_context));
    assert!(
        preflight["figure_plan"]["result"]
            .get("frame_contract")
            .is_none()
    );
    assert_eq!(
        preflight["figure_plan"]["modified"],
        json!(stored.figure_plan.as_ref().unwrap().modified)
    );
    let result = session
        .call(
            &mut app,
            "plotium_execute",
            json!({"plan_id":plan["plan_id"]}),
        )
        .unwrap();
    assert_eq!(result["summary"]["succeeded"], stored.targets.len());
    assert!(result["targets"].as_array().unwrap().is_empty());
    assert!(result["value"].get("frame_contract").is_none());
    assert!(!result["modified"].as_array().unwrap().is_empty());
    assert_eq!(
        result["project_context"],
        json!(app.session.project_context)
    );
}

#[test]
fn rejected_targets_stay_in_plans_and_results() {
    let (mut app, mut session, ids) = fixture();
    session.allow_modify = true;
    let canvas = app.doc.canvases[0].resource_id.to_string();
    let mut args = context(&app);
    args["tool_id"] = json!("figure.apply_theme");
    args["ids"] = json!([canvas, ids[0]]);
    args["parameters"] = json!({"theme_id":"nature"});
    let plan = session.call(&mut app, "plotium_plan", args).unwrap();
    let rows = plan["preflight"]["targets"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(plan["preflight"]["summary"]["compatible"], 1);
    assert_eq!(
        plan["preflight"]["summary"]["skipped"].as_u64().unwrap()
            + plan["preflight"]["summary"]["failed"].as_u64().unwrap(),
        1
    );
    let result = session
        .call(
            &mut app,
            "plotium_execute",
            json!({"plan_id":plan["plan_id"]}),
        )
        .unwrap();
    assert_eq!(result["targets"].as_array().unwrap().len(), 1);
    assert_eq!(result["summary"]["succeeded"], 1);
}

#[test]
fn property_reads_share_schema_and_searches_state_a_common_reason_once() {
    let (mut app, mut session, ids) = fixture();
    let mut args = context(&app);
    args["ids"] = json!(ids);
    args["key"] = json!("object.axes.x_label");
    let result = session
        .call(&mut app, "plotium_properties_inspect", args)
        .unwrap();
    let readings = result["value"]["readings"].as_array().unwrap();
    assert!(readings.len() > 1);
    assert!(result["value"]["schema"].is_object());
    assert!(
        readings.iter().all(|r| r.get("schema").is_none()
            && r["target"].is_object()
            && r.get("value").is_some())
    );
    assert!(result["targets"].as_array().unwrap().is_empty());
    let mut args = context(&app);
    args["query"] = json!({});
    let found = session
        .call(&mut app, "plotium_resources_search", args)
        .unwrap();
    assert!(found["value"].get("reasons").is_none());
    assert_eq!(found["value"]["reason"], "matched all resources");
    assert!(!found["value"]["targets"].as_array().unwrap().is_empty());
}

#[test]
fn compact_diagnostic_pages_drop_static_detail_and_full_pages_keep_it() {
    let mut page = json!({"items":[],"summary":{"issues":0},"budget":{},"preview_request":{},
        "read_policy":{},"output":{"size_mm":[89,60],"size_pt":[252,170],"origin":"top_left"},
        "coverage":{"status":"complete","scan_complete":true,"requested_checks":["bounds"],
        "supported_checks":["bounds"],"unsupported_checks":["x"],"unavailable_items":0}});
    diagnostic_page(&mut page);
    assert_eq!(
        page,
        json!({"items":[],"summary":{"issues":0},"output":{"size_mm":[89,60],"size_pt":[252,170]},
        "coverage":{"status":"complete","scan_complete":true,"unavailable_items":0}})
    );
}
