use super::*;
use plotium_core::{automation::*, data::*, state::*};
use serde_json::{Value, json};
use std::time::Duration;

fn fixture() -> PlotiumApp {
    let mut app = PlotiumApp::new();
    let cols = [
        ("x", vec![0., 1., 2.]),
        ("y", vec![1., 2., 4.]),
        ("error", vec![0.1, 0.2, 0.3]),
        ("bad", vec![0., -1., 0.]),
    ]
    .into_iter()
    .map(|(n, v)| {
        (
            ColumnSchema::new(n, LogicalType::Float64),
            ColumnChunk::optional_f64(v.into_iter().map(Some)),
        )
    })
    .collect::<Vec<_>>();
    let t = TableDataset::from_materialized(cols, vec![], None, vec![], "plotium.test.r5.agent")
        .unwrap();
    app.doc.datasets.push(Dataset::Table(Box::new(t)));
    app
}
fn args(app: &PlotiumApp) -> Value {
    let context = app.session.project_context;
    let t = app.doc.datasets[0].as_table().unwrap();
    let r = &t.typed_state.envelope.revision;
    let c = &r.snapshot.schema.columns;
    json!({"expected_project_context":context,"expected_revision":app.doc.automation_revision,"tool_id":PLOT_CREATE,"parameters":{"parent":document_root_target(),"name":"Native single plot","series":[{"kind":"line","source":{"dataset_id":t.resource_id,"field_id":app.doc.datasets[0].default_field_id().unwrap(),"table_id":r.table_id,"table_revision":r.id,"x_column_id":c[0].id,"y_column_id":c[1].id,"y_error_column_id":c[2].id}}]}})
}
#[test]
fn creation_permissions_one_use_revision_and_invalid_batch_are_precise() {
    let mut app = fixture();
    let mut s = crate::Session::default();
    let a = args(&app);
    assert_eq!(
        s.call(&mut app, "plotium_plan", a.clone())
            .unwrap_err()
            .code,
        "permission_denied"
    );
    s.allow_modify = true;
    let p = s.call(&mut app, "plotium_plan", a.clone()).unwrap();
    assert!(app.doc.canvases.is_empty());
    s.allow_modify = false;
    let e = json!({"plan_id":p["plan_id"]});
    assert_eq!(
        s.call(&mut app, "plotium_execute", e.clone())
            .unwrap_err()
            .code,
        "permission_denied"
    );
    assert_eq!(
        s.call(&mut app, "plotium_execute", e).unwrap_err().code,
        "expired_plan"
    );
    s.allow_modify = true;
    let mut bad = a.clone();
    let mut curve = bad["parameters"]["series"][0].clone();
    curve["source"]["y_error_column_id"] = json!(
        app.doc.datasets[0]
            .as_table()
            .unwrap()
            .typed_state
            .envelope
            .revision
            .snapshot
            .schema
            .columns[3]
            .id
    );
    bad["parameters"]["series"]
        .as_array_mut()
        .unwrap()
        .push(curve);
    assert_eq!(
        s.call(&mut app, "plotium_plan", bad).unwrap_err().code,
        "invalid_parameters"
    );
    assert!(app.doc.canvases.is_empty());
    assert!(!app.can_undo());
    let mut stale = a.clone();
    stale["parameters"]["series"][0]["source"]["table_revision"] = json!(RevisionId::new());
    assert_eq!(
        s.call(&mut app, "plotium_plan", stale).unwrap_err().code,
        "stale_revision"
    );
    let mut duplicate = a.clone();
    duplicate["ids"] = json!([]);
    assert_eq!(
        s.call(&mut app, "plotium_plan", duplicate)
            .unwrap_err()
            .code,
        "invalid_parameters"
    );
    let p = s.call(&mut app, "plotium_plan", a.clone()).unwrap();
    let pre = p["preflight"]["binding_plan"].clone();
    let e = json!({"plan_id":p["plan_id"]});
    let r = s.call(&mut app, "plotium_execute", e.clone()).unwrap();
    assert_eq!(r["value"], pre);
    assert_eq!(r["after_revision"], 1);
    assert_eq!(
        s.call(&mut app, "plotium_execute", e).unwrap_err().code,
        "expired_plan"
    );
    assert_eq!(app.doc.canvases.len(), 1);
    assert_eq!(
        s.call(&mut app, "plotium_plan", a).unwrap_err().code,
        "stale_revision"
    );
    app.undo();
    app.redo();
    assert_eq!(
        app.doc.canvases[0].resource_id.to_string(),
        pre["creation"]["canvas"]["resource"]["id"]
    );
}
#[test]
fn creation_execute_loses_race_to_other_plan_and_busy_consumes_plan() {
    let mut app = fixture();
    let mut s = crate::Session::default();
    s.allow_modify = true;
    let a = args(&app);
    let p = s.call(&mut app, "plotium_plan", a.clone()).unwrap();
    let p2 = s.call(&mut app, "plotium_plan", a.clone()).unwrap();
    s.call(&mut app, "plotium_execute", json!({"plan_id":p["plan_id"]}))
        .unwrap();
    assert_eq!(
        s.call(
            &mut app,
            "plotium_execute",
            json!({"plan_id":p2["plan_id"]})
        )
        .unwrap_err()
        .code,
        "stale_revision"
    );
    assert_eq!(app.doc.canvases.len(), 1);
    let a = args(&app);
    let p = s.call(&mut app, "plotium_plan", a).unwrap();
    let e = json!({"plan_id":p["plan_id"]});
    assert_eq!(
        s.call_when_available(&mut app, "plotium_execute", e.clone(), true)
            .unwrap_err()
            .code,
        "busy"
    );
    assert_eq!(
        s.call(&mut app, "plotium_execute", e).unwrap_err().code,
        "expired_plan"
    );
    assert_eq!(app.doc.canvases.len(), 1);
}
#[test]
fn creation_discovery_schema_and_pending_curve_budget_are_shared_with_bindings() {
    let mut app = fixture();
    let mut s = crate::Session::default();
    s.allow_modify = true;
    let state = s.call(&mut app, "plotium_get_state", json!({})).unwrap();
    let cap = &state["connection_capabilities"];
    assert!(
        cap["editable_operations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == PLOT_CREATE)
    );
    assert_eq!(
        cap["operation_constraints"][PLOT_CREATE]["schema"],
        plot_create_schema()
    );
    assert_eq!(
        cap["operation_constraints"][PLOT_CREATE]["preflight_permission"],
        "reversible_modify"
    );
    let defs = crate::protocol::definitions();
    let schema = serde_json::to_value(
        &defs
            .iter()
            .find(|t| t.name == "plotium_plan")
            .unwrap()
            .input_schema,
    )
    .unwrap();
    let entry = schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["properties"]["tool_id"]["const"] == PLOT_CREATE)
        .unwrap();
    assert_eq!(entry["properties"]["parameters"], plot_create_schema());
    let columns = ["x", "y", "error"]
        .into_iter()
        .enumerate()
        .map(|(i, n)| {
            (
                ColumnSchema::new(n, LogicalType::Float64),
                ColumnChunk::optional_f64(
                    (0..100_001).map(|row| Some(if i == 2 { 0.1 } else { row as f64 })),
                ),
            )
        })
        .collect();
    app.doc.datasets[0] = Dataset::Table(Box::new(
        TableDataset::from_materialized(columns, vec![], None, vec![], "plotium.test.r5.budget")
            .unwrap(),
    ));
    let a = args(&app);
    for _ in 0..19 {
        s.call(&mut app, "plotium_plan", a.clone()).unwrap();
    }
    assert_eq!(
        s.call(&mut app, "plotium_plan", a).unwrap_err().code,
        "plan_limit"
    );
}
#[test]
fn creation_real_http_discovery_execute_png_and_precise_property_edit() {
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
    let a = args(&app);
    let mut connection = Connection::start(|| {}).unwrap();
    connection.session.allow_modify = true;
    let mut config = StreamableHttpClientTransportConfig::with_uri(connection.url.clone());
    config.auth_header = Some(connection.token.clone());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let task=tokio::spawn(async move {
            let client=().serve(StreamableHttpClientTransport::with_client(reqwest::Client::builder().no_proxy().build().unwrap(),config)).await.unwrap();
            let call=|name:&'static str,v:Value|CallToolRequestParams::new(name).with_arguments(v.as_object().unwrap().clone());
            let p=client.call_tool(call("plotium_plan",a)).await.unwrap().structured_content.unwrap();
            let result=client.call_tool(call("plotium_execute",json!({"plan_id":p["plan_id"]}))).await.unwrap();assert_eq!(result.is_error,Some(false));let r=result.structured_content.unwrap();
            let canvas=r["value"]["creation"]["canvas"].clone();let curve=r["value"]["bindings"][0]["target"].clone();
            let detail=client.call_tool(call("plotium_document_inspect",json!({"expected_project_context":context,"expected_revision":1,"target":curve,"view":"self"}))).await.unwrap();assert_eq!(detail.is_error,Some(false));
            for rev in [1,2] {
                let image=client.call_tool(call("plotium_render_preview",json!({"expected_project_context":context,"expected_revision":rev,"target":canvas,"max_edge":320}))).await.unwrap();
                assert_eq!(image.is_error,Some(false));assert!(image.content.iter().any(|c|c.as_image().is_some()));assert_eq!(image.structured_content.unwrap()["value"]["render_revision"],rev);
                if rev==1 {
                    let p=client.call_tool(call("plotium_plan",json!({"expected_project_context":context,"expected_revision":1,"tool_id":"properties.set","targets":[curve],"parameters":{"key":"series.line.stroke_width","value":1.5}}))).await.unwrap().structured_content.unwrap();
                    let r=client.call_tool(call("plotium_execute",json!({"plan_id":p["plan_id"]}))).await.unwrap();assert_eq!(r.is_error,Some(false));
                }
            }
            let stale=client.call_tool(call("plotium_render_preview",json!({"expected_project_context":context,"expected_revision":1,"target":canvas}))).await.unwrap();assert_eq!(stale.is_error,Some(true));assert!(!stale.content.iter().any(|c|c.as_image().is_some()));
            client.cancel().await.unwrap();
        });
        tokio::time::timeout(Duration::from_secs(30),async {
            while !task.is_finished() {
                if let Some(req)=connection.next_request() && let Err(error)=connection.dispatch_request(&mut app,req,false) { assert_eq!(error.code,"stale_revision"); }
                connection.poll_preview(&mut app).unwrap();tokio::time::sleep(Duration::from_millis(2)).await;
            }task.await.unwrap();
        }).await.unwrap();
    });
    assert_eq!(
        app.doc.canvases[0].objects[0]
            .plot()
            .unwrap()
            .figure()
            .series[0]
            .width,
        1.5
    );
    let plot = app.doc.canvases[0].objects[0].plot().unwrap();
    assert!(
        plot.figure().y.max > 4.3,
        "Auto Y must include the upper uncertainty after a line-width edit"
    );
    assert!(plot.figure().y.min < 0.9);
}

