use super::*;
use plotium_core::{automation::*, data::*, state::*};
use serde_json::{Value, json};
use std::time::Duration;
fn fixture() -> PlotiumApp {
    let mut app = PlotiumApp::new();
    let cols = [
        ("x", vec![0., 1., 2.]),
        ("y", vec![1., 2., 4.]),
        ("other", vec![2., 3., 5.]),
        ("error", vec![0.1, 0.2, 0.3]),
    ]
    .into_iter()
    .map(|(name, values)| {
        (
            ColumnSchema::new(name, LogicalType::Float64),
            ColumnChunk::optional_f64(values.into_iter().map(Some)),
        )
    })
    .collect::<Vec<_>>();
    let x = cols[0].0.id;
    let y = cols[1].0.id;
    let table = TableDataset::from_materialized(
        cols,
        vec![],
        Some(x),
        vec![TableSeriesBinding {
            value_column: y,
            uncertainty_column: None,
            fit: None,
        }],
        "plotium.test.r4.agent",
    )
    .unwrap();
    app.doc.datasets.push(Dataset::Table(Box::new(table)));
    let canvas = plotium_core::workflow::build_default_canvas(&app.doc.datasets[0], "R4 agent");
    app.doc.canvases.push(canvas);
    app.session.active_canvas = Some(0);
    app
}
fn args(app: &PlotiumApp, y: usize) -> Value {
    let context = app.session.project_context;
    let t = app.doc.datasets[0].as_table().unwrap();
    let r = &t.typed_state.envelope.revision;
    let c = &r.snapshot.schema.columns;
    let o = &app.doc.canvases[0].objects[0];
    let s = &o.plot().unwrap().binding.series[0];
    json!({"expected_project_context":context,"expected_revision":app.doc.automation_revision,"tool_id":CURVE_BIND,"parameters":{"edits":[{"target":app.series_target(0,o.id,s.id).unwrap(),"source":{"dataset_id":t.resource_id,"field_id":s.source.field,"table_id":r.table_id,"table_revision":r.id,"x_column_id":c[0].id,"y_column_id":c[y].id,"y_error_column_id":c[3].id}}]}})
}
#[test]
fn binding_session_permissions_atomicity_revision_and_one_use_plans() {
    let mut app = fixture();
    let mut s = crate::Session::default();
    let a = args(&app, 2);
    assert_eq!(
        s.call(&mut app, "plotium_plan", a.clone())
            .unwrap_err()
            .code,
        "permission_denied"
    );
    s.allow_modify = true;
    let p = s.call(&mut app, "plotium_plan", a.clone()).unwrap();
    assert!(p["preflight"]["binding_plan"]["bindings"].is_array());
    s.allow_modify = false;
    let execute = json!({"plan_id":p["plan_id"]});
    assert_eq!(
        s.call(&mut app, "plotium_execute", execute.clone())
            .unwrap_err()
            .code,
        "permission_denied"
    );
    assert_eq!(
        s.call(&mut app, "plotium_execute", execute)
            .unwrap_err()
            .code,
        "expired_plan"
    );
    s.allow_modify = true;
    let p = s.call(&mut app, "plotium_plan", a.clone()).unwrap();
    let result = s
        .call(&mut app, "plotium_execute", json!({"plan_id":p["plan_id"]}))
        .unwrap();
    assert_eq!(result["after_revision"], 1);
    assert_eq!(
        s.call(&mut app, "plotium_plan", a).unwrap_err().code,
        "stale_revision"
    );
    let mut a = args(&app, 1);
    a["parameters"]["edits"][0]["source"]["table_revision"] = json!(RevisionId::new());
    assert_eq!(
        s.call(&mut app, "plotium_plan", a).unwrap_err().code,
        "stale_revision"
    );
    let mut a = args(&app, 1);
    a["ids"] = json!([]);
    assert_eq!(
        s.call(&mut app, "plotium_plan", a).unwrap_err().code,
        "invalid_parameters"
    );
    let before = app.doc.canvases[0].objects[0]
        .plot()
        .unwrap()
        .binding
        .clone();
    let mut a = args(&app, 1);
    let mut bad = a["parameters"]["edits"][0].clone();
    bad["target"]["component"]["id"] = json!(999);
    a["parameters"]["edits"].as_array_mut().unwrap().push(bad);
    assert_eq!(
        s.call(&mut app, "plotium_plan", a).unwrap_err().code,
        "invalid_parameters"
    );
    assert_eq!(
        app.doc.canvases[0].objects[0].plot().unwrap().binding,
        before
    );
    app.undo();
    assert!(
        app.doc.canvases[0].objects[0]
            .plot()
            .unwrap()
            .binding
            .coordinates
            .is_none()
    );
    app.redo();
    assert_eq!(
        app.doc.canvases[0].objects[0].plot().unwrap().binding,
        before
    );
}
#[test]
fn binding_schema_uses_exact_embedded_targets_without_changing_old_selectors() {
    let definitions = crate::protocol::definitions();
    let plan = definitions
        .iter()
        .find(|t| t.name == "plotium_plan")
        .unwrap();
    let schema = serde_json::to_value(&plan.input_schema).unwrap();
    assert!(schema.get("oneOf").is_none());
    let omitted = crate::capabilities::NO_SELECTOR;
    for (id, materialize) in [(CURVE_BIND, false), (CURVE_MATERIALIZE, true)] {
        assert!(omitted.contains(&id));
        let served = crate::capabilities::respond(
            serde_json::json!({"section":"operation_constraints","key":id}),
        )
        .unwrap();
        assert_eq!(
            served["contract"]["schema"],
            plotium_core::automation::curve_binding_schema(materialize)
        );
    }
    let constraint = crate::capabilities::respond(
        serde_json::json!({"section":"operation_constraints","key":CURVE_BIND}),
    )
    .unwrap();
    assert_eq!(
        constraint["contract"]["preflight_permission"],
        "reversible_modify"
    );
}
#[test]
fn binding_real_http_mcp_plans_executes_and_returns_native_preview() {
    use rmcp::{
        ServiceExt,
        model::CallToolRequestParams,
        transport::{
            StreamableHttpClientTransport,
            streamable_http_client::StreamableHttpClientTransportConfig,
        },
    };
    let mut app = fixture();
    let context = app.session.project_context;
    let a = args(&app, 2);
    let canvas = TargetRef::resource(app.doc.canvases[0].resource_id.into());
    let mut connection = Connection::start(|| {}).unwrap();
    connection.session.allow_modify = true;
    let mut config = StreamableHttpClientTransportConfig::with_uri(connection.url.clone());
    config.auth_header = Some(connection.token.clone());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let client = tokio::spawn(async move {
            let http = reqwest::Client::builder().no_proxy().build().unwrap();
            let client =
                ().serve(StreamableHttpClientTransport::with_client(http, config))
                    .await
                    .unwrap();
            let call = |name: &'static str, args: Value| {
                CallToolRequestParams::new(name).with_arguments(args.as_object().unwrap().clone())
            };
            let p = client.call_tool(call("plotium_plan", a)).await.unwrap();
            assert_eq!(p.is_error, Some(false));
            let p = p.structured_content.unwrap();
            let result = client
                .call_tool(call("plotium_execute", json!({"plan_id":p["plan_id"]})))
                .await
                .unwrap();
            assert_eq!(result.is_error, Some(false));
            assert_eq!(result.structured_content.unwrap()["after_revision"], 1);
            let image = client
                .call_tool(call(
                    "plotium_render_preview",
                    json!({"expected_project_context":context,"expected_revision":1,"target":canvas,"max_edge":320}),
                ))
                .await
                .unwrap();
            assert_eq!(image.is_error, Some(false));
            assert!(image.content.iter().any(|c| c.as_image().is_some()));
            client.cancel().await.unwrap();
        });
        tokio::time::timeout(Duration::from_secs(20), async {
            while !client.is_finished() {
                if let Some(request) = connection.next_request() {
                    connection
                        .dispatch_request(&mut app, request, false)
                        .unwrap();
                }
                connection.poll_preview(&mut app).unwrap();
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
            client.await.unwrap();
        })
        .await
        .unwrap();
    });
    assert_eq!(
        app.doc.canvases[0].objects[0]
            .plot()
            .unwrap()
            .figure()
            .series[0]
            .points[1],
        [1., 3.]
    );
}

