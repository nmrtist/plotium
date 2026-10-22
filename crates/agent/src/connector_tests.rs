use super::*;
use crate::discovery::Discovery;
use rmcp::{ServiceExt, model::CallToolRequestParams};
use serde_json::json;
use std::time::Duration;

#[test]
fn discovery_failures_explain_missing_permission_and_invalid_data_without_secrets() {
    for (kind, expected) in [
        (std::io::ErrorKind::NotFound, "enable MCP"),
        (std::io::ErrorKind::PermissionDenied, "outside a sandbox"),
        (std::io::ErrorKind::InvalidData, "republish discovery"),
    ] {
        let message = discovery_error(
            std::path::Path::new("private/mcp"),
            &std::io::Error::from(kind),
        );
        assert!(message.contains("service.json"), "{message}");
        assert!(message.contains(expected), "{message}");
    }
}

#[test]
fn one_stdio_session_survives_offline_start_restart_and_revocation_without_replay() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().join("mcp");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let connector = Connector {
            directory: directory.clone(),
            client: reqwest::Client::builder().no_proxy().build().unwrap(),
            identity: "claude-desktop",
            diagnostic: Default::default(),
        };
        let (server_io, client_io) = tokio::io::duplex(1024 * 1024);
        let server = tokio::spawn(connector.clone().serve(server_io));
        let client = ().serve(client_io).await.unwrap();
        let service = server.await.unwrap().unwrap();
        assert_eq!(
            client.list_all_tools().await.unwrap(),
            crate::protocol::definitions()
        );
        let result = client
            .call_tool(CallToolRequestParams::new("plotium_get_state"))
            .await
            .unwrap();
        assert_eq!(
            result.structured_content.unwrap()["error"]["code"],
            "unavailable"
        );
        let mut connection = crate::Connection::start_in(directory.clone(), || {}).unwrap();
        let first = Discovery::read(&directory).unwrap();
        connector.heartbeat().await;
        assert!(connection.activity().last.is_some());
        assert!(connection.activity().desktop_seen);
        assert!(connection.next_request().is_none());
        let mut app = plotium_core::state::PlotiumApp::new_with_settings(Default::default());
        assert!(app.doc.automation_runs.is_empty());
        assert!(!app.doc.dirty);
        app.start_new_project();
        let context = app.session.project_context;
        let handle = client.peer().clone();
        let call = tokio::spawn(async move {
            handle
                .call_tool(CallToolRequestParams::new("plotium_get_state"))
                .await
                .unwrap()
        });
        pump(&mut connection, &mut app, &call).await;
        let state = call.await.unwrap().structured_content.unwrap();
        assert_eq!(state["project_context"], json!(context));
        assert!(state.get("connection_capabilities").is_none());
        let handle = client.peer().clone();
        let call = tokio::spawn(async move {
            handle
                .call_tool(CallToolRequestParams::new("plotium_capabilities_get"))
                .await
                .unwrap()
        });
        pump(&mut connection, &mut app, &call).await;
        let discovery = call.await.unwrap().structured_content.unwrap();
        assert_eq!(
            discovery["capability_fingerprint"],
            state["capability_fingerprint"]
        );
        assert_eq!(
            discovery["connection_capabilities"],
            *crate::capabilities::manifest()
        );
        let handle = client.peer().clone();
        let call = tokio::spawn(async move {
            handle
                .call_tool(
                    CallToolRequestParams::new("plotium_save_status")
                        .with_arguments(json!({"save_id":"missing"}).as_object().unwrap().clone()),
                )
                .await
                .unwrap()
        });
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Some(request) = connection.next_request() {
                    assert_eq!(
                        connection
                            .dispatch_request(&mut app, request, false)
                            .unwrap_err()
                            .code,
                        "unknown_save"
                    );
                    break;
                }
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await
        .unwrap();
        let error = call.await.unwrap().structured_content.unwrap();
        assert_eq!(error["error"]["code"], "unknown_save");
        assert_eq!(
            error["error"]["recovery"]["receipt"]["arguments"]["save_id"],
            "missing"
        );
        assert_eq!(error["error"]["recovery"]["receipt"]["retained"], false);
        assert!(error["error"]["recovery"]["committed"].is_null());
        connection.stop();
        assert!(Discovery::read(&directory).is_err());
        let result = client
            .call_tool(CallToolRequestParams::new("plotium_get_state"))
            .await
            .unwrap();
        assert_eq!(
            result.structured_content.unwrap()["error"]["code"],
            "unavailable"
        );
        let mut connection = crate::Connection::start_in(directory.clone(), || {}).unwrap();
        let second = Discovery::read(&directory).unwrap();
        assert_ne!(first.instance_id, second.instance_id);
        assert_ne!(first.token, second.token);
        assert_ne!(first.url, second.url);
        app.start_new_project();
        let context = app.session.project_context;
        let handle = client.peer().clone();
        let call = tokio::spawn(async move {
            handle
                .call_tool(CallToolRequestParams::new("plotium_get_state"))
                .await
                .unwrap()
        });
        pump(&mut connection, &mut app, &call).await;
        assert_eq!(
            call.await.unwrap().structured_content.unwrap()["project_context"],
            json!(context)
        );
        // Lose a submitted call before the desktop consumes it. Reconnect must
        // not enqueue that call on the new instance, even for a read.
        let handle = client.peer().clone();
        let call = tokio::spawn(async move {
            handle
                .call_tool(CallToolRequestParams::new("plotium_get_state"))
                .await
                .unwrap()
        });
        let pending = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Some(pending) = connection.next_request() {
                    break pending;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        connection.stop();
        drop(pending);
        let result = tokio::time::timeout(Duration::from_secs(5), call)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.is_error, Some(true));
        let error = result.structured_content.unwrap();
        assert!(error["error"]["recovery"]["committed"].is_null());
        assert!(error["error"]["recovery"]["plan_consumed"].is_null());
        assert_eq!(error["error"]["recovery"]["submission"], "unknown");
        assert_eq!(error["error"]["recovery"]["replay_allowed"], false);
        let mut connection = crate::Connection::start_in(directory.clone(), || {}).unwrap();
        connector.heartbeat().await;
        assert!(connection.next_request().is_none());
        assert!(app.doc.automation_runs.is_empty());
        client.cancel().await.unwrap();
        service.cancel().await.unwrap();
    });
}
async fn pump<T>(
    connection: &mut crate::Connection,
    app: &mut plotium_core::state::PlotiumApp,
    task: &tokio::task::JoinHandle<T>,
) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !task.is_finished() {
            if let Some(request) = connection.next_request() {
                connection.dispatch_request(app, request, false).unwrap();
            }
            let _ = connection.poll_diagnostic(app, false);
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
}