#[test]
fn line_width_changes_preserve_native_sources_and_automatic_axis_ranges() {
    let mut app = fixture();
    let context = app.session.project_context;
    let mut s = crate::Session::default();
    s.allow_modify = true;
    let a = args(&app);
    let p = s.call(&mut app, "plotium_plan", a).unwrap();
    let r = s
        .call(&mut app, "plotium_execute", json!({"plan_id":p["plan_id"]}))
        .unwrap();
    let curve = r["value"]["bindings"][0]["target"].clone();
    let before = app.doc.canvases[0].objects[0].plot().unwrap();
    let binding = before.binding.clone();
    let x = before.viewport.view_x;
    let y = before.viewport.view_y;
    let p=s.call(&mut app,"plotium_plan",json!({"expected_project_context":context,"expected_revision":1,"tool_id":"properties.set","targets":[curve],"parameters":{"key":"series.line.stroke_width","value":1.5}})).unwrap();
    s.call(&mut app, "plotium_execute", json!({"plan_id":p["plan_id"]}))
        .unwrap();
    let after = app.doc.canvases[0].objects[0].plot().unwrap();
    assert_eq!(after.viewport.view_x, x);
    assert_eq!(after.viewport.view_y, y);
    for (a, b) in binding.series.iter().zip(&after.binding.series) {
        assert_eq!(a.id, b.id);
        assert_eq!(a.source, b.source);
    }
    assert_eq!(after.binding.coordinates, binding.coordinates);
    assert_eq!(after.figure().series[0].width, 1.5);
}

