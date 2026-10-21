use super::*;

fn save_plan(session: &mut Session, app: &mut PlotiumApp, path: &std::path::Path) -> Value {
    session
        .call(
            app,
            "plotium_plan",
            json!({
                "expected_project_context":app.session.project_context,
                "expected_revision":app.doc.automation_revision,
                "tool_id":"project.save","parameters":{"path":path}
            }),
        )
        .unwrap()
}

#[test]
fn save_denial_and_revocation_have_terminal_receipts_without_replay() {
    let dir = tempfile::tempdir().unwrap();
    for revoke in [false, true] {
        let mut app = PlotiumApp::new_with_settings(Default::default());
        let mut session = Session::default();
        let path = dir.path().join("consent.plotium");
        let plan = save_plan(&mut session, &mut app, &path);
        let result = session
            .call(
                &mut app,
                "plotium_execute",
                json!({"plan_id":plan["plan_id"]}),
            )
            .unwrap();
        let recovery = &result["save"]["recovery"];
        assert_eq!(recovery["completion"], "pending");
        assert_eq!(recovery["submission"], "submitted");
        assert_eq!(recovery["plan_consumed"], true);
        assert_eq!(recovery["next_action"]["kind"], "desktop_file_confirmation");
        let id = plan["save_id"].as_str().unwrap();
        if revoke {
            session.revoke_saves();
        } else {
            app.decide_external_save(id, false, false);
        }
        let result = session
            .call(&mut app, "plotium_save_status", json!({"save_id":id}))
            .unwrap();
        assert_eq!(result["save"]["recovery"]["completion"], "failed");
        assert_eq!(result["save"]["recovery"]["committed"], false);
        assert_eq!(
            result["save"]["recovery"]["receipt"]["arguments"]["save_id"],
            id
        );
        assert_eq!(result["save"]["recovery"]["replay_allowed"], false);
        let expired = session
            .call(&mut app, "plotium_execute", json!({"plan_id":id}))
            .unwrap_err()
            .value();
        assert_eq!(expired["error"]["code"], "expired_plan");
        assert_eq!(
            expired["error"]["recovery"]["receipt"]["arguments"]["save_id"],
            id
        );
        assert_eq!(expired["error"]["recovery"]["plan_consumed"], true);
        assert_eq!(expired["error"]["recovery"]["committed"], false);
        assert!(!path.exists());
    }
}

#[test]
fn full_desktop_queue_reports_non_submission_over_the_protocol() {
    use rmcp::{ServiceExt, model::CallToolRequestParams};
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let (send, _receive) = tokio::sync::mpsc::channel(1);
            let (response, _response) = tokio::sync::oneshot::channel();
            send.try_send(crate::PendingRequest {
                name: "plotium_get_state".into(),
                arguments: json!({}),
                response,
                cancellation: Default::default(),
                transport: None,
            })
            .unwrap();
            let handler = crate::protocol::Handler {
                send,
                cancel: Default::default(),
                wake: std::sync::Arc::new(|| {}),
            };
            let (server_io, client_io) = tokio::io::duplex(64 * 1024);
            let server = tokio::spawn(handler.serve(server_io));
            let client = ().serve(client_io).await.unwrap();
            let service = server.await.unwrap().unwrap();
            let result = client
                .call_tool(
                    CallToolRequestParams::new("plotium_execute").with_arguments(
                        json!({"plan_id":"queued-plan"})
                            .as_object()
                            .unwrap()
                            .clone(),
                    ),
                )
                .await
                .unwrap();
            let error = result.structured_content.unwrap();
            assert_eq!(error["error"]["code"], "busy");
            assert_eq!(error["error"]["retryable"], false);
            assert_eq!(error["error"]["recovery"]["plan_consumed"], false);
            assert_eq!(error["error"]["recovery"]["submission"], "not_submitted");
            assert_eq!(error["error"]["recovery"]["committed"], false);
            assert_eq!(error["error"]["recovery"]["plan_id"], "queued-plan");
            client.cancel().await.unwrap();
            service.cancel().await.unwrap();
        });
}

