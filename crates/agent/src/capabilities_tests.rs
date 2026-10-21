use super::*;
use crate::{Connection, PendingRequest};
use plotium_core::state::{BoardFrameId, CanvasDocument, PlotiumApp};

fn dispatch(
    connection: &mut Connection,
    app: &mut PlotiumApp,
    name: &str,
    arguments: Value,
    blocked: bool,
) -> Result<Value, crate::Failure> {
    let (response, mut receive) = tokio::sync::oneshot::channel();
    let status = connection.dispatch_request(
        app,
        PendingRequest {
            name: name.into(),
            arguments,
            response,
            cancellation: tokio_util::sync::CancellationToken::new(),
            transport: None,
        },
        blocked,
    );
    let delivered = receive.try_recv().unwrap();
    assert_eq!(
        status.as_ref().err().map(|e| e.value()),
        delivered.as_ref().err().map(|e| e.value())
    );
    delivered
}

#[test]
fn fingerprint_covers_nested_contracts_and_ignores_object_order() {
    let contract = manifest();
    assert_eq!(fingerprint(), fingerprint_of(contract));
    assert_eq!(fingerprint().len(), 71);
    let mut reversed = serde_json::Map::new();
    for (key, value) in contract.as_object().unwrap().iter().rev() {
        reversed.insert(key.clone(), value.clone());
    }
    assert_eq!(fingerprint(), fingerprint_of(&Value::Object(reversed)));
    for path in [
        "/tools/0/input_schema/properties/selection_limit/maximum",
        "/tools/0/annotations/readOnlyHint",
        "/operation_constraints/data.import/schema",
        "/property_definitions/properties/0/schema/min",
        "/limits/document_page_max",
        "/recovery_contract/unknown",
    ] {
        let mut changed = contract.clone();
        *changed.pointer_mut(path).unwrap() = json!("changed");
        assert_ne!(fingerprint(), fingerprint_of(&changed), "{path}");
    }
    assert_ne!(
        fingerprint_of(&json!([1, 2])),
        fingerprint_of(&json!([2, 1]))
    );
    let encoded = serde_json::to_string(contract).unwrap();
    assert!(!encoded.contains("currently_permitted"));
    assert!(!encoded.contains("allow_reversible_modify"));
}

#[test]
fn desktop_state_is_bounded_and_selection_remains_discoverable() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    for i in 0..123 {
        let canvas = CanvasDocument::new(format!("Canvas {i}"), [100., 80.]);
        app.session
            .ui
            .frame_selection
            .push(BoardFrameId::Page(canvas.resource_id));
        app.doc.canvases.push(canvas);
    }
    let before = (
        app.doc.dirty,
        app.doc.edit_generation,
        app.doc.automation_revision,
        app.doc.automation_runs.len(),
    );
    let mut connection = Connection::start(|| {}).unwrap();
    let state = dispatch(
        &mut connection,
        &mut app,
        "plotium_get_state",
        json!({}),
        false,
    )
    .unwrap();
    assert!(state.get("connection_capabilities").is_none());
    assert_eq!(
        state["project"],
        json!({"dataset_count":0,"canvas_count":123})
    );
    assert_eq!(state["selection"]["items"].as_array().unwrap().len(), 16);
    assert_eq!(state["selection"]["total"], 123);
    assert_eq!(state["selection"]["next_offset"], 16);
    assert!(serde_json::to_vec(&state).unwrap().len() < 8192);
    let mut ids = Vec::new();
    let mut offset = 0;
    loop {
        let page = dispatch(
            &mut connection,
            &mut app,
            "plotium_get_state",
            json!({"selection_offset":offset,"selection_limit":50}),
            false,
        )
        .unwrap();
        ids.extend(
            page["selection"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["id"].clone()),
        );
        match page["selection"]["next_offset"].as_u64() {
            Some(next) => offset = next,
            None => break,
        }
    }
    assert_eq!(
        ids,
        app.doc
            .canvases
            .iter()
            .map(|c| json!(c.resource_id))
            .collect::<Vec<_>>()
    );
    let discovery = dispatch(
        &mut connection,
        &mut app,
        "plotium_capabilities_get",
        json!({}),
        false,
    )
    .unwrap();
    assert_eq!(
        discovery["capability_fingerprint"],
        state["capability_fingerprint"]
    );
    let root = discovery["connection_capabilities"]["document_root"].clone();
    let children = dispatch(&mut connection, &mut app, "plotium_document_inspect",
        json!({"expected_project_context":state["project_context"],"expected_revision":state["revision"],"target":root,"view":"children","limit":50}), false).unwrap();
    assert_eq!(children["value"]["total"], 123);
    assert_eq!(
        children["value"]["items"][0]["target"]["resource"]["id"],
        ids[0]
    );
    app.session.ui.frame_selection.truncate(1);
    let latest = dispatch(
        &mut connection,
        &mut app,
        "plotium_get_state",
        json!({"selection_offset":50}),
        false,
    )
    .unwrap();
    assert_eq!(latest["selection"]["total"], 1);
    assert_eq!(latest["selection"]["items"], json!([]));
    assert!(latest["selection"]["next_offset"].is_null());
    assert_eq!(
        before,
        (
            app.doc.dirty,
            app.doc.edit_generation,
            app.doc.automation_revision,
            app.doc.automation_runs.len()
        )
    );
}

