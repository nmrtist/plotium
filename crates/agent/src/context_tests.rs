use super::*;
use plotium_core::state::{PlotiumApp, ProjectTransition};
use serde_json::json;
fn project() -> PlotiumApp {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    app.start_new_project();
    app
}
#[test]
fn equal_revisions_never_authorize_another_project_and_old_plans_stay_revoked() {
    let mut app = project();
    let mut session = Session::default();
    session.allow_modify = true;
    let context = app.session.project_context;
    let query = json!({"expected_project_context":context,"expected_revision":0,"query":{}});
    let p = session.call(&mut app, "plotium_plan", json!({"expected_project_context":context,"expected_revision":0,"ids":["document"],"tool_id":"properties.set","parameters":{"key":"document.figure.typography.tick_pt","value":15}})).unwrap();
    app.start_new_project();
    assert_ne!(app.session.project_context, context);
    assert_eq!(app.doc.automation_revision, 0);
    assert_eq!(
        session
            .call(&mut app, "plotium_resources_search", query)
            .unwrap_err()
            .code,
        "project_changed"
    );
    // Reading the new state must not reauthorize a plan from an old project.
    session
        .call(&mut app, "plotium_get_state", json!({}))
        .unwrap();
    assert_eq!(
        session
            .call(&mut app, "plotium_execute", json!({"plan_id":p["plan_id"]}))
            .unwrap_err()
            .code,
        "project_changed"
    );
    assert_eq!(app.doc.automation_revision, 0);
}
#[test]
fn missing_project_and_transition_cancellation_have_explicit_results() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let mut session = Session::default();
    assert_eq!(
        session
            .call(&mut app, "plotium_get_state", json!({}))
            .unwrap_err()
            .code,
        "no_project"
    );
    app.start_new_project();
    let context = app.session.project_context;
    app.request_project_transition(ProjectTransition::Close);
    assert_eq!(
        session
            .call(&mut app, "plotium_get_state", json!({}))
            .unwrap_err()
            .code,
        "busy"
    );
    app.session.ui.project_transition = None;
    let state = session
        .call(&mut app, "plotium_get_state", json!({}))
        .unwrap();
    assert_eq!(state["project_context"], json!(context));
    assert_eq!(
        session
            .call(
                &mut app,
                "plotium_resources_search",
                json!({"expected_revision":0,"query":{}})
            )
            .unwrap_err()
            .code,
        "invalid_parameters"
    );
    app.close_project();
    assert_ne!(app.session.project_context, context);
}
#[test]
fn queue_and_background_snapshot_cannot_complete_into_replacement_project() {
    let mut app = project();
    app.doc
        .canvases
        .push(plotium_core::state::CanvasDocument::new(
            "test".into(),
            [100., 80.],
        ));
    let context = app.session.project_context;
    let mut connection = Connection::start(|| {}).unwrap();
    let target =
        plotium_core::automation::TargetRef::resource(app.doc.canvases[0].resource_id.into());
    let (response, mut receive) = oneshot::channel();
    let cancellation = CancellationToken::new();
    connection.dispatch_request(&mut app, PendingRequest {
        name:"plotium_render_preview".into(),
        arguments: json!({"expected_project_context":context,"expected_revision":0,"target":target,"max_edge":320}),
        response, cancellation: cancellation.clone(),
    }, false).unwrap();
    let (send, requests) = mpsc::channel(2);
    connection.requests = requests;
    let (response, mut queued) = oneshot::channel();
    send.try_send(PendingRequest {
        name: "plotium_get_state".into(),
        arguments: json!({}),
        response,
        cancellation: CancellationToken::new(),
    })
    .unwrap();
    app.start_new_project();
    connection.poll_preview(&mut app).unwrap();
    assert!(!cancellation.is_cancelled());
    assert_eq!(
        receive.try_recv().unwrap().unwrap_err().code,
        "project_changed"
    );
    assert_eq!(
        queued.try_recv().unwrap().unwrap_err().code,
        "project_changed"
    );
    assert!(app.doc.automation_runs.is_empty());
    assert!(!app.doc.dirty);
    assert!(connection.is_running());
}

#[test]
fn project_switch_returns_project_changed_over_http_for_background_requests() {
    use rmcp::{
        ServiceExt,
        model::CallToolRequestParams,
        transport::{
            StreamableHttpClientTransport,
            streamable_http_client::StreamableHttpClientTransportConfig,
        },
    };
    use std::time::Duration;

    let file = tempfile::NamedTempFile::with_suffix(".csv").unwrap();
    std::fs::write(file.path(), "x,y\n1,2\n").unwrap();
    for tool in ["plotium_plan", "plotium_render_preview"] {
        let mut app = project();
        let canvas = plotium_core::state::CanvasDocument::new("test".into(), [100., 80.]);
        let target = plotium_core::automation::TargetRef::resource(canvas.resource_id.into());
        app.doc.canvases.push(canvas);
        let context = app.session.project_context;
        let arguments = if tool == "plotium_plan" {
            json!({"expected_project_context":context,"expected_revision":0,"tool_id":"data.import",
                "parameters":{"mode":"table_data_only","paths":[file.path()],"header":"present"}})
        } else {
            json!({"expected_project_context":context,"expected_revision":0,"target":target,"max_edge":320})
        };
        let mut connection = Connection::start(|| {}).unwrap();
        let mut config = StreamableHttpClientTransportConfig::with_uri(connection.url.clone());
        config.auth_header = Some(connection.token.clone());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(15), async {
                let http = reqwest::Client::builder().no_proxy().build().unwrap();
                let mut client =
                    ().serve(StreamableHttpClientTransport::with_client(http, config))
                        .await
                        .unwrap();
                let call = tokio::spawn(async move {
                    let result = client
                        .call_tool(
                            CallToolRequestParams::new(tool)
                                .with_arguments(arguments.as_object().unwrap().clone()),
                        )
                        .await
                        .unwrap();
                    client
                        .close_with_timeout(Duration::from_millis(250))
                        .await
                        .unwrap();
                    result
                });
                let request = loop {
                    if let Some(request) = connection.next_request() {
                        break request;
                    }
                    tokio::time::sleep(Duration::from_millis(1)).await;
                };
                connection
                    .dispatch_request(&mut app, request, false)
                    .unwrap();
                assert!(connection.import.is_some() || connection.preview.is_some());
                app.start_new_project();
                connection.synchronize_project(&app);
                let result = call.await.unwrap();
                assert_eq!(result.is_error, Some(true));
                assert_eq!(
                    result.structured_content.unwrap()["error"]["code"],
                    "project_changed"
                );
                assert!(connection.import.is_none() && connection.preview.is_none());
                assert!(app.doc.automation_runs.is_empty());
                assert!(!app.doc.dirty);
            })
            .await
            .unwrap();
        });
    }
}
