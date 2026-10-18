use super::*;
use crate::ui::commands::{self, CommandId};
use plotium_core::state::ProjectTransition;

#[test]
fn connection_settings_only_open_from_explicit_catalog_command() {
    let mut connections = AgentConnections::default();
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let ctx = egui::Context::default();
    let descriptor = commands::describe(&app, CommandId::AgentConnections);
    assert!(descriptor.enabled);
    assert_eq!(descriptor.label, "AI connections (MCP)…");
    assert_eq!(
        CommandId::AgentConnections.stable_id(),
        "file.agent_connections"
    );
    connections.error = Some("Background tool failure".into());
    connections.poll(&mut app, &ctx, false);
    assert!(app.session.ui.settings_dialog.is_none());
    commands::execute_without_clipboard(CommandId::AgentConnections, &mut app, &ctx);
    connections.poll(&mut app, &ctx, false);
    assert_eq!(
        app.session.ui.settings_dialog.as_ref().unwrap().category,
        plotium_core::state::SettingsCategory::AgentConnections
    );
    assert!(connections.connection.is_none());
}
#[test]
fn switching_projects_keeps_transport_and_access_but_revokes_plans() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    app.start_new_project();
    app.settings.agent.enabled = true;
    app.settings.agent.access = AgentAccess::ReadOnly;
    let mut connections = AgentConnections {
        connection: Some(Connection::start(|| {}).unwrap()),
        connector_checked: true,
        ..Default::default()
    };
    let ctx = egui::Context::default();
    connections.poll(&mut app, &ctx, false);
    let info = connections.connection.as_ref().unwrap().connection_info();
    let context = app.session.project_context;
    let plan = connections.connection.as_mut().unwrap().session.call(&mut app, "plotium_plan", serde_json::json!({
        "expected_project_context":context,"expected_revision":0,"tool_id":"properties.set",
        "parameters":{"key":"document.figure.typography.tick_pt","value":15},"ids":["document"]
    })).unwrap();
    app.request_project_transition(ProjectTransition::New);
    connections.poll(&mut app, &ctx, false);
    assert_eq!(
        connections.connection.as_ref().unwrap().connection_info(),
        info
    );
    app.start_new_project();
    connections.poll(&mut app, &ctx, false);
    let connection = connections.connection.as_mut().unwrap();
    assert_eq!(connection.connection_info(), info);
    assert!(!connection.session.allow_modify);
    let failure = connection
        .session
        .call(
            &mut app,
            "plotium_execute",
            serde_json::json!({"plan_id":plan["plan_id"]}),
        )
        .unwrap_err();
    assert_eq!(failure.code, "project_changed");
    assert!(app.session.ui.settings_dialog.is_none());
    assert_eq!(app.doc.automation_revision, 0);
    app.session.ui.settings_dialog = None;
    connections.poll(&mut app, &ctx, false);
    assert_eq!(
        connections.connection.as_ref().unwrap().connection_info(),
        info
    );
}
#[test]
fn disabling_immediately_revokes_transport_and_cancels_retries() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let mut connections = AgentConnections {
        connection: Some(Connection::start(|| {}).unwrap()),
        retry_at: Some(Instant::now()),
        attempts: 4,
        ..Default::default()
    };
    connections.poll(&mut app, &egui::Context::default(), false);
    assert!(connections.connection.is_none());
    assert!(connections.retry_at.is_none());
    assert_eq!(connections.attempts, 0);
}
#[test]
fn indicator_priority_and_activity_window_use_controlled_time() {
    let now = Instant::now();
    assert_eq!(
        Indicator::resolve(false, true, true, Some(now), now),
        Indicator::Off
    );
    assert_eq!(
        Indicator::resolve(true, true, true, Some(now), now),
        Indicator::Attention
    );
    assert_eq!(
        Indicator::resolve(true, false, true, Some(now), now + Duration::from_secs(29)),
        Indicator::Active
    );
    assert_eq!(
        Indicator::resolve(true, false, true, Some(now), now + Duration::from_secs(30)),
        Indicator::Ready
    );
    assert_eq!(
        Indicator::resolve(true, false, false, Some(now), now),
        Indicator::Starting
    );
    assert_eq!(
        Indicator::resolve(true, false, true, None, now),
        Indicator::Ready
    );
}