#[test]
fn cached_contract_never_caches_permission_or_project_authority() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let mut connection = Connection::start(|| {}).unwrap();
    let original = dispatch(
        &mut connection,
        &mut app,
        "plotium_capabilities_get",
        json!({}),
        false,
    )
    .unwrap();
    let context = app.session.project_context;
    connection.session.set_allow_modify(true);
    let plan = dispatch(&mut connection, &mut app, "plotium_plan", json!({
        "expected_project_context":context,"expected_revision":0,"ids":["document"],
        "tool_id":"properties.set","parameters":{"key":"document.figure.typography.tick_pt","value":15}
    }), false).unwrap();
    connection.session.set_allow_modify(false);
    let state = dispatch(
        &mut connection,
        &mut app,
        "plotium_get_state",
        json!({}),
        false,
    )
    .unwrap();
    assert_eq!(state["permissions"]["allow_reversible_modify"], false);
    assert_eq!(
        state["availability"]["project_modify"]["reason"]["code"],
        "permission_denied"
    );
    let error = dispatch(
        &mut connection,
        &mut app,
        "plotium_execute",
        json!({"plan_id":plan["plan_id"]}),
        false,
    )
    .unwrap_err()
    .value();
    assert_eq!(error["error"]["code"], "permission_denied");
    assert_eq!(error["error"]["recovery"]["plan_consumed"], true);
    assert_eq!(error["error"]["recovery"]["committed"], false);
    for blocked in [false, true] {
        let cached = dispatch(
            &mut connection,
            &mut app,
            "plotium_capabilities_get",
            json!({}),
            blocked,
        )
        .unwrap();
        assert_eq!(cached, original);
        let busy = dispatch(
            &mut connection,
            &mut app,
            "plotium_get_state",
            json!({}),
            blocked,
        )
        .unwrap();
        assert_eq!(
            busy["capability_fingerprint"],
            original["capability_fingerprint"]
        );
        if blocked {
            assert!(busy["project"].is_null());
            assert!(busy["selection"].is_null());
            assert_eq!(
                busy["availability"]["project_read"]["reason"]["code"],
                "busy"
            );
        }
    }
    connection.session.set_allow_modify(true);
    let plan = dispatch(&mut connection, &mut app, "plotium_plan", json!({
        "expected_project_context":context,"expected_revision":0,"ids":["document"],
        "tool_id":"properties.set","parameters":{"key":"document.figure.typography.tick_pt","value":15}
    }), false).unwrap();
    dispatch(
        &mut connection,
        &mut app,
        "plotium_execute",
        json!({"plan_id":plan["plan_id"]}),
        false,
    )
    .unwrap();
    let stale = dispatch(
        &mut connection,
        &mut app,
        "plotium_resources_search",
        json!({
            "expected_project_context":context,"expected_revision":0,"query":{}
        }),
        false,
    )
    .unwrap_err();
    assert_eq!(stale.code, "stale_revision");
    app.close_project();
    let changed = dispatch(
        &mut connection,
        &mut app,
        "plotium_resources_search",
        json!({
            "expected_project_context":context,"expected_revision":0,"query":{}
        }),
        false,
    )
    .unwrap_err();
    assert_eq!(changed.code, "project_changed");
    let fresh = dispatch(
        &mut connection,
        &mut app,
        "plotium_get_state",
        json!({}),
        false,
    )
    .unwrap();
    assert_ne!(fresh["project_context"], json!(context));
    assert_eq!(
        fresh["capability_fingerprint"],
        original["capability_fingerprint"]
    );
    assert_eq!(
        dispatch(
            &mut connection,
            &mut app,
            "plotium_capabilities_get",
            json!({}),
            false
        )
        .unwrap(),
        original
    );
}

