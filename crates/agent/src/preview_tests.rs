use super::*;
use plotium_core::{automation::*, state::*};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
fn fixture() -> PlotiumApp {
    let mut app = PlotiumApp::new();
    let mut c = CanvasDocument::new("Preview test".into(), [100., 50.]);
    let id = c.allocate_object_id();
    c.objects.push(ContentItem {
        id,
        name: "Text".into(),
        frame: ObjectFrame::new(5., 5., 150., 20.),
        locked: false,
        visible: true,
        kind: ContentKind::Text(TextBox::label("SNAPSHOT BEFORE".into())),
    });
    app.doc.canvases.push(c);
    app
}
fn args(app: &PlotiumApp) -> Value {
    let context = app.session.project_context;
    json!({"expected_project_context":context,"expected_revision":app.doc.automation_revision,"target":TargetRef::resource(app.doc.canvases[0].resource_id.into()),"max_edge":320})
}
fn pending(arguments: Value) -> (PendingRequest, oneshot::Receiver<Result<Value, Failure>>) {
    let (response, receive) = oneshot::channel();
    (
        PendingRequest {
            name: "plotium_render_preview".into(),
            arguments,
            response,
            cancellation: CancellationToken::new(),
            transport: None,
        },
        receive,
    )
}
fn wait(c: &mut Connection, app: &mut PlotiumApp) {
    let until = Instant::now() + Duration::from_secs(10);
    while c.preview.is_some() && Instant::now() < until {
        if let Err(error) = c.poll_preview(app) {
            assert_eq!(error.code, "request_cancelled");
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(c.preview.is_none());
}
#[test]
fn desktop_preview_is_background_readonly_bounded_and_revision_frozen() {
    let mut app = fixture();
    let mut c = Connection::start(|| {}).unwrap();
    c.session.allow_modify = false;
    let (req, receive) = pending(args(&app));
    c.dispatch_request(&mut app, req, false).unwrap();
    assert!(c.preview.is_some());
    let (req, rx) = pending(args(&app));
    assert_eq!(
        c.dispatch_request(&mut app, req, false).unwrap_err().code,
        "busy"
    );
    assert_eq!(rx.blocking_recv().unwrap().unwrap_err().code, "busy");
    app.execute_action(plotium_core::actions::Action::rename_canvas(
        0,
        "Preview test".into(),
        "After capture".into(),
    ));
    let state = c
        .session
        .call(&mut app, "plotium_get_state", json!({}))
        .unwrap();
    assert_eq!(state["revision"], 1);
    wait(&mut c, &mut app);
    let result = receive.blocking_recv().unwrap().unwrap();
    assert_eq!(result["value"]["render_revision"], 0);
    assert_eq!(result["after_revision"], 0);
    let image = crate::protocol::image_result(result, &serde_json::from_value(json!(1)).unwrap());
    assert_eq!(
        image
            .content
            .iter()
            .filter(|v| v.as_image().is_some())
            .count(),
        1
    );
    assert!(
        image
            .content
            .iter()
            .filter_map(|v| v.as_text())
            .all(|v| !v.text.contains("png_base64"))
    );
    assert_eq!(image.structured_content.unwrap()["value"]["width_px"], 320);
    assert!(app.doc.automation_runs.is_empty());
}
#[test]
fn cancellation_stop_barrier_and_stale_revision_are_not_successful_previews() {
    let mut app = fixture();
    let mut c = Connection::start(|| {}).unwrap();
    let (req, rx) = pending(args(&app));
    assert_eq!(
        c.dispatch_request(&mut app, req, true).unwrap_err().code,
        "busy"
    );
    assert!(rx.blocking_recv().unwrap().is_err());
    let (req, rx) = pending(args(&app));
    let token = req.cancellation.clone();
    c.dispatch_request(&mut app, req, false).unwrap();
    token.cancel();
    wait(&mut c, &mut app);
    assert_eq!(
        rx.blocking_recv().unwrap().unwrap_err().code,
        "request_cancelled"
    );
    assert!(app.doc.automation_runs.is_empty());
    let (req, rx) = pending(args(&app));
    c.dispatch_request(&mut app, req, false).unwrap();
    let runs = app.doc.automation_runs.len();
    c.stop();
    assert!(c.preview.is_none());
    assert!(rx.blocking_recv().is_err());
    assert_eq!(app.doc.automation_runs.len(), runs);
    let mut c = Connection::start(|| {}).unwrap();
    let mut a = args(&app);
    a["expected_revision"] = json!(99);
    let (req, rx) = pending(a);
    assert_eq!(
        c.dispatch_request(&mut app, req, false).unwrap_err().code,
        "stale_revision"
    );
    assert!(rx.blocking_recv().unwrap().is_err());
}
#[test]
fn real_http_preview_returns_native_mcp_image_and_metadata_not_json_image_text() {
    use rmcp::{
        ServiceExt,
        model::CallToolRequestParams,
        transport::{
            StreamableHttpClientTransport,
            streamable_http_client::StreamableHttpClientTransportConfig,
        },
    };
    let mut app = fixture();
    let arguments = args(&app);
    let mut c = Connection::start(|| {}).unwrap();
    c.session.allow_modify = false;
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
            let result = client
                .call_tool(
                    CallToolRequestParams::new("plotium_render_preview")
                        .with_arguments(arguments.as_object().unwrap().clone()),
                )
                .await
                .unwrap();
            assert_eq!(result.is_error, Some(false));
            let image = result
                .content
                .iter()
                .find_map(|v| v.as_image())
                .expect("actual MCP image block");
            assert_eq!(image.mime_type, "image/png");
            assert!(image.data.starts_with("iVBOR"));
            assert!(
                result
                    .content
                    .iter()
                    .filter_map(|v| v.as_text())
                    .all(|v| !v.text.contains("png_base64"))
            );
            assert_eq!(
                result.structured_content.unwrap()["value"]["render_revision"],
                0
            );
            for (key, value, code) in [
                ("expected_revision", json!(99), "stale_revision"),
                ("max_edge", json!(2049), "preview_budget_exceeded"),
            ] {
                let mut invalid = arguments.clone();
                invalid[key] = value;
                let result = client
                    .call_tool(
                        CallToolRequestParams::new("plotium_render_preview")
                            .with_arguments(invalid.as_object().unwrap().clone()),
                    )
                    .await
                    .unwrap();
                assert_eq!(result.is_error, Some(true));
                assert!(result.content.iter().all(|v| v.as_image().is_none()));
                assert_eq!(result.structured_content.unwrap()["error"]["code"], code);
            }
            let mut invalid = arguments;
            invalid["target"]["resource"]["id"] = json!("missing-canvas");
            let result = client
                .call_tool(
                    CallToolRequestParams::new("plotium_render_preview")
                        .with_arguments(invalid.as_object().unwrap().clone()),
                )
                .await
                .unwrap();
            assert_eq!(result.is_error, Some(true));
            assert_eq!(
                result.structured_content.unwrap()["error"]["code"],
                "invalid_parameters"
            );
            client.cancel().await.unwrap();
        });
        tokio::time::timeout(Duration::from_secs(15), async {
            while !client.is_finished() {
                if let Some(req) = c.next_request()
                    && let Err(error) = c.dispatch_request(&mut app, req, false)
                {
                    assert!(matches!(
                        error.code,
                        "stale_revision" | "preview_budget_exceeded" | "invalid_parameters"
                    ));
                }
                c.poll_preview(&mut app).unwrap();
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
            client.await.unwrap();
        })
        .await
        .unwrap();
    });
}

