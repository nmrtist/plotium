use super::*;
use crate::{Connection, Session};
use plotium_core::state::*;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
struct Input(PathBuf);
impl Input {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("plotium-r3-agent-{}.csv", uuid::Uuid::new_v4()));
        std::fs::write(&p, "time,value,note\n0,2,first\n1,,second\n2,6,third\n").unwrap();
        Self(p)
    }
}
impl Drop for Input {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).unwrap();
    }
}
fn arguments(file: &Input, revision: u64, context: ProjectContextId) -> Value {
    json!({"tool_id":"data.import","parameters":{"mode":"table_data_only","paths":[file.0],"header":"present"},"expected_project_context":context,"expected_revision":revision})
}
fn pending(
    args: Value,
) -> (
    PendingRequest,
    tokio::sync::oneshot::Receiver<Result<Value, Failure>>,
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
fn finish(c: &mut Connection, app: &mut PlotiumApp) -> Result<(), Failure> {
    let until = Instant::now() + Duration::from_secs(10);
    while c.import.is_some() {
        if Instant::now() > until {
            panic!("import timeout");
        }
        c.poll_import(app, false)?;
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(())
}
#[test]
fn data_only_import_requires_permission_and_one_use_plan() {
    let f = Input::new();
    let mut s = Session::default();
    let mut app = PlotiumApp::new();
    let context = app.session.project_context;
    assert_eq!(
        s.call(&mut app, "plotium_plan", arguments(&f, 0, context))
            .unwrap_err()
            .code,
        "permission_denied"
    );
    s.allow_modify = true;
    let plan = s
        .call(&mut app, "plotium_plan", arguments(&f, 0, context))
        .unwrap();
    assert!(app.doc.datasets.is_empty());
    assert_eq!(plan["import"]["status"], "prepared");
    let proposed = plan["import"]["items"][0]["metadata"]["dataset_id"].clone();
    s.allow_modify = false;
    let execute = json!({"plan_id":plan["plan_id"]});
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
    assert!(app.doc.datasets.is_empty());
    let plan = s
        .call(&mut app, "plotium_plan", arguments(&f, 0, context))
        .unwrap();
    let result = s
        .call(
            &mut app,
            "plotium_execute",
            json!({"plan_id":plan["plan_id"]}),
        )
        .unwrap();
    assert_eq!(result["value"]["status"], "succeeded");
    assert_ne!(result["produced"][0]["id"], proposed);
    assert!(app.doc.canvases.is_empty());
    assert!(app.session.active_canvas.is_none());
    assert!(app.doc.dirty);
    assert_eq!(app.doc.automation_runs.len(), 1);
    let id = app.doc.datasets[0].resource_id();
    app.undo();
    assert!(app.doc.datasets.is_empty());

    assert!(app.doc.dirty);
    app.redo();
    assert_eq!(app.doc.datasets[0].resource_id(), id);
    s.allow_modify = false;
    let metadata = &result["value"]["items"][0]["metadata"];
    let read = json!({"expected_project_context":context,"expected_revision":app.doc.automation_revision,"dataset_id":metadata["dataset_id"],"table_revision":metadata["table_revision"]});
    assert_eq!(
        s.call(&mut app, "plotium_table_columns", read.clone())
            .unwrap()["value"]["total_rows"],
        3
    );
    assert_eq!(
        s.call(&mut app, "plotium_table_read_rows", read).unwrap()["value"]["rows"][1]["values"][1]
            ["kind"],
        "null"
    );
}
#[test]
fn all_failed_import_keeps_pristine_project_clean_and_undo_free() {
    let dir = tempfile::tempdir().unwrap();
    for parameters in [
        json!({"mode":"table_data_only","paths":[dir.path().join("missing.csv")],"header":"present"}),
        json!({"paths":[dir.path().join("missing.jdf")]}),
    ] {
        let mut app = PlotiumApp::new_with_settings(Default::default());
        let mut s = Session::default();
        s.set_allow_modify(true);
        let context = app.session.project_context;
        let plan = s.call(&mut app, "plotium_plan", json!({
            "expected_project_context":context,"expected_revision":0,"tool_id":"data.import",
            "parameters":parameters
        })).unwrap();
        let result = s
            .call(
                &mut app,
                "plotium_execute",
                json!({"plan_id":plan["plan_id"]}),
            )
            .unwrap();
        assert_eq!(result["value"]["status"], "failed");
        assert_eq!(app.session.project_context, context);
        assert!(app.doc.datasets.is_empty() && app.doc.canvases.is_empty());
        assert!(!app.doc.dirty);
        assert_eq!(app.doc.edit_generation, 0);
        assert_eq!(app.doc.automation_revision, 0);
        assert!(app.doc.automation_runs.is_empty());
        assert!(app.session.undo_stack.is_empty() && app.session.redo_stack.is_empty());
    }
}

#[test]
fn background_preflight_keeps_other_tools_available_and_returns_only_when_prepared() {
    let f = Input::new();
    let mut app = PlotiumApp::new();
    let context = app.session.project_context;
    let mut c = Connection::start(|| {}).unwrap();
    let (req, mut rx) = pending(arguments(&f, 0, context));
    c.dispatch_request(&mut app, req, false).unwrap();
    assert!(app.doc.datasets.is_empty());
    assert!(rx.try_recv().is_err());
    let state = c
        .session
        .call(&mut app, "plotium_get_state", json!({}))
        .unwrap();
    assert_eq!(state["revision"], 0);
    let (duplicate, rx2) = pending(arguments(&f, 0, context));
    assert_eq!(
        c.dispatch_request(&mut app, duplicate, false)
            .unwrap_err()
            .code,
        "busy"
    );
    assert_rejected_preflight(&rx2.blocking_recv().unwrap().unwrap_err());
    finish(&mut c, &mut app).unwrap();
    let plan = rx.blocking_recv().unwrap().unwrap();
    assert_eq!(plan["import"]["status"], "prepared");
    assert!(app.doc.datasets.is_empty());
    let result = c
        .session
        .call(
            &mut app,
            "plotium_execute",
            json!({"plan_id":plan["plan_id"]}),
        )
        .unwrap();
    assert_eq!(
        result["value"]["items"][0]["metadata"]["dataset_id"],
        plan["import"]["items"][0]["metadata"]["dataset_id"]
    );
}
#[test]
fn project_change_stops_import_worker_without_cancelling_its_response() {
    let file = Input::new();
    let mut app = PlotiumApp::new_with_settings(Default::default());
    app.start_new_project();
    let mut connection = Connection::start(|| {}).unwrap();
    let (request, response) = pending(arguments(&file, 0, app.session.project_context));
    let request_cancellation = request.cancellation.clone();
    connection
        .dispatch_request(&mut app, request, false)
        .unwrap();
    let worker_cancellation = connection.import.as_ref().unwrap().cancellation.clone();
    app.start_new_project();
    connection.synchronize_project(&app);
    assert!(worker_cancellation.is_cancelled());
    assert!(!request_cancellation.is_cancelled());
    let error = response.blocking_recv().unwrap().unwrap_err();
    assert_eq!(error.code, "project_changed");
    assert_rejected_preflight(&error);
    assert!(app.doc.datasets.is_empty());
    assert!(app.doc.automation_runs.is_empty());
    assert!(!app.doc.dirty);
}
#[test]
fn cancelled_stopped_stale_or_revoked_preflight_never_installs_data() {
    let f = Input::new();
    for mode in ["cancel", "stop", "stale", "permission", "barrier"] {
        let mut app = PlotiumApp::new();
        let context = app.session.project_context;
        let mut c = Connection::start(|| {}).unwrap();
        let (req, rx) = pending(arguments(&f, 0, context));
        let token = req.cancellation.clone();
        c.dispatch_request(&mut app, req, false).unwrap();
        match mode {
            "cancel" => token.cancel(),
            "stop" => c.stop(),
            "stale" => app.doc.automation_revision += 1,
            "permission" => c.session.allow_modify = false,
            _ => (),
        }
        if mode == "barrier" {
            let until = Instant::now() + Duration::from_secs(10);
            while !c.import.as_ref().unwrap().worker.is_finished() {
                assert!(Instant::now() < until);
                std::thread::sleep(Duration::from_millis(1));
            }
            assert_eq!(c.poll_import(&mut app, true).unwrap_err().code, "busy");
        } else if mode != "stop" {
            let error = finish(&mut c, &mut app).unwrap_err();
            assert_eq!(
                error.code,
                match mode {
                    "cancel" => "request_cancelled",
                    "stale" => "stale_revision",
                    _ => "permission_denied",
                }
            );
        }
        assert!(app.doc.datasets.is_empty());
        match rx.blocking_recv() {
            Ok(result) => assert_rejected_preflight(&result.unwrap_err()),
            Err(_) => assert_eq!(mode, "stop"),
        }
        assert!(!app.doc.dirty);
        assert!(app.doc.automation_runs.is_empty());
        assert!(app.session.undo_stack.is_empty() && app.session.redo_stack.is_empty());
    }
}
#[test]
fn schema_discovery_permissions_and_unexposed_core_tools_are_precise() {
    let mut s = Session::default();
    let mut app = PlotiumApp::new();
    let context = app.session.project_context;
    let state = s
        .call(&mut app, "plotium_capabilities_get", json!({}))
        .unwrap();
    assert_eq!(
        state["connection_capabilities"]["tools"]
            .as_array()
            .unwrap()
            .len(),
        crate::protocol::definitions().len()
    );
    let limits = s
        .call(
            &mut app,
            "plotium_capabilities_get",
            json!({"section":"limits"}),
        )
        .unwrap();
    let caps = &json!({"limits":limits["contract"]});
    assert_eq!(caps["limits"]["table_read"]["max_rows"], 1000);
    assert_eq!(caps["limits"]["table_import"]["files"], 16);
    assert_eq!(
        caps["limits"]["table_import"]["schema_file_bytes"],
        256 * 1024
    );
    assert_eq!(
        caps["limits"]["table_import"]["prepared_summary_bytes"],
        128 * 1024
    );
    assert_eq!(
        caps["limits"]["table_import"]["preflight_response_bytes"],
        256 * 1024
    );
    assert_eq!(
        s.call(
            &mut app,
            "plotium_plan",
            json!({"expected_project_context":context,"expected_revision":0,"ids":["document"],"tool_id":"results.compare","parameters":{}})
        )
        .unwrap_err()
        .code,
        "unsupported_operation"
    );
    let f = Input::new();
    s.allow_modify = true;
    let mut args = arguments(&f, 0, context);
    args["ids"] = json!(["document"]);
    assert_eq!(
        s.call(&mut app, "plotium_plan", args).unwrap_err().code,
        "invalid_parameters"
    );
}
#[test]
fn import_worker_failure_reaches_client_and_keeps_document_unchanged() {
    let f = Input::new();
    let mut app = PlotiumApp::new();
    let context = app.session.project_context;
    let mut c = Connection::start(|| {}).unwrap();
    let args = arguments(&f, 0, context);
    let (plan, _) = c
        .session
        .prepare_import_plan(&app, args.clone(), false)
        .unwrap();
    let (request, rx) = pending(args);
    c.import = Some(ImportFlight {
        worker: std::thread::spawn(|| panic!("injected import worker failure")),
        plan,
        cancellation: request.cancellation.child_token(),
        request,
    });
    assert_eq!(
        finish(&mut c, &mut app).unwrap_err().code,
        "execution_failed"
    );
    assert_rejected_preflight(&rx.blocking_recv().unwrap().unwrap_err());
    assert!(app.doc.datasets.is_empty());
}

#[test]
fn real_http_import_columns_and_page_10001_use_the_same_native_table() {
    use rmcp::{
        ServiceExt,
        model::CallToolRequestParams,
        transport::{
            StreamableHttpClientTransport,
            streamable_http_client::StreamableHttpClientTransportConfig,
        },
    };
    let f = Input::new();
    let mut text = String::from("time,value,note\n");
    for i in 0..10005 {
        text.push_str(&format!("{i},{},row{i}\n", i * 2));
    }
    std::fs::write(&f.0, text).unwrap();
    let mut app = PlotiumApp::new();
    let context = app.session.project_context;
    let args = arguments(&f, 0, context);
    let mut c = Connection::start(|| {}).unwrap();
    let mut config = StreamableHttpClientTransportConfig::with_uri(c.url.clone());
    config.auth_header = Some(c.token.clone());
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let client = tokio::spawn(async move {
            let http = reqwest::Client::builder().no_proxy().build().unwrap();
            let client =
                ().serve(StreamableHttpClientTransport::with_client(http, config))
                    .await
                    .unwrap();
            async fn call(
                client: &rmcp::service::RunningService<rmcp::RoleClient, ()>,
                name: &str,
                args: Value,
            ) -> Value {
                let result = client
                    .call_tool(
                        CallToolRequestParams::new(name.to_owned())
                            .with_arguments(args.as_object().unwrap().clone()),
                    )
                    .await
                    .unwrap();
                assert_eq!(result.is_error, Some(false));
                result.structured_content.unwrap()
            }
            let plan = call(&client, "plotium_plan", args).await;
            assert_eq!(plan["import"]["status"], "prepared");
            let result = call(
                &client,
                "plotium_execute",
                json!({"plan_id":plan["plan_id"]}),
            )
            .await;
            let metadata = &result["value"]["items"][0]["metadata"];
            let id = metadata["dataset_id"].clone();
            let revision = metadata["table_revision"].clone();
            let p = json!({"dataset_id":id,"table_revision":revision,"expected_project_context":context,"expected_revision":1});
            let columns = call(&client, "plotium_table_columns", p.clone()).await;
            assert_eq!(columns["value"]["columns"].as_array().unwrap().len(), 3);
            assert_eq!(columns["value"]["total_rows"], 10005);
            let mut p = p;
            let mut count = 0;
            let mut ids = std::collections::BTreeSet::new();
            loop {
                p["offset"] = json!(count);
                p["limit"] = json!(200);
                let page = call(&client, "plotium_table_read_rows", p.clone()).await;
                for row in page["value"]["rows"].as_array().unwrap() {
                    assert!(ids.insert(row["row_id"].as_str().unwrap().to_owned()));
                    if count == 10000 {
                        assert_eq!(row["values"][0]["value"], "10000");
                        assert_eq!(row["values"][1]["value"], "20000");
                    }
                    count += 1;
                }
                if page["value"]["end"] == true {
                    break;
                }
            }
            assert_eq!(count, 10005);
            assert_eq!(ids.len(), 10005);
            p["expected_revision"] = json!(0);
            let error = client
                .call_tool(
                    CallToolRequestParams::new("plotium_table_read_rows")
                        .with_arguments(p.as_object().unwrap().clone()),
                )
                .await
                .unwrap();
            assert_eq!(error.is_error, Some(true));
            assert_eq!(
                error.structured_content.unwrap()["error"]["code"],
                "stale_revision"
            );
            client.cancel().await.unwrap();
        });
        tokio::time::timeout(Duration::from_secs(30), async {
            while !client.is_finished() {
                if let Some(req) = c.next_request() {
                    if req.name == "plotium_table_columns" {
                        c.session.allow_modify = false;
                    }
                    if let Err(e) = c.dispatch_request(&mut app, req, false) {
                        assert_eq!(e.code, "stale_revision");
                    }
                }
                c.poll_import(&mut app, false).unwrap();
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            client.await.unwrap();
        })
        .await
        .unwrap();
    });
    assert_eq!(app.doc.datasets.len(), 1);
    assert!(app.doc.canvases.is_empty());
    let Dataset::Table(t) = &app.doc.datasets[0] else {
        panic!()
    };
    assert_eq!(
        t.typed_rows_page(10000, 1, &[]).unwrap().columns[1].values[0],
        plotium_core::data::ScalarValue::Int64(20000)
    );
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
fn desktop_import_rejection_reports_known_non_submission() {
    let file = Input::new();
    for mode in ["busy", "permission_denied", "stale_revision"] {
        let mut app = PlotiumApp::new_with_settings(Default::default());
        let mut connection = Connection::start(|| {}).unwrap();
        let args = arguments(&file, 0, app.session.project_context);
        if mode == "permission_denied" {
            connection.session.set_allow_modify(false);
        }
        if mode == "stale_revision" {
            app.doc.automation_revision += 1;
        }
        let (request, response) = pending(args);
        let error = connection
            .dispatch_request(&mut app, request, mode == "busy")
            .unwrap_err();
        assert_eq!(error.code, mode);
        assert_rejected_preflight(&error);
        let delivered = response.blocking_recv().unwrap().unwrap_err();
        assert_eq!(delivered.value(), error.value());
        assert!(connection.import.is_none());
        assert!(app.doc.datasets.is_empty());
        assert!(!app.doc.dirty);
    }
}