#[test]
fn native_stdio_facade_keeps_diagnostic_pages_until_stdio_cleanup() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().join("mcp");
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let connector=Connector {directory: directory.clone(),client:reqwest::Client::builder().no_proxy().build().unwrap(),identity:"stdio",diagnostic:Default::default()};
        let transport=connector.diagnostic.clone();
        let mut connection=crate::Connection::start_in(directory,||{}).unwrap();
        connection.session.allow_modify=false;
        let (mut app,_,_)=crate::guide_fixture::fixture();
        let (server_io,client_io)=tokio::io::duplex(1024*1024);
        let server=tokio::spawn(connector.serve(server_io));
        let client=().serve(client_io).await.unwrap();
        let service=server.await.unwrap().unwrap();
        let mut args=json!({"expected_project_context":app.session.project_context,"expected_revision":app.doc.automation_revision,"canvas":plotium_core::automation::TargetRef::resource(app.doc.canvases[0].resource_id.into()),"output":{"size_mm":app.doc.canvases[0].size_mm},"view":"both","limit":20});
        let initial=args.clone();
        let mut pages=0;
        loop {
            let peer=client.peer().clone();
            let call=tokio::spawn(async move {peer.call_tool(CallToolRequestParams::new("plotium_figure_diagnose").with_arguments(args.as_object().unwrap().clone())).await.unwrap()});
            pump(&mut connection,&mut app,&call).await;
            let result=call.await.unwrap();
            assert_ne!(result.is_error,Some(true),"{:?}",result.structured_content);
            let value=result.structured_content.unwrap()["value"].clone();
            pages+=1;
            if value["continuation"].is_null() { assert_eq!(value["coverage"]["scan_complete"],true);break; }
            args=json!({"expected_project_context":app.session.project_context,"expected_revision":app.doc.automation_revision,"cursor":value["continuation"]});
        }
        assert!(pages>1);
        let peer=client.peer().clone();
        let call=tokio::spawn(async move {peer.call_tool(CallToolRequestParams::new("plotium_figure_diagnose").with_arguments(initial.as_object().unwrap().clone())).await.unwrap()});
        pump(&mut connection,&mut app,&call).await;
        assert_ne!(call.await.unwrap().is_error,Some(true));
        assert!(connection.session.diagnostic.is_some());
        client.cancel().await.unwrap();
        service.waiting().await.unwrap();
        // run_stdio performs this same cleanup before its runtime exits.
        transport.close().await;
        tokio::time::timeout(Duration::from_secs(5),async {
            while connection.session.diagnostic.is_some() {
                connection.poll_diagnostic(&mut app,false).unwrap();
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }).await.unwrap();
    });
}

