use super::*;
use rmcp::{
    ServiceExt,
    model::CallToolRequestParams,
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde_json::json;
use std::time::Duration;

#[test]
fn real_http_mcp_handshake_discovery_bridge_and_authentication() {
    let mut connection = Connection::start(|| {}).unwrap();
    let url = connection.url.clone();
    let token = connection.token.clone();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut app = plotium_core::state::PlotiumApp::new();
    app.session.project_present = true;
    runtime.block_on(async {
        let client = tokio::spawn(async move {
            let http = reqwest::Client::builder().no_proxy().build().unwrap();
            let response = http.post(&url).send().await.unwrap();
            assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
            for (header, value) in [
                ("Host", "evil.example"),
                ("Origin", "http://evil.example"),
                ("Origin", "null"),
            ] {
                let response = http
                    .post(&url)
                    .bearer_auth(&token)
                    .header(header, value)
                    .send()
                    .await
                    .unwrap();
                assert_eq!(response.status(), reqwest::StatusCode::FORBIDDEN);
            }
            let origin = url.trim_end_matches("/mcp");
            let response = http.post(&url).bearer_auth(&token)
                .header("Origin", origin)
                .header("Accept", "application/json, text/event-stream")
                .header("Content-Type", "application/json")
                .body(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
                    "protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"Plotium test","version":"1"}
                }}).to_string()).send().await.unwrap();
            assert_eq!(response.status(), reqwest::StatusCode::OK);
            let body = response.text().await.unwrap();
            let payload = body.lines().filter_map(|line| line.strip_prefix("data:")).find(|data| !data.trim().is_empty()).unwrap_or(&body);
            let initialized: serde_json::Value = serde_json::from_str(payload).unwrap();
            assert_eq!(initialized["result"]["protocolVersion"], "2025-11-25");
            let mut config = StreamableHttpClientTransportConfig::with_uri(url);
            config.auth_header = Some(token);
            let client =
                ().serve(StreamableHttpClientTransport::with_client(http, config))
                    .await
                    .unwrap();
            let tools = client.list_all_tools().await.unwrap();
            assert_eq!(tools.len(), 11);
            assert!(
                tools
                    .iter()
                    .all(|tool| tool.input_schema.get("type") == Some(&json!("object")))
            );
            let result = client
                .call_tool(CallToolRequestParams::new("plotium_get_state"))
                .await
                .unwrap();
            assert_eq!(result.structured_content.as_ref().unwrap()["revision"], 0);
            assert_eq!(result.structured_content.as_ref().unwrap()["permissions"]["allow_reversible_modify"], true);
            assert!(!result.content.is_empty());
            let catalog = client
                .call_tool(CallToolRequestParams::new("plotium_properties_list"))
                .await
                .unwrap();
            assert_eq!(
                catalog.structured_content.unwrap()["properties"]
                    .as_array()
                    .unwrap()
                    .len(),
                5
            );
            client.cancel().await.unwrap();
        });
        tokio::time::timeout(Duration::from_secs(15), async {
            while !client.is_finished() {
                if let Some(request) = connection.next_request() {
                    let result =
                        connection
                            .session
                            .call(&mut app, &request.name, request.arguments);
                    request.response.send(result).unwrap();
                }
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
            client.await.unwrap();
        })
        .await
        .unwrap();
    });
    assert_eq!(app.doc.automation_runs.len(), 1);
    assert_eq!(
        app.doc.automation_runs[0].caller,
        plotium_core::automation::CallerType::Agent
    );
    connection.stop();
}

#[test]
fn stop_revokes_token_rejects_queue_and_restart_changes_credentials() {
    let mut first = Connection::start(|| {}).unwrap();
    let old_token = first.token.clone();
    first.session.allow_modify = false;
    let (response, receive) = oneshot::channel();
    let (send, requests) = mpsc::channel(1);
    first.requests = requests;
    send.try_send(PendingRequest {
        name: "plotium_get_state".into(),
        arguments: json!({}),
        response,
        cancellation: CancellationToken::new(),
    })
    .unwrap();
    first.stop();
    assert_eq!(
        receive.blocking_recv().unwrap().unwrap_err().code,
        "connection_stopped"
    );
    assert!(!first.active.load(Ordering::Acquire));
    assert!(!first.session.allow_modify);
    let second = Connection::start(|| {}).unwrap();
    assert!(second.session.allow_modify);
    assert_ne!(old_token, second.token);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let response = reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .post(&second.url)
            .bearer_auth(old_token)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    });
}