#[test]
fn returned_creation_targets_can_be_styled_together_with_one_undo() {
    let mut app = fixture();
    let mut session = crate::Session::default();
    session.set_allow_modify(true);
    let arguments = args(&app);
    let plan = session.call(&mut app, "plotium_plan", arguments).unwrap();
    let created = session
        .call(
            &mut app,
            "plotium_execute",
            json!({"plan_id":plan["plan_id"]}),
        )
        .unwrap();
    let series = &created["value"]["bindings"][0]["target"];
    let plot = &created["value"]["creation"]["plot"];
    let before =
        serde_json::to_value(app.doc.canvases[0].objects[0].plot().unwrap().figure()).unwrap();
    let undo = app.session.undo_stack.len();
    let arguments = json!({"expected_project_context":app.session.project_context,
    "expected_revision":created["after_revision"],"tool_id":"properties.set_many",
    "parameters":{"writes":[
        {"target":series,"key":"series.label","value":"Sample 1"},
        {"target":series,"key":"series.line.color","value":"#0072B2"},
        {"target":plot,"key":"object.axes.x_label","value":"Temperature"}
    ]}});
    let plan = session.call(&mut app, "plotium_plan", arguments).unwrap();
    let result = session
        .call(
            &mut app,
            "plotium_execute",
            json!({"plan_id":plan["plan_id"]}),
        )
        .unwrap();
    assert!(
        result["value"]["writes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|w| w["outcome"] == "succeeded")
    );
    assert_eq!(app.session.undo_stack.len(), undo + 1);
    app.undo();
    assert_eq!(
        serde_json::to_value(app.doc.canvases[0].objects[0].plot().unwrap().figure()).unwrap(),
        before
    );
}
