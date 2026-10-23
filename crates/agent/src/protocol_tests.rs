use super::*;

#[test]
fn advertised_inputs_expose_fields_without_top_level_combinations() {
    for (name, fields) in [
        (
            "plotium_plan",
            vec![
                "expected_project_context",
                "expected_revision",
                "tool_id",
                "parameters",
                "ids",
                "targets",
            ],
        ),
        (
            "plotium_properties_inspect",
            vec![
                "expected_project_context",
                "expected_revision",
                "key",
                "ids",
                "targets",
            ],
        ),
        (
            "plotium_document_inspect",
            vec![
                "expected_project_context",
                "expected_revision",
                "target",
                "view",
                "offset",
                "limit",
            ],
        ),
        (
            "plotium_figure_diagnose",
            vec![
                "expected_project_context",
                "expected_revision",
                "canvas",
                "output",
                "cursor",
            ],
        ),
    ] {
        let tool = definitions().into_iter().find(|t| t.name == name).unwrap();
        let schema = &tool.input_schema;
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], false);
        assert!(!schema.contains_key("oneOf"));
        assert!(!schema.contains_key("allOf"));
        for field in fields {
            assert!(schema["properties"].get(field).is_some(), "{name}: {field}");
        }
        for field in ["expected_project_context", "expected_revision"] {
            assert!(
                schema["required"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(field))
            );
        }
    }
}

#[test]
fn plan_selector_policy_is_enforced_without_client_schema_validation() {
    let mut app = plotium_core::state::PlotiumApp::new_with_settings(Default::default());
    let mut session = crate::Session::default();
    session.allow_modify = true;
    for (tool, selectors) in [
        ("resource.rename", json!({})),
        ("resource.rename", json!({"ids":[]})),
        ("resource.rename", json!({"ids":["document"],"targets":[]})),
        (
            "resource.rename",
            json!({"targets":[{"resource":{"id":"document","kind":"plotium.document"},"component":{"kind":"series","id":0}}]}),
        ),
        ("plot.create", json!({"ids":["document"]})),
        ("project.save", json!({"targets":[]})),
        ("properties.set_many", json!({"ids":null})),
        ("table.patch", json!({"ids":["a","b"]})),
        ("data.transform", json!({"ids":[]})),
        ("data.import", json!({"ids":[]})),
    ] {
        let mut args = json!({"expected_project_context":app.session.project_context,
            "expected_revision":app.doc.automation_revision,"tool_id":tool,"parameters":{}});
        args.as_object_mut()
            .unwrap()
            .extend(selectors.as_object().unwrap().clone());
        assert_eq!(
            session
                .call(&mut app, "plotium_plan", args)
                .unwrap_err()
                .code,
            "invalid_parameters",
            "{tool}"
        );
        assert_eq!(app.doc.automation_revision, 0);
    }
}
