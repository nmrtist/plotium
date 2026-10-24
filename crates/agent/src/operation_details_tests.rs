use super::*;
use crate::capabilities::respond;
use plotium_core::automation::*;

#[test]
fn every_figure_branch_has_exact_detail_and_a_parseable_example() {
    let full = respond(json!({"section":"operation_constraints","key":"figure.edit"})).unwrap()["contract"].clone();
    let details = build(&full);
    let branches = full["schema"]["properties"]["edits"]["items"]["oneOf"]
        .as_array()
        .unwrap();
    assert_eq!(details.len(), branches.len());
    for (key, detail) in &details {
        let branch = &detail["schema"]["properties"]["edits"]["items"];
        assert!(branches.contains(branch), "{key}");
        let mut restored = detail["schema"].clone();
        restored["properties"]["edits"]["items"] =
            full["schema"]["properties"]["edits"]["items"].clone();
        restored["description"] = full["schema"]["description"].clone();
        assert_eq!(restored, full["schema"]);
        assert_ne!(
            detail["purpose"],
            json!(key.trim_start_matches("figure.edit/").replace('_', " ")),
            "{key}"
        );
        let shared =
            serde_json::to_vec(detail).unwrap().len() - serde_json::to_vec(branch).unwrap().len();
        assert!(shared <= 5 * 512, "{key}: {shared}");
        assert_eq!(detail["execute_permission"], full["execute_permission"]);
        let _: plotium_core::state::FigureEditParams =
            serde_json::from_value(detail["example"]["parameters"].clone()).unwrap();
        assert_eq!(
            respond(json!({"section":"operation_details","key":key})).unwrap()["contract"],
            *detail
        );
    }
    let keys = json!(["figure.edit/page", "figure.edit/annotation_create.arrow"]);
    let result = respond(json!({"section":"operation_details","keys":keys})).unwrap();
    assert_eq!(result["contract"].as_object().unwrap().len(), 2);
    assert_eq!(
        result["contract"]["figure.edit/page"],
        details["figure.edit/page"]
    );
    let tool = crate::protocol::definitions()
        .into_iter()
        .find(|t| t.name == "plotium_plan")
        .unwrap();
    let detail = respond(json!({"section":"tools","key":"plotium_plan"})).unwrap();
    assert_eq!(detail["contract"]["input_schema"], json!(tool.input_schema));
}

#[test]
fn batch_detail_rejects_invalid_combinations_and_unknown_keys() {
    for args in [
        json!({"keys":["figure.edit/page"]}),
        json!({"section":"operation_details"}),
        json!({"section":"operation_details","keys":null}),
        json!({"section":"operation_details","keys":[]}),
        json!({"section":"operation_details","keys":vec!["figure.edit/page";33]}),
        json!({"section":"operation_details","keys":["figure.edit/page","figure.edit/page"]}),
        json!({"section":"tools","keys":["plotium_plan"]}),
        json!({"section":"operation_details","key":"figure.edit/page","keys":["figure.edit/page"]}),
        json!({"section":"operation_details","keys":["figure.edit/page","missing"]}),
    ] {
        assert_eq!(respond(args).unwrap_err().code, "invalid_parameters");
    }
}

#[test]
fn common_examples_execute_with_discovered_native_targets() {
    let (mut app, mut session, _) = crate::guide_fixture::fixture();
    for key in [
        "figure.edit/page",
        "figure.edit/panel_label",
        "figure.edit/annotation_create.arrow",
    ] {
        let mut params = respond(json!({"section":"operation_details","key":key})).unwrap()["contract"]["example"]["parameters"].clone();
        let canvas = &app.doc.canvases[0];
        params["canvas"] = json!(TargetRef::resource(canvas.resource_id.into()));
        if key.ends_with("panel_label") {
            params["edits"][0]["target"] = json!(TargetRef::resource(panel_resource(
                canvas,
                &canvas.panels[0]
            )));
        } else if key.ends_with(".arrow") {
            params["edits"][0]["target"] = json!(TargetRef::resource(canvas_object_ref(
                canvas.resource_id,
                canvas.objects[0].id
            )));
        }
        let args = json!({"expected_project_context":app.session.project_context,"expected_revision":app.doc.automation_revision,"tool_id":"figure.edit","parameters":params});
        let plan = session.call(&mut app, "plotium_plan", args).unwrap();
        let result = session
            .call(
                &mut app,
                "plotium_execute",
                json!({"plan_id":plan["plan_id"]}),
            )
            .unwrap();
        assert_eq!(result["tool_id"], "figure.edit");
    }
}

#[test]
fn every_shared_clause_routes_to_existing_ops_exactly_once() {
    let full = respond(json!({"section":"operation_constraints","key":"figure.edit"})).unwrap()["contract"].clone();
    let ops: Vec<_> = full["schema"]["properties"]["edits"]["items"]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["properties"]["op"]["const"].as_str().unwrap().to_owned())
        .collect();
    for (text, rules) in [
        (full["schema"]["description"].as_str().unwrap(), DESCRIPTION),
        (full["result_contract"].as_str().unwrap(), RESULT),
    ] {
        let clauses: Vec<_> = clauses(text).collect();
        for clause in &clauses {
            let matches = rules.iter().filter(|(p, _)| clause.starts_with(p)).count();
            assert_eq!(matches, 1, "{clause}");
        }
        for (prefix, routed) in rules {
            assert!(clauses.iter().any(|c| c.starts_with(prefix)), "{prefix}");
            assert!(
                routed.iter().all(|op| ops.iter().any(|o| o == op)),
                "{prefix}"
            );
        }
    }
    let details = build(&full);
    let label = &details["figure.edit/panel_label"];
    let description = label["schema"]["description"].as_str().unwrap();
    assert!(description.starts_with("Atomic native authoring"));
    assert!(!description.contains("panel_stack needs"));
    assert!(
        !label["result_contract"]
            .as_str()
            .unwrap()
            .contains("Deletion-only")
    );
    let delete = details["figure.edit/content_delete"]["schema"]["description"]
        .as_str()
        .unwrap();
    assert!(delete.contains("Deletion-only batches"));
}
