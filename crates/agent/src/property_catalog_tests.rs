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
        let id = full["id"].as_str().unwrap();
        for key in ["id", "label"] {
            assert_eq!(full[key], compact[key], "{id}: {key}");
        }
        assert_eq!(compact["type"], full["schema"]["type"], "{id}");
        // Dropped fields must stay derivable from the id.
        assert_eq!(
            full["scope"] == "Document",
            id.starts_with("document."),
            "{id}"
        );
        assert_eq!(
            full["applicability"]["component"] == "series",
            id.starts_with("series."),
            "{id}"
        );
        for key in ["min", "max", "format"] {
            assert_eq!(full["schema"].get(key), compact.get(key), "{id}: {key}");
        }
        assert_eq!(
            compact["requires_full"] == true,
            full["schema"].get("input_schema").is_some() || full["schema"].get("codes").is_some(),
            "{id}"
        );
        if let Some(variants) = full["schema"]["variants"].as_array() {
            assert_eq!(
                compact["values"],
                json!(variants.iter().map(|v| v["id"].clone()).collect::<Vec<_>>())
            );
        }
        let unit = full["unit"].as_str().unwrap();
        assert_eq!(
            compact.get("unit").and_then(Value::as_str),
            (!unit.is_empty()).then_some(unit)
        );
        assert_eq!(
            compact.get("encoding"),
            full["applicability"]
                .get("encoding")
                .filter(|e| !e.is_null())
        );
        assert!(serde_json::to_vec(compact).unwrap().len() <= 200, "{id}");
        let detail = respond(json!({"keys":[full["id"]],"detail":"full"})).unwrap();
        assert_eq!(detail["properties"], json!([full]));
    }
    let keys = json!([rows[0]["id"], rows[1]["id"]]);
    assert_eq!(
        respond(json!({"keys":keys,"detail":"full"})).unwrap()["properties"],
        json!([rows[0], rows[1]])
    );
    assert!(serde_json::to_vec(&result).unwrap().len() <= 8 * 1024);
    let lines = respond(json!({"prefix":"series.line."})).unwrap();
    let lines = lines["properties"].as_array().unwrap();
    assert!(!lines.is_empty());
    assert!(
        lines
            .iter()
            .all(|r| r["id"].as_str().unwrap().starts_with("series.line."))
    );
    assert_eq!(
        lines.len(),
        rows.iter()
            .filter(|r| r["id"].as_str().unwrap().starts_with("series.line."))
            .count()
    );
    for scope in scopes(rows) {
        let result = respond(json!({"scope":scope,"detail":"full"})).unwrap();
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
        json!({"prefix":null}),
        json!({"prefix":""}),
    ] {
        assert_eq!(respond(args).unwrap_err().code, "invalid_parameters");
    }
    assert_eq!(
        respond(json!({"keys":[first["id"],"missing"]}))
            .unwrap_err()
            .code,
        "unsupported_property"
    );
    let error = respond(json!({"prefix":"series.lines"})).unwrap_err();
    assert_eq!(error.code, "invalid_parameters");
    assert!(
        error.details.unwrap()["prefixes"]
            .as_array()
            .unwrap()
            .contains(&json!("series.line"))
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