#[test]
fn execution_gates_consume_plans_but_planning_busy_does_not() {
    let dir = tempfile::tempdir().unwrap();
    for gate in ["busy", "stale_revision", "project_changed"] {
        let mut app = PlotiumApp::new_with_settings(Default::default());
        let mut session = Session::default();
        let plan = save_plan(&mut session, &mut app, &dir.path().join("save.plotium"));
        if gate == "stale_revision" {
            app.doc.automation_revision += 1;
        }
        if gate == "project_changed" {
            app.start_new_project();
        }
        let error = session
            .call_when_available(
                &mut app,
                "plotium_execute",
                json!({"plan_id":plan["plan_id"]}),
                gate == "busy",
            )
            .unwrap_err()
            .value();
        assert_eq!(error["error"]["code"], gate);
        assert_eq!(error["error"]["retryable"], false);
        let recovery = &error["error"]["recovery"];
        assert_eq!(recovery["replay_allowed"], false);
        if gate != "project_changed" {
            assert_eq!(recovery["plan_consumed"], true);
            assert_eq!(recovery["submission"], "not_submitted");
            assert_eq!(recovery["committed"], false);
        }
        assert!(
            app.session
                .external_saves
                .receipt(plan["plan_id"].as_str().unwrap())
                .is_none()
        );
    }
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let mut session = Session::default();
    let error = session
        .call_when_available(&mut app, "plotium_plan", json!({}), true)
        .unwrap_err()
        .value();
    assert_eq!(error["error"]["recovery"]["plan_consumed"], false);
}

#[test]
fn unknown_receipts_and_lost_responses_preserve_uncertainty() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let mut session = Session::default();
    for (tool, key, id) in [
        ("plotium_save_status", "save_id", "missing"),
        ("plotium_export_status", "export_id", "missing"),
        (
            "plotium_tasks_get",
            "task_id",
            "00000000-0000-0000-0000-000000000000",
        ),
    ] {
        let error = session
            .call(&mut app, tool, json!({key:id}))
            .unwrap_err()
            .value();
        assert!(error["error"]["recovery"]["committed"].is_null());
        assert_eq!(error["error"]["recovery"]["completion"], "unknown");
        assert_eq!(error["error"]["recovery"]["receipt"]["arguments"][key], id);
    }
    let error = Failure::new("connection_lost", "Disconnected")
        .for_request("plotium_execute", &json!({"plan_id":"saved-id"}))
        .value();
    assert!(error["error"]["recovery"]["plan_consumed"].is_null());
    assert_eq!(error["error"]["recovery"]["submission"], "unknown");
    assert!(error["error"]["recovery"]["committed"].is_null());
    assert_eq!(error["error"]["recovery"]["plan_id"], "saved-id");
}

#[test]
fn unconfirmed_save_and_partial_export_are_not_success() {
    let mut value =
        json!({"save":{"save_id":"s","status":"completed","phase":"installed_unconfirmed"}});
    annotate(&mut value);
    assert!(value["save"]["recovery"]["committed"].is_null());
    assert_eq!(value["save"]["recovery"]["completion"], "unknown");
    for (statuses, expected) in [
        (vec!["succeeded", "failed"], "partial"),
        (vec!["succeeded", "succeeded"], "succeeded"),
        (vec!["failed", "failed"], "failed"),
        (vec!["pending"], "unknown"),
    ] {
        let mut value = json!({"export":{"export_id":"e","status":"completed",
            "files":statuses.iter().map(|s|json!({"status":s})).collect::<Vec<_>>()}});
        annotate(&mut value);
        assert_eq!(value["export"]["recovery"]["completion"], expected);
        assert_eq!(value["export"]["recovery"]["replay_allowed"], false);
    }
}