#[test]
fn external_client_finds_inspects_plans_and_modifies_a_line_with_undo() {
    let (mut app, targets) = crate::component_tests::fixture();
    let context = app.session.project_context;
    app.session.project_present = true;
    let resource_id = targets[0].resource.id.clone();
    let original = crate::component_tests::values(&app, &targets);
    let before = app.doc.automation_revision;
    let mut connection = Connection::start(|| {}).unwrap();
    let mut config = StreamableHttpClientTransportConfig::with_uri(connection.url.clone());
    config.auth_header = Some(connection.token.clone());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let client = tokio::spawn(async move {
            let http = reqwest::Client::builder().no_proxy().build().unwrap();
            let client = ().serve(StreamableHttpClientTransport::with_client(http, config)).await.unwrap();
            let state = client.call_tool(CallToolRequestParams::new("plotium_get_state")).await.unwrap();
            let revision = state.structured_content.unwrap()["revision"].clone();
            let root = client.call_tool(CallToolRequestParams::new("plotium_document_inspect").with_arguments(
                json!({"expected_project_context":context,"expected_revision":revision,"target":plotium_core::automation::document_root_target(),"view":"children"}).as_object().unwrap().clone()
            )).await.unwrap();
            assert_eq!(root.is_error,Some(false));
            assert_eq!(root.structured_content.unwrap()["value"]["total"],2);
            let search = client.call_tool(CallToolRequestParams::new("plotium_resources_search").with_arguments(
                json!({"expected_project_context":context,"expected_revision": revision, "query": {"kinds":["plotium.canvas.object"], "name_contains":"Plot"}}).as_object().unwrap().clone()
            )).await.unwrap();
            let found = search.structured_content.unwrap();
            let id = found["value"]["targets"].as_array().unwrap().iter()
                .find(|target| target["id"] == resource_id).unwrap()["id"].clone();
            assert!(id.is_string());
            let inspect = client.call_tool(CallToolRequestParams::new("plotium_properties_inspect").with_arguments(
                json!({"expected_project_context":context,"expected_revision": revision, "ids":[id],"key":"series.line.stroke_width"}).as_object().unwrap().clone()
            )).await.unwrap();
            assert_eq!(inspect.is_error, Some(false));
            let target = inspect.structured_content.unwrap()["value"]["readings"][0]["target"].clone();
            let structure = client.call_tool(CallToolRequestParams::new("plotium_document_inspect").with_arguments(
                json!({"expected_project_context":context,"expected_revision":revision,"target":target,"view":"self"}).as_object().unwrap().clone()
            )).await.unwrap();
            assert_eq!(structure.is_error,Some(false));
            assert_eq!(structure.structured_content.unwrap()["value"]["node"]["source"]["status"],"resolved");
            let plan = client.call_tool(CallToolRequestParams::new("plotium_plan").with_arguments(
                json!({"expected_project_context":context,"expected_revision": revision, "targets":[target],"tool_id":"properties.set","parameters":{"key":"series.line.stroke_width","value":1.5}}).as_object().unwrap().clone()
            )).await.unwrap();
            let plan_id = plan.structured_content.unwrap()["plan_id"].clone();
            let result = client.call_tool(CallToolRequestParams::new("plotium_execute").with_arguments(
                json!({"plan_id":plan_id}).as_object().unwrap().clone()
            )).await.unwrap();
            assert_eq!(result.structured_content.unwrap()["targets"][0]["outcome"], "succeeded");
            let stale = client.call_tool(CallToolRequestParams::new("plotium_document_inspect").with_arguments(
                json!({"expected_project_context":context,"expected_revision":revision,"target":target,"view":"self"}).as_object().unwrap().clone()
            )).await.unwrap();
            assert_eq!(stale.is_error,Some(true));
            assert_eq!(stale.structured_content.unwrap()["error"]["code"],"stale_revision");
            client.cancel().await.unwrap();
        });
        tokio::time::timeout(Duration::from_secs(15), async {
            while !client.is_finished() {
                if let Some(request) = connection.next_request() {
                    let result = connection.session.call(&mut app, &request.name, request.arguments);
                    request.response.send(result).unwrap();
                }
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
            client.await.unwrap();
        }).await.unwrap();
    });
    assert!(app.doc.automation_revision > before);
    let changed = crate::component_tests::values(&app, &targets);
    assert_ne!(changed[0], original[0]);
    assert_eq!(changed[1..], original[1..]);
    assert!(app.can_undo());
    app.undo();
    assert_eq!(crate::component_tests::values(&app, &targets), original);
    assert!(app.can_redo());
    app.redo();
    assert_eq!(crate::component_tests::values(&app, &targets), changed);
}