#[test]
fn tool_result_budget_and_discovery_are_explicit() {
    let mut app = fixture();
    let mut s = crate::Session::default();
    let state = s.call(&mut app, "plotium_get_state", json!({})).unwrap();
    let tools = state["connection_capabilities"]["tools"]
        .as_array()
        .unwrap();
    let t = tools
        .iter()
        .find(|v| v["name"] == "plotium_render_preview")
        .unwrap();
    assert_eq!(t["required_permission"], "read");
    assert_eq!(
        state["connection_capabilities"]["limits"]["image_preview"]["png_bytes"],
        PREVIEW_PNG_BYTES
    );
    let response = crate::protocol::image_result(
        json!({"value":{"png_base64":"x".repeat(PREVIEW_RESPONSE_BYTES)}}),
        &serde_json::from_value(json!(1)).unwrap(),
    );
    assert_eq!(response.is_error, Some(true));
    assert!(response.content.iter().all(|v| v.as_image().is_none()));
    assert_eq!(
        response.structured_content.unwrap()["error"]["code"],
        "preview_budget_exceeded"
    );
}

#[test]
fn response_budget_includes_the_json_rpc_envelope_and_request_id() {
    let response = crate::protocol::image_result(
        json!({"value":{"png_base64":"small"}}),
        &serde_json::from_value(json!("x".repeat(PREVIEW_RESPONSE_BYTES))).unwrap(),
    );
    assert_eq!(response.is_error, Some(true));
    assert!(response.content.iter().all(|v| v.as_image().is_none()));
}

#[test]
fn worker_failure_reaches_client_and_leaves_a_failed_run() {
    let mut app = fixture();
    let mut connection = Connection::start(|| {}).unwrap();
    let (request, receive) = pending(args(&app));
    let (_, run) = connection
        .session
        .prepare_preview(&mut app, request.arguments.clone(), false)
        .unwrap();
    connection.preview = Some(crate::preview::PreviewFlight {
        worker: std::thread::spawn(|| panic!("controlled renderer failure")),
        run,
        candidate: None,
        cancellation: request.cancellation.child_token(),
        request,
    });
    let until = Instant::now() + Duration::from_secs(5);
    while !connection.preview.as_ref().unwrap().worker.is_finished() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        connection.poll_preview(&mut app).unwrap_err().code,
        "execution_failed"
    );
    assert_eq!(
        receive.blocking_recv().unwrap().unwrap_err().code,
        "execution_failed"
    );
    assert!(app.doc.automation_runs.is_empty());
}