/// The desktop hop negotiates an older revision whose results omit
/// `resultType`; a 2026-07-28 stdio peer still requires it on the wire.
#[test]
fn stdio_facade_restores_result_type_for_current_revision_peers() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().join("mcp");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let connector = Connector {
            directory: directory.clone(),
            client: reqwest::Client::builder().no_proxy().build().unwrap(),
            identity: "claude-code",
            diagnostic: Default::default(),
        };
        let mut connection = crate::Connection::start_in(directory, || {}).unwrap();
        let mut app = plotium_core::state::PlotiumApp::new_with_settings(Default::default());
        app.start_new_project();
        let (server_io, client_io) = tokio::io::duplex(1024 * 1024);
        let server = tokio::spawn(connector.serve(server_io));
        let (read, mut write) = tokio::io::split(client_io);
        // Stateless requests select the 2026-07-28 lifecycle only with complete
        // inline client metadata, as Claude Code 2.1.302 sends it.
        let meta = json!({
            "io.modelcontextprotocol/protocolVersion": "2026-07-28",
            "io.modelcontextprotocol/clientInfo": {"name": "probe", "version": "0"},
            "io.modelcontextprotocol/clientCapabilities": {}
        });
        let call = tokio::spawn(async move {
            let mut lines = BufReader::new(read).lines();
            let mut send = async |request: serde_json::Value| {
                write.write_all(format!("{request}\n").as_bytes()).await.unwrap();
                let line = lines.next_line().await.unwrap().unwrap();
                serde_json::from_str::<serde_json::Value>(&line).unwrap()
            };
            let discover = send(json!({"jsonrpc":"2.0","id":1,"method":"server/discover","params":{"_meta":meta}})).await;
            assert!(
                discover["result"]["supportedVersions"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("2026-07-28"))
            );
            send(json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"plotium_get_state","arguments":{},"_meta":meta}})).await
        });
        pump(&mut connection, &mut app, &call).await;
        let response = call.await.unwrap();
        assert_eq!(response["result"]["resultType"], "complete", "{response}");
        assert_eq!(
            response["result"]["structuredContent"]["project_context"],
            json!(app.session.project_context)
        );
        server.abort();
    });
}
