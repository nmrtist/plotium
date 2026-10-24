use crate::{capabilities, protocol};
use serde_json::json;

#[test]
fn every_advertised_tool_has_explicit_conservative_annotations() {
    let reads = [
        "plotium_get_state",
        "plotium_capabilities_get",
        "plotium_resources_search",
        "plotium_resources_inspect",
        "plotium_properties_list",
        "plotium_properties_inspect",
        "plotium_document_inspect",
        "plotium_preview_plan",
        "plotium_render_preview",
        "plotium_table_columns",
        "plotium_table_read_rows",
        "plotium_analysis_results_inspect",
        "plotium_diffusion_inspect",
        "plotium_processing_inspect",
        "plotium_tasks_get",
        "plotium_tasks_list",
        "plotium_save_status",
        "plotium_export_status",
    ];
    let definitions = protocol::definitions();
    assert_eq!(definitions.len(), reads.len() + 5);
    for tool in definitions {
        let (read, destructive, idempotent, external) = match tool.name.as_ref() {
            "plotium_plan" => (false, false, false, true),
            "plotium_execute" | "plotium_apply" => (false, true, false, true),
            "plotium_tasks_cancel" => (false, true, true, false),
            "plotium_figure_diagnose" => (true, false, false, false),
            name if reads.contains(&name) => (true, false, true, false),
            name => panic!("unreviewed tool: {name}"),
        };
        assert_eq!(
            json!(tool)["annotations"],
            json!({
                "readOnlyHint":read, "destructiveHint":destructive,
                "idempotentHint":idempotent, "openWorldHint":external,
            })
        );
    }
}

#[test]
fn discovery_preserves_annotations_independently_of_current_permission() {
    let mut first = None;
    for allow_modify in [false, true] {
        let mut app = plotium_core::state::PlotiumApp::new_with_settings(Default::default());
        let mut session = crate::Session::default();
        session.set_allow_modify(allow_modify);
        let discovery = session
            .call(&mut app, "plotium_capabilities_get", json!({}))
            .unwrap();
        assert_eq!(first.get_or_insert_with(|| discovery.clone()), &discovery);
        let manifest = &discovery["connection_capabilities"];
        for tool in protocol::definitions() {
            assert_eq!(
                tool.annotations,
                Some(crate::tool_effects::annotations(&tool.name))
            );
            let advertised = manifest["tools"]
                .as_array()
                .unwrap()
                .iter()
                .find(|entry| entry["name"] == tool.name.as_ref())
                .unwrap();
            // Annotations and schemas live in tools/list; the index never repeats them.
            assert!(advertised.get("annotations").is_none());
            assert!(advertised.get("input_schema").is_none());
            if let Some(operation) = match tool.name.as_ref() {
                "plotium_tasks_get" => Some("tasks.get"),
                "plotium_tasks_list" => Some("tasks.list"),
                "plotium_tasks_cancel" => Some("tasks.cancel"),
                _ => None,
            } {
                assert_eq!(advertised["operations"], json!([operation]));
            }
        }
        let grouped = manifest["editable_operations"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|group| group["operations"].as_array().unwrap().clone())
            .collect::<Vec<_>>();
        assert_eq!(grouped.len(), capabilities::EDIT_OPERATIONS.len());
        for id in capabilities::EDIT_OPERATIONS {
            assert!(grouped.contains(&json!(id)));
        }
    }
}

#[test]
fn repeated_save_preflight_retains_distinct_plans_and_submission_still_needs_consent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("consent.plotium");
    let mut app = plotium_core::state::PlotiumApp::new_with_settings(Default::default());
    app.start_new_project();
    let mut session = crate::Session::default();
    let revision = app.doc.automation_revision;
    let runs = app.doc.automation_runs.len();
    let args = json!({"expected_project_context":app.session.project_context,
        "expected_revision":revision,"tool_id":"project.save","parameters":{"path":path}});
    let first = session
        .call(&mut app, "plotium_plan", args.clone())
        .unwrap();
    let second = session.call(&mut app, "plotium_plan", args).unwrap();
    assert_ne!(first["plan_id"], second["plan_id"]);
    let result = session
        .call(
            &mut app,
            "plotium_execute",
            json!({"plan_id":first["plan_id"]}),
        )
        .unwrap();
    assert_eq!(result["save"]["status"], "awaiting_authorization");
    assert!(!path.exists());
    app.decide_external_save(first["save_id"].as_str().unwrap(), false, false);
    let status = session
        .call(
            &mut app,
            "plotium_save_status",
            json!({"save_id":first["save_id"]}),
        )
        .unwrap();
    assert_eq!(status["save"]["phase"], "not_installed");
    assert!(!path.exists());
    assert_eq!(app.doc.automation_revision, revision);
    assert_eq!(app.doc.automation_runs.len(), runs);
    assert!(!session.allow_modify);
    assert_eq!(
        session
            .call(
                &mut app,
                "plotium_execute",
                json!({"plan_id":first["plan_id"]})
            )
            .unwrap_err()
            .code,
        "expired_plan"
    );
}
