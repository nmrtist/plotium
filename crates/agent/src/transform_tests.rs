use super::*;
use crate::Session;
use plotium_core::{data::*, state::*};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
fn fixture() -> PlotiumApp {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    app.start_new_project();
    let table = TableDataset::from_materialized(
        vec![
            (
                ColumnSchema::new("x_left", LogicalType::Int64),
                ColumnChunk::all_valid(ColumnValues::Int64(vec![0, 1, 2])),
            ),
            (
                ColumnSchema::new("y_left", LogicalType::Float64),
                ColumnChunk::optional_f64([Some(1.), Some(2.), Some(4.)]),
            ),
        ],
        vec![],
        None,
        vec![],
        "plotium.test.r8.agent",
    )
    .unwrap();
    app.doc.datasets.push(Dataset::Table(Box::new(table)));
    app
}
fn args(app: &PlotiumApp) -> Value {
    let t = app.doc.datasets[0].as_table().unwrap();
    let r = &t.typed_state.envelope.revision;
    let c = &r.snapshot.schema.columns;
    let mut column = ColumnSchema::new("signal_x2", LogicalType::Float64);
    column.unit = Some(UnitSpec::dimensionless("a.u."));
    let root = Relation::ComputedColumn {
        input: Box::new(Relation::SnapshotRead(SnapshotRead {
            table: r.table_id,
            revision: r.id,
            fingerprint: r.snapshot.fingerprint,
        })),
        column,
        expression: Expression::call(
            "multiply.v1",
            vec![
                Expression::column(c[1].id),
                Expression::Literal {
                    value: LiteralValue::Float64(FiniteOrSpecial::new(2.)),
                },
            ],
        ),
    };
    json!({"expected_project_context":app.session.project_context,"tool_id":"data.transform","ids":[t.resource_id],"expected_revision":app.doc.automation_revision,"parameters":{"mode":"table_data_only","name":"Scaled","memory_limit_bytes":16*1024*1024,"plan":RelPlanV1::new(root)}})
}
fn pending(
    args: Value,
) -> (
    PendingRequest,
    tokio::sync::oneshot::Receiver<std::result::Result<Value, Failure>>,
) {
    let (response, rx) = tokio::sync::oneshot::channel();
    (
        PendingRequest {
            name: "plotium_plan".into(),
            arguments: args,
            response,
            cancellation: tokio_util::sync::CancellationToken::new(),
            transport: None,
        },
        rx,
    )
}
fn finish(c: &mut Connection, app: &mut PlotiumApp) -> std::result::Result<(), Failure> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while c.transform.is_some() {
        assert!(Instant::now() < deadline);
        c.poll_transform(app, false)?;
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(())
}
#[test]
fn transform_plan_is_data_only_permission_checked_and_consumed_on_failed_execute() {
    let mut app = fixture();
    let mut s = Session::default();
    let a = args(&app);
    assert_eq!(
        s.call(&mut app, "plotium_plan", a.clone())
            .unwrap_err()
            .code,
        "permission_denied"
    );
    s.allow_modify = true;
    let p = s.call(&mut app, "plotium_plan", a.clone()).unwrap();
    assert_eq!(p["transform"]["status"], "prepared");
    assert_eq!(app.doc.datasets.len(), 1);
    s.allow_modify = false;
    let execute = json!({"plan_id":p["plan_id"]});
    assert_eq!(
        s.call(&mut app, "plotium_execute", execute.clone())
            .unwrap_err()
            .code,
        "permission_denied"
    );
    s.allow_modify = true;
    assert_eq!(
        s.call(&mut app, "plotium_execute", execute)
            .unwrap_err()
            .code,
        "expired_plan"
    );
    let p = s.call(&mut app, "plotium_plan", a).unwrap();
    let r = s
        .call(&mut app, "plotium_execute", json!({"plan_id":p["plan_id"]}))
        .unwrap();
    assert_eq!(r["value"]["status"], "succeeded");
    assert_eq!(
        r["produced"][0]["id"],
        p["transform"]["output"]["dataset_id"]
    );
    assert!(app.doc.canvases.is_empty());
    let read = json!({"expected_project_context":app.session.project_context,"expected_revision":app.doc.automation_revision,"dataset_id":r["value"]["output"]["dataset_id"],"table_revision":r["value"]["output"]["table_revision"],"include_provenance":true});
    s.allow_modify = false;
    let rows = s
        .call(&mut app, "plotium_table_read_rows", read.clone())
        .unwrap();
    assert_eq!(rows["value"]["rows"][2]["values"][2]["value"], 8.);
    let metadata = s.call(&mut app, "plotium_table_columns", read).unwrap();
    assert_eq!(
        metadata["value"]["provenance"]["operation"]["plan"],
        p["preflight"]["request"]["parameters"]["plan"]
    );
    app.undo();
    assert_eq!(app.doc.datasets.len(), 1);
    app.redo();
    assert_eq!(
        app.doc.datasets[1].resource_id().to_string(),
        r["produced"][0]["id"]
    );
}
#[test]
fn background_permissions_revisions_cancel_disconnect_and_barriers_discard_candidates() {
    for scenario in [
        "permission",
        "revision",
        "table_revision",
        "cancel",
        "disconnect",
        "barrier",
        "stop",
    ] {
        let mut app = fixture();
        let a = args(&app);
        let mut c = Connection::start(|| {}).unwrap();
        let (request, mut rx) = pending(a);
        let cancel = request.cancellation.clone();
        c.dispatch_request(&mut app, request, false).unwrap();
        assert_eq!(app.doc.datasets.len(), 1);
        // Reads and other native operations remain available during immutable work.
        c.session
            .call(&mut app, "plotium_get_state", json!({}))
            .unwrap();
        match scenario {
            "permission" => c.session.allow_modify = false,
            "revision" => app.doc.automation_revision += 1,
            "table_revision" => {
                app.doc.datasets[0]
                    .as_table_mut()
                    .unwrap()
                    .typed_state
                    .envelope
                    .revision
                    .id = RevisionId::new()
            }
            "cancel" => cancel.cancel(),
            "disconnect" => rx.close(),
            "barrier" => {}
            "stop" => c.stop(),
            _ => unreachable!(),
        }
        if scenario == "stop" {
            assert!(c.transform.is_none());
            continue;
        }
        let error = if scenario == "barrier" {
            let deadline = Instant::now() + Duration::from_secs(10);
            while c.transform.is_some() {
                assert!(Instant::now() < deadline);
                match c.poll_transform(&mut app, true) {
                    Err(e) => {
                        assert_eq!(e.code, "busy");
                        break;
                    }
                    Ok(()) => std::thread::sleep(Duration::from_millis(1)),
                }
            }
            "busy".to_owned()
        } else {
            finish(&mut c, &mut app).unwrap_err().code.to_owned()
        };
        let expected = match scenario {
            "permission" => "permission_denied",
            "revision" | "table_revision" => "stale_revision",
            "cancel" | "disconnect" => "request_cancelled",
            _ => "busy",
        };
        assert_eq!(error, expected);
        assert_eq!(app.doc.datasets.len(), 1);
        if scenario != "disconnect" {
            let error = rx.try_recv().unwrap().unwrap_err();
            assert_eq!(error.code, expected);
            assert_rejected_preflight(&error);
        }
    }
}
#[test]
fn bounded_pending_plans_budget_error_and_transport_schema_are_discoverable() {
    let mut app = fixture();
    let mut s = Session::default();
    s.allow_modify = true;
    let a = args(&app);
    let mut bad = a.clone();
    bad["parameters"]["memory_limit_bytes"] = json!(1);
    assert_eq!(
        s.call(&mut app, "plotium_plan", bad).unwrap_err().code,
        "transform_budget_exceeded"
    );
    assert_eq!(app.doc.datasets.len(), 1);
    for _ in 0..4 {
        s.call(&mut app, "plotium_plan", a.clone()).unwrap();
    }
    assert_eq!(
        s.call(&mut app, "plotium_plan", a.clone())
            .unwrap_err()
            .code,
        "plan_limit"
    );
    let constraint = crate::capabilities::respond(
        json!({"section":"operation_constraints","key":"data.transform"}),
    )
    .unwrap();
    let schema = &constraint["contract"]["schema"];
    assert_eq!(
        schema["properties"]["plan"]["properties"]["version"]["const"],
        1
    );
    assert_eq!(
        schema["$defs"]["transform_relation"]["oneOf"]
            .as_array()
            .unwrap()
            .len(),
        5
    );
    let protocol = crate::protocol::definitions();
    let plan = protocol.iter().find(|t| t.name == "plotium_plan").unwrap();
    let wire = serde_json::to_value(&plan.input_schema).unwrap();
    // The plan tool defers the transform schema to its operation_constraints section.
    assert!(wire.get("$defs").is_none());
    let mut s = Session::default();
    s.allow_modify = true;
    let mut a = a;
    a["parameters"]["memory_limit_bytes"] = json!(TABLE_TRANSFORM_MEMORY);
    s.call(&mut app, "plotium_plan", a.clone()).unwrap();
    assert_eq!(
        s.call(&mut app, "plotium_plan", a).unwrap_err().code,
        "plan_limit"
    );
}
#[test]
fn real_http_transform_creates_a_native_plot_and_returns_its_preview() {
    use rmcp::{
        ServiceExt,
        model::CallToolRequestParams,
        transport::{
            StreamableHttpClientTransport,
            streamable_http_client::StreamableHttpClientTransportConfig,
        },
    };
    let mut app = fixture();
    let a = args(&app);
    let context = app.session.project_context;
    let mut c = Connection::start(|| {}).unwrap();
    let mut config = StreamableHttpClientTransportConfig::with_uri(c.url.clone());
    config.auth_header = Some(c.token.clone());
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let client=tokio::spawn(async move {
            let http=reqwest::Client::builder().no_proxy().build().unwrap();let client=().serve(StreamableHttpClientTransport::with_client(http,config)).await.unwrap();
            let call=|name:&'static str,args:Value|CallToolRequestParams::new(name).with_arguments(args.as_object().unwrap().clone());
            let tools=client.list_all_tools().await.unwrap();assert_eq!(tools.len(),crate::protocol::definitions().len());
            let p=client.call_tool(call("plotium_plan",a)).await.unwrap();assert_eq!(p.is_error,Some(false));let p=p.structured_content.unwrap();assert_eq!(p["transform"]["status"],"prepared");
            let r=client.call_tool(call("plotium_execute",json!({"plan_id":p["plan_id"]}))).await.unwrap();assert_eq!(r.is_error,Some(false));let r=r.structured_content.unwrap();let out=&r["value"]["output"];
            let rows=client.call_tool(call("plotium_table_read_rows",json!({"expected_project_context":context,"dataset_id":out["dataset_id"],"table_revision":out["table_revision"],"expected_revision":r["after_revision"],"include_provenance":true}))).await.unwrap();assert_eq!(rows.is_error,Some(false));let v=rows.structured_content.unwrap();assert_eq!(v["value"]["rows"][2]["values"][2]["value"],8.);assert_eq!(v["value"]["rows"][2]["sources"][0]["input_position"],2);
            let metadata=client.call_tool(call("plotium_table_columns",json!({"expected_project_context":context,"dataset_id":out["dataset_id"],"table_revision":out["table_revision"],"expected_revision":r["after_revision"],"include_provenance":true}))).await.unwrap();assert_eq!(metadata.is_error,Some(false));let metadata=metadata.structured_content.unwrap();let m=&metadata["value"];
            let state=client.call_tool(call("plotium_get_state",json!({}))).await.unwrap().structured_content.unwrap();
            let discovery=client.call_tool(call("plotium_capabilities_get",json!({}))).await.unwrap().structured_content.unwrap();
            for key in ["data.transform","plot.create"] {let detail=client.call_tool(call("plotium_capabilities_get",json!({"section":"operation_constraints","key":key}))).await.unwrap().structured_content.unwrap();assert!(detail["contract"]["schema"].is_object());}
            let create=client.call_tool(call("plotium_plan",json!({"expected_project_context":context,"expected_revision":state["revision"],"tool_id":"plot.create","parameters":{"parent":discovery["connection_capabilities"]["document_root"],"name":"Processed native plot","series":[{"kind":"line","source":{"dataset_id":m["dataset_id"],"field_id":m["fields"][0]["field_id"],"table_id":m["table_id"],"table_revision":m["table_revision"],"x_column_id":m["columns"][0]["column_id"],"y_column_id":m["columns"][2]["column_id"],"y_error_column_id":null}}]}}))).await.unwrap();assert_eq!(create.is_error,Some(false));let create=create.structured_content.unwrap();
            let created=client.call_tool(call("plotium_execute",json!({"plan_id":create["plan_id"]}))).await.unwrap();assert_eq!(created.is_error,Some(false));let created=created.structured_content.unwrap();
            let canvas=created["value"]["creation"]["canvas"].clone();let target=created["value"]["bindings"][0]["target"].clone();
            let detail=client.call_tool(call("plotium_document_inspect",json!({"expected_project_context":context,"expected_revision":created["after_revision"],"target":target,"view":"self"}))).await.unwrap();assert_eq!(detail.is_error,Some(false));let detail=detail.structured_content.unwrap();
            assert_eq!(detail["value"]["node"]["source"]["dataset"]["id"],out["dataset_id"]);
            let image=client.call_tool(call("plotium_render_preview",json!({"expected_project_context":context,"expected_revision":created["after_revision"],"target":canvas,"max_edge":320}))).await.unwrap();assert_eq!(image.is_error,Some(false));assert!(image.content.iter().any(|c|c.as_image().is_some()));assert_eq!(image.structured_content.unwrap()["value"]["render_revision"],created["after_revision"]);
            client.cancel().await.unwrap();
        });
        tokio::time::timeout(Duration::from_secs(30),async{while !client.is_finished(){while let Some(r)=c.next_request(){c.dispatch_request(&mut app,r,false).unwrap();}c.poll_transform(&mut app,false).unwrap();c.poll_preview(&mut app).unwrap();tokio::time::sleep(Duration::from_millis(2)).await;}client.await.unwrap();}).await.unwrap();
    });
    assert_eq!(app.doc.datasets.len(), 2);
    let canvas = app.doc.canvases[0].resource_id;
    let object = app.doc.canvases[0].objects[0].id;
    let plot = app.doc.canvases[0].objects[0].plot().unwrap();
    let binding = plot.binding.clone();
    assert_eq!(
        plot.figure().series[0].points,
        vec![[0., 2.], [1., 4.], [2., 8.]]
    );
    app.undo();
    assert!(app.doc.canvases.is_empty());
    assert_eq!(app.doc.datasets.len(), 2);
    app.redo();
    assert_eq!(app.doc.canvases[0].resource_id, canvas);
    assert_eq!(app.doc.canvases[0].objects[0].id, object);
    assert_eq!(
        app.doc.canvases[0].objects[0].plot().unwrap().binding,
        binding
    );
}