#[test]
fn cancelling_a_queued_execute_closes_its_desktop_response() {
    let mut connection = Connection::start(|| {}).unwrap();
    let mut config = StreamableHttpClientTransportConfig::with_uri(connection.url.clone());
    config.auth_header = Some(connection.token.clone());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(10), async {
            let http = reqwest::Client::builder().no_proxy().build().unwrap();
            let client =
                ().serve(StreamableHttpClientTransport::with_client(http, config))
                    .await
                    .unwrap();
            let handle = client
                .send_cancellable_request(
                    rmcp::model::ClientRequest::CallToolRequest(rmcp::model::Request::new(
                        CallToolRequestParams::new("plotium_execute").with_arguments(
                            json!({"plan_id":"queued-plan"})
                                .as_object()
                                .unwrap()
                                .clone(),
                        ),
                    )),
                    rmcp::service::PeerRequestOptions::no_options(),
                )
                .await
                .unwrap();
            // Keep the desktop idle until the real protocol cancellation arrives.
            let mut pending = connection.requests.recv().await.unwrap();
            assert!(!pending.is_cancelled());
            handle.cancel(Some("User cancelled".into())).await.unwrap();
            pending.response.closed().await;
            assert!(pending.cancellation.is_cancelled());
            assert!(pending.is_cancelled());
            client.cancel().await.unwrap();
        })
        .await
        .unwrap();
    });
}

#[test]
fn cancelled_queued_edit_is_skipped_before_desktop_execution() {
    let mut connection = Connection::start(|| {}).unwrap();
    let mut app = plotium_core::state::PlotiumApp::new_with_settings(Default::default());
    app.start_new_project();
    let context = app.session.project_context;
    connection.session.allow_modify = true;
    let plan = connection.session.call(&mut app, "plotium_plan", json!({
        "tool_id":"properties.set", "parameters":{"key":"document.figure.typography.tick_pt","value":15},
        "ids":["document"], "expected_project_context":context,"expected_revision":0,
    })).unwrap();
    let runs = app.doc.automation_runs.len();
    let (send, requests) = mpsc::channel(1);
    connection.requests = requests;
    let cancellation = CancellationToken::new();
    let (response, _receive) = oneshot::channel();
    send.try_send(PendingRequest {
        name: "plotium_execute".into(),
        arguments: json!({"plan_id":plan["plan_id"]}),
        response,
        cancellation: cancellation.clone(),
    })
    .unwrap();
    cancellation.cancel();
    // No HTTP task needs to be scheduled for the desktop to observe cancellation.
    while let Some(request) = connection.next_request() {
        let result = connection
            .session
            .call(&mut app, &request.name, request.arguments);
        request.response.send(result).unwrap();
    }
    assert_eq!(app.doc.automation_revision, 0);
    assert_eq!(app.doc.automation_runs.len(), runs);
    assert!(!app.can_undo());
}