#[test]
fn discovery_and_state_reject_invalid_arguments_even_while_busy() {
    let mut connection = Connection::start(|| {}).unwrap();
    let mut app = PlotiumApp::new_with_settings(Default::default());
    for (name, args) in [
        ("plotium_capabilities_get", json!({"allow_modify":true})),
        ("plotium_capabilities_get", json!({"expected_revision":0})),
        ("plotium_get_state", json!({"selection_limit":0})),
        ("plotium_get_state", json!({"selection_limit":51})),
        ("plotium_get_state", json!({"selection_limit":null})),
        ("plotium_get_state", json!({"selection_offset":-1})),
        ("plotium_get_state", json!({"selection_limit":"50"})),
        ("plotium_get_state", json!({"include_capabilities":true})),
    ] {
        assert_eq!(
            dispatch(&mut connection, &mut app, name, args, true)
                .unwrap_err()
                .code,
            "invalid_parameters"
        );
    }
}

#[test]
fn state_keeps_bounded_save_ids_and_detailed_recovery_stays_queryable() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let mut connection = Connection::start(|| {}).unwrap();
    let context = app.session.project_context;
    let mut ids = Vec::new();
    for i in 0..20 {
        let plan = dispatch(
            &mut connection,
            &mut app,
            "plotium_plan",
            json!({
                "expected_project_context":context,"expected_revision":0,"tool_id":"project.save",
                "parameters":{"path":directory.path().join(format!("save{i}.plotium"))}
            }),
            false,
        )
        .unwrap();
        dispatch(
            &mut connection,
            &mut app,
            "plotium_execute",
            json!({"plan_id":plan["plan_id"]}),
            false,
        )
        .unwrap();
        ids.push(plan["save_id"].clone());
        app.decide_external_save(ids.last().unwrap().as_str().unwrap(), false, false);
    }
    let state = dispatch(
        &mut connection,
        &mut app,
        "plotium_get_state",
        json!({}),
        false,
    )
    .unwrap();
    assert!(state["save_state"].get("receipts").is_none());
    assert_eq!(state["save_state"]["receipt_ids"], json!(&ids[4..]));
    let status = dispatch(
        &mut connection,
        &mut app,
        "plotium_save_status",
        json!({"save_id":ids[19]}),
        false,
    )
    .unwrap();
    assert_eq!(status["save"]["recovery"]["completion"], "failed");
    assert_eq!(status["save"]["recovery"]["committed"], false);
    let missing = dispatch(
        &mut connection,
        &mut app,
        "plotium_save_status",
        json!({"save_id":ids[0]}),
        false,
    )
    .unwrap_err()
    .value();
    assert_eq!(missing["error"]["recovery"]["submission"], "unknown");
    assert_eq!(missing["error"]["recovery"]["replay_allowed"], false);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}