#[test]
fn append_schema_permissions_context_and_frozen_series_identity() {
    let mut app = fixture();
    let mut session = crate::Session::default();
    session.allow_modify = true;
    let a = args(&app, 1);
    let plan = session.call(&mut app, "plotium_plan", a.clone()).unwrap();
    session
        .call(
            &mut app,
            "plotium_execute",
            json!({"plan_id":plan["plan_id"]}),
        )
        .unwrap();
    let a = args(&app, 2);
    let owner = TargetRef::resource(canvas_object_ref(
        app.doc.canvases[0].resource_id,
        app.doc.canvases[0].objects[0].id,
    ));
    let mut append = json!({"expected_project_context":app.session.project_context,"expected_revision":app.doc.automation_revision,"tool_id":CURVE_APPEND,"parameters":{"plot":owner,"series":[{"kind":"line","source":a["parameters"]["edits"][0]["source"]}]}});
    session.allow_modify = false;
    assert_eq!(
        session
            .call(&mut app, "plotium_plan", append.clone())
            .unwrap_err()
            .code,
        "permission_denied"
    );
    session.allow_modify = true;
    let plan = session
        .call(&mut app, "plotium_plan", append.clone())
        .unwrap();
    let proposed = plan["preflight"]["binding_plan"]["appended"][0].clone();
    assert!(!proposed.is_null());
    let result = session
        .call(
            &mut app,
            "plotium_execute",
            json!({"plan_id":plan["plan_id"]}),
        )
        .unwrap();
    assert_eq!(result["value"]["appended"][0], proposed);
    let actual = app
        .series_target(
            0,
            app.doc.canvases[0].objects[0].id,
            app.doc.canvases[0].objects[0]
                .plot()
                .unwrap()
                .binding
                .series[1]
                .id,
        )
        .unwrap();
    assert_eq!(json!(actual), proposed);
    append["expected_revision"] = json!(app.doc.automation_revision);
    let plan = session.call(&mut app, "plotium_plan", append).unwrap();
    app.session.project_context = ProjectContextId::default();
    assert_eq!(
        session
            .call(
                &mut app,
                "plotium_execute",
                json!({"plan_id":plan["plan_id"]})
            )
            .unwrap_err()
            .code,
        "project_changed"
    );
    assert_eq!(
        app.doc.canvases[0].objects[0]
            .plot()
            .unwrap()
            .binding
            .series
            .len(),
        2
    );
}
