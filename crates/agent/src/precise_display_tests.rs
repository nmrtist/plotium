use super::*;
use plotium_core::{automation::*, data::*, state::*};
use serde_json::{Value, json};
use std::time::Duration;
fn fixture() -> PlotiumApp {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    app.start_new_project();
    let registry = UnitRegistry::plotium_v1();
    let columns = [
        ("Time", "s", vec![0., 1., 2.]),
        ("Current", "mA", vec![1., 2., 4.]),
        ("Error", "mA", vec![0.1, 0.2, 0.3]),
    ]
    .into_iter()
    .map(|(n, u, v)| {
        let mut c = ColumnSchema::new(n, LogicalType::Float64);
        c.unit = Some(registry.resolve(u).unwrap());
        (c, ColumnChunk::optional_f64(v.into_iter().map(Some)))
    })
    .collect();
    let t = TableDataset::from_materialized(columns, vec![], None, vec![], "plotium.test.r6.agent")
        .unwrap();
    app.doc.datasets.push(Dataset::Table(Box::new(t)));
    app
}
fn create(app: &mut PlotiumApp, s: &mut crate::Session) -> Value {
    let context = s.call(app, "plotium_get_state", json!({})).unwrap()["project_context"].clone();
    let t = app.doc.datasets[0].as_table().unwrap();
    let r = &t.typed_state.envelope.revision;
    let c = &r.snapshot.schema.columns;
    let curve = json!({"kind":"line","source":{"dataset_id":t.resource_id,"field_id":app.doc.datasets[0].default_field_id().unwrap(),"table_id":r.table_id,"table_revision":r.id,"x_column_id":c[0].id,"y_column_id":c[1].id,"y_error_column_id":c[2].id}});
    let p=s.call(app,"plotium_plan",json!({"expected_project_context":context,"expected_revision":0,"tool_id":"plot.create","parameters":{"parent":document_root_target(),"name":"R6","series":[curve.clone(),curve]}})).unwrap();
    let result = s
        .call(app, "plotium_execute", json!({"plan_id":p["plan_id"]}))
        .unwrap();
    let mut creation = result["value"]["creation"].clone();
    creation["bindings"] = result["value"]["bindings"].clone();
    creation
}
fn plan(
    app: &mut PlotiumApp,
    s: &mut crate::Session,
    key: &str,
    value: Value,
    selector: Value,
) -> std::result::Result<Value, crate::session::Failure> {
    let context = s.call(app, "plotium_get_state", json!({})).unwrap()["project_context"].clone();
    let mut a = json!({"expected_project_context":context,"expected_revision":app.doc.automation_revision,"tool_id":"properties.set","parameters":{"key":key,"value":value}});
    if let Some(target) = selector.as_str() {
        a["ids"] = json!([target]);
    } else {
        a["targets"] = json!([selector]);
    }
    s.call(app, "plotium_plan", a)
}
#[test]
fn r6_session_discovery_target_validation_permissions_revision_and_batch_undo() {
    let mut app = fixture();
    let mut s = crate::Session::default();
    s.allow_modify = true;
    let created = create(&mut app, &mut s);
    let curve = created["bindings"][1]["target"].clone();
    let object = created["plot"]["resource"]["id"].clone();
    let catalog = crate::session::property_catalog();
    assert!(
        catalog["properties"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == "series.line.color")
    );
    let context =
        s.call(&mut app, "plotium_get_state", json!({})).unwrap()["project_context"].clone();
    let rev = app.doc.automation_revision;
    let details = s
        .call(
            &mut app,
            "plotium_properties_inspect",
            json!({"expected_project_context":context,"expected_revision":rev,"ids":[object],"key":"object.axes.y_display_unit"}),
        )
        .unwrap();
    let codes = details["value"]["readings"][0]["schema"]["codes"]
        .as_array()
        .unwrap();
    assert!(codes.contains(&json!("A")) && codes.contains(&json!("nA")));
    // The static catalog lists every code; a target narrows it to compatible ones.
    assert!(!codes.contains(&json!("ms")), "{codes:?}");
    assert_eq!(
        plan(
            &mut app,
            &mut s,
            "object.axes.y_range",
            json!({"mode":"manual","min":0.,"max":2.}),
            object.clone()
        )
        .unwrap_err()
        .code,
        "invalid_parameters"
    );
    assert_eq!(
        plan(
            &mut app,
            &mut s,
            "object.axes.y_display_unit",
            json!("ms"),
            object.clone()
        )
        .unwrap_err()
        .code,
        "invalid_parameters"
    );
    let p = plan(
        &mut app,
        &mut s,
        "series.line.color",
        json!("#0072B2"),
        curve.clone(),
    )
    .unwrap();
    s.allow_modify = false;
    assert_eq!(
        s.call(&mut app, "plotium_execute", json!({"plan_id":p["plan_id"]}))
            .unwrap_err()
            .code,
        "permission_denied"
    );
    assert_eq!(
        s.call(&mut app, "plotium_execute", json!({"plan_id":p["plan_id"]}))
            .unwrap_err()
            .code,
        "expired_plan"
    );
    assert_eq!(app.doc.automation_revision, rev);
    s.allow_modify = true;
    let stale = plan(
        &mut app,
        &mut s,
        "series.line.color",
        json!("#0072B2"),
        curve.clone(),
    )
    .unwrap();
    let p = plan(
        &mut app,
        &mut s,
        "series.line.marker",
        json!("square"),
        curve.clone(),
    )
    .unwrap();
    s.call(&mut app, "plotium_execute", json!({"plan_id":p["plan_id"]}))
        .unwrap();
    assert_eq!(
        s.call(
            &mut app,
            "plotium_execute",
            json!({"plan_id":stale["plan_id"]})
        )
        .unwrap_err()
        .code,
        "stale_revision"
    );
    let mut bad = curve.clone();
    bad["component"]["id"] = json!(9999);
    assert_eq!(
        plan(&mut app, &mut s, "series.line.color", json!("#0072B2"), bad)
            .unwrap_err()
            .code,
        "invalid_parameters"
    );
    let p = plan(
        &mut app,
        &mut s,
        "series.line.color",
        json!("#0072B2"),
        curve,
    )
    .unwrap();
    let r = s
        .call(&mut app, "plotium_execute", json!({"plan_id":p["plan_id"]}))
        .unwrap();
    assert_eq!(r["summary"]["succeeded"], 1, "{r}");
    let binding = app.doc.canvases[0].objects[0]
        .plot()
        .unwrap()
        .binding
        .clone();
    app.undo();
    app.redo();
    assert_eq!(
        app.doc.canvases[0].objects[0].plot().unwrap().binding,
        binding
    );
}
#[test]
fn r6_real_http_discovers_all_display_properties_and_renders_same_revision() {
    use rmcp::{
        ServiceExt,
        model::CallToolRequestParams,
        transport::{
            StreamableHttpClientTransport,
            streamable_http_client::StreamableHttpClientTransportConfig,
        },
    };
    let mut app = fixture();
    let mut seed = crate::Session::default();
    seed.allow_modify = true;
    let created = create(&mut app, &mut seed);
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
            let context=client.call_tool(call("plotium_get_state",json!({}))).await.unwrap().structured_content.unwrap()["project_context"].clone();
            let catalog=client.call_tool(call("plotium_properties_list",json!({}))).await.unwrap().structured_content.unwrap();assert_eq!(catalog["properties"].as_array().unwrap().len(),crate::session::PROPERTIES.len());
            let curve=created["bindings"][1]["target"].clone();let object=created["plot"]["resource"]["id"].clone();let canvas=created["canvas"].clone();let mut revision=1;
            for (key,value,series) in [
                ("series.line.color",json!("#0072B2"),true),("series.line.stroke_width",json!(1.5),true),("series.line.style",json!("dash_dot"),true),("series.line.marker",json!("diamond"),true),("series.line.marker_size",json!(5.),true),("series.line.marker_fill",json!("#F2D797"),true),("series.line.marker_edge_color",json!("#404040"),true),("series.line.marker_edge_width",json!(0.5),true),("series.line.marker_hollow",json!(true),true),("series.label",json!("Target"),true),("series.visible",json!(false),true),("series.visible",json!(true),true),
                ("object.axes.x_label",json!("Time"),false),("object.axes.y_label",json!("Current"),false),("object.axes.frame",json!("box"),false),("object.axes.x_display_unit",json!("ms"),false),("object.axes.y_display_unit",json!("A"),false),
                ("object.axes.x_range",json!({"mode":"manual","min":0.,"max":2500.,"unit":"ms"}),false),("object.axes.y_range",json!({"mode":"manual","min":0.,"max":0.01,"unit":"A"}),false),
                ("object.figure.typography.tick_pt",json!(9.),false),("object.figure.typography.label_pt",json!(10.),false),("object.figure.typography.legend_pt",json!(9.),false),
            ] {
                let mut selector=json!({"expected_project_context":context,"expected_revision":revision,"key":key});if series {selector["targets"]=json!([curve]);}else {selector["ids"]=json!([object]);}
                let read=client.call_tool(call("plotium_properties_inspect",selector)).await.unwrap();assert_eq!(read.is_error,Some(false),"{key}");
                let mut args=json!({"expected_project_context":context,"expected_revision":revision,"tool_id":"properties.set","parameters":{"key":key,"value":value}});if series {args["targets"]=json!([curve]);}else {args["ids"]=json!([object]);}
                let pre=client.call_tool(call("plotium_plan",args)).await.unwrap();assert_eq!(pre.is_error,Some(false),"{key}");let pre=pre.structured_content.unwrap();
                let result=client.call_tool(call("plotium_execute",json!({"plan_id":pre["plan_id"]}))).await.unwrap();assert_eq!(result.is_error,Some(false),"{key}");revision=result.structured_content.unwrap()["after_revision"].as_u64().unwrap();
            }
            let image=client.call_tool(call("plotium_render_preview",json!({"expected_project_context":context,"expected_revision":revision,"target":canvas,"max_edge":640}))).await.unwrap();assert_eq!(image.is_error,Some(false));assert!(image.content.iter().any(|c|c.as_image().is_some()));assert_eq!(image.structured_content.unwrap()["value"]["render_revision"],revision);
            client.cancel().await.unwrap();
        });
        tokio::time::timeout(Duration::from_secs(30),async {while !task.is_finished() {if let Some(req)=connection.next_request() {connection.dispatch_request(&mut app,req,false).unwrap();}connection.poll_preview(&mut app).unwrap();tokio::time::sleep(Duration::from_millis(2)).await;}task.await.unwrap();}).await.unwrap();
    });
}

