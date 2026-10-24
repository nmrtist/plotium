use super::*;

#[test]
fn filtered_full_definitions_are_exact_and_compact_covers_every_property() {
    let full = property_catalog();
    let rows = full["properties"].as_array().unwrap();
    assert_eq!(
        respond(json!({"detail":"full"})).unwrap()["properties"],
        full["properties"]
    );
    let result = respond(json!({})).unwrap();
    let compact = result["properties"].as_array().unwrap();
    assert_eq!(compact.len(), rows.len());
    for (full, compact) in rows.iter().zip(compact) {
        for key in ["id", "label", "scope", "access", "unit", "applicability"] {
            assert_eq!(full[key], compact[key], "{}: {key}", full["id"]);
        }
        for (key, value) in full["schema"].as_object().unwrap() {
            if !matches!(key.as_str(), "variants" | "input_schema" | "drag_step") {
                assert_eq!(value, &compact["schema"][key]);
            }
        }
        if full["schema"].get("input_schema").is_some() {
            assert_eq!(compact["schema"]["requires_full"], true);
        }
        if let Some(variants) = full["schema"]["variants"].as_array() {
            assert_eq!(
                compact["schema"]["variants"],
                json!(variants.iter().map(|v| v["id"].clone()).collect::<Vec<_>>())
            );
        }
        let detail = respond(json!({"keys":[full["id"]],"detail":"full"})).unwrap();
        assert_eq!(detail["properties"], json!([full]));
    }
    let keys = json!([rows[0]["id"], rows[1]["id"]]);
    assert_eq!(
        respond(json!({"keys":keys,"detail":"full"})).unwrap()["properties"],
        json!([rows[0], rows[1]])
    );
    for scope in scopes(rows) {
        let result = respond(json!({"scope":scope})).unwrap();
        assert!(
            result["properties"]
                .as_array()
                .unwrap()
                .iter()
                .all(|r| r["scope"] == scope)
        );
    }
}

#[test]
fn invalid_keys_and_filters_never_silently_drop_requested_properties() {
    let full = property_catalog();
    let first = &full["properties"][0];
    for args in [
        json!({"keys":[]}),
        json!({"keys":null}),
        json!({"scope":null}),
        json!({"keys":[first["id"],first["id"]]}),
        json!({"scope":"missing"}),
        json!({"extra":true}),
        json!({"detail":"other"}),
    ] {
        assert_eq!(respond(args).unwrap_err().code, "invalid_parameters");
    }
    assert_eq!(
        respond(json!({"keys":[first["id"],"missing"]}))
            .unwrap_err()
            .code,
        "unsupported_property"
    );
    let different_scope = scopes(full["properties"].as_array().unwrap())
        .into_iter()
        .find(|s| *s != first["scope"].as_str().unwrap())
        .unwrap();
    assert_eq!(
        respond(json!({"keys":[first["id"]],"scope":different_scope}))
            .unwrap_err()
            .code,
        "invalid_parameters"
    );
}

#[test]
fn session_catalog_is_static_and_available_while_project_work_is_blocked() {
    let mut app = plotium_core::state::PlotiumApp::new_with_settings(Default::default());
    let mut session = crate::Session::default();
    let args = json!({"keys":["series.line.stroke_width"],"detail":"full"});
    let before = (
        app.doc.automation_revision,
        app.doc.dirty,
        app.doc.automation_runs.len(),
    );
    assert_eq!(
        session
            .call_when_available(&mut app, "plotium_properties_list", args.clone(), true)
            .unwrap(),
        respond(args).unwrap()
    );
    assert_eq!(
        before,
        (
            app.doc.automation_revision,
            app.doc.dirty,
            app.doc.automation_runs.len()
        )
    );
}