#[test]
fn project_switch_rejects_transform_inputs_plans_and_background_completion() {
    let mut app = fixture();
    let mut c = Connection::start(|| {}).unwrap();
    let arguments = args(&app);
    let plan = c
        .session
        .call(&mut app, "plotium_plan", arguments.clone())
        .unwrap();
    let (request, mut response) = pending(arguments.clone());
    let cancellation = request.cancellation.clone();
    c.dispatch_request(&mut app, request, false).unwrap();
    assert!(c.transform.is_some());
    let revision = app.doc.automation_revision;
    app.start_new_project();
    assert_eq!(app.doc.automation_revision, revision);
    c.poll_transform(&mut app, false).unwrap();
    assert!(c.transform.is_none());
    assert!(!cancellation.is_cancelled());
    let error = response.try_recv().unwrap().unwrap_err();
    assert_eq!(error.code, "project_changed");
    assert_rejected_preflight(&error);
    assert_eq!(
        c.session
            .call(&mut app, "plotium_plan", arguments)
            .unwrap_err()
            .code,
        "project_changed"
    );
    assert!(app.doc.automation_runs.is_empty());
    c.session
        .call(&mut app, "plotium_get_state", json!({}))
        .unwrap();
    assert_eq!(
        c.session
            .call(
                &mut app,
                "plotium_execute",
                json!({"plan_id":plan["plan_id"]})
            )
            .unwrap_err()
            .code,
        "project_changed"
    );
    assert!(app.doc.datasets.is_empty());
}