#[test]
fn r6_display_queries_and_plans_cannot_cross_equal_revision_project_contexts() {
    let mut app = fixture();
    let mut session = crate::Session::default();
    session.allow_modify = true;
    let created = create(&mut app, &mut session);
    let target = created["bindings"][1]["target"].clone();
    let state = session
        .call(&mut app, "plotium_get_state", json!({}))
        .unwrap();
    let query = json!({"expected_project_context":state["project_context"],"expected_revision":state["revision"],"targets":[target],"key":"series.line.color"});
    let old = plan(
        &mut app,
        &mut session,
        "series.line.color",
        json!("#0072B2"),
        target,
    )
    .unwrap();
    // Install the same document/IDs at the same revision through the native
    // project-install seam: only the ephemeral project identity changes.
    let mut replacement = fixture();
    std::mem::swap(&mut app.doc, &mut replacement.doc);
    app.install_loaded_project(replacement);
    let after = session
        .call(&mut app, "plotium_get_state", json!({}))
        .unwrap();
    assert_eq!(state["revision"], after["revision"]);
    assert_ne!(state["project_context"], after["project_context"]);
    assert_eq!(
        session
            .call(&mut app, "plotium_properties_inspect", query.clone())
            .unwrap_err()
            .code,
        "project_changed"
    );
    assert_eq!(
        session
            .call(
                &mut app,
                "plotium_execute",
                json!({"plan_id":old["plan_id"]})
            )
            .unwrap_err()
            .code,
        "project_changed"
    );
    let mut fresh = query;
    fresh["expected_project_context"] = after["project_context"].clone();
    let read = session
        .call(&mut app, "plotium_properties_inspect", fresh)
        .unwrap();
    assert_eq!(
        read["value"]["readings"][0]["value"]["value"]["value"],
        "#0f4d92"
    );
    assert_eq!(
        app.doc.automation_revision,
        state["revision"].as_u64().unwrap()
    );
}