fn assert_rejected_preflight(error: &Failure) {
    let value = error.value();
    let recovery = &value["error"]["recovery"];
    assert_eq!(recovery["plan_consumed"], false);
    assert_eq!(recovery["submission"], "not_submitted");
    assert_eq!(recovery["committed"], false);
    assert_eq!(recovery["completion"], "failed");
    assert_eq!(recovery["replay_allowed"], false);
    assert_eq!(value["error"]["retryable"], false);
}

#[test]
fn desktop_table_preflight_rejection_reports_known_non_submission() {
    for operation in ["data.transform", "table.patch", "table.refresh"] {
        for mode in ["busy", "permission_denied"] {
            let mut app = fixture();
            let mut arguments = args(&app);
            arguments["tool_id"] = json!(operation);
            let mut connection = Connection::start(|| {}).unwrap();
            if mode == "permission_denied" {
                connection.session.set_allow_modify(false);
            }
            let (request, response) = pending(arguments);
            let error = connection
                .dispatch_request(&mut app, request, mode == "busy")
                .unwrap_err();
            assert_eq!(error.code, mode);
            assert_rejected_preflight(&error);
            assert_eq!(
                response.blocking_recv().unwrap().unwrap_err().value(),
                error.value()
            );
            assert!(connection.transform.is_none());
            assert_eq!(app.doc.datasets.len(), 1);
            assert!(!app.doc.dirty);
        }
    }
}
