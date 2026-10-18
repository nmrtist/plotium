use super::*;
use crate::ui::commands::{self, CommandId};
use plotium_core::state::ProjectTransition;

#[test]
fn connection_settings_only_open_from_explicit_catalog_command() {
    let mut ui = AgentConnections::default();
    let mut app = PlotiumApp::new();
    let ctx = egui::Context::default();
    let descriptor = commands::describe(&app, CommandId::AgentConnections);
    assert!(descriptor.enabled);
    assert_eq!(descriptor.label, "Agent connections…");
    assert_eq!(
        CommandId::AgentConnections.stable_id(),
        "file.agent_connections"
    );
    ui.error = Some("Background failure".into());
    let _ = ctx.run_ui(egui::RawInput::default(), |root| {
        ui.poll(&mut app, root.ctx(), false)
    });
    assert!(app.session.ui.settings_dialog.is_none());
    commands::execute_without_clipboard(CommandId::AgentConnections, &mut app, &ctx);
    let _ = ctx.run_ui(egui::RawInput::default(), |root| {
        ui.poll(&mut app, root.ctx(), false)
    });
    assert_eq!(
        app.session.ui.settings_dialog.as_ref().unwrap().category,
        plotium_core::state::SettingsCategory::AgentConnections
    );
    assert!(ui.connection.is_none());
}

#[test]
fn project_transition_stops_connection_and_revokes_session_plans() {
    let mut ui = AgentConnections::default();
    let mut app = PlotiumApp::new();
    let ctx = egui::Context::default();
    let mut connection = Connection::start(|| {}).unwrap();
    let plan = connection.session.call(&mut app, "plotium_plan", serde_json::json!({
        "tool_id":"properties.set","parameters":{"key":"document.figure.typography.tick_pt","value":15},
        "ids":["document"], "expected_revision":0,
    })).unwrap();
    let old_plan = plan["plan_id"].clone();
    ui.connection = Some(connection);
    app.request_project_transition(ProjectTransition::New);
    let _ = ctx.run_ui(egui::RawInput::default(), |root| {
        ui.poll(&mut app, root.ctx(), false)
    });
    assert!(ui.connection.is_none());
    assert!(app.session.ui.settings_dialog.is_none());
    app.start_new_project();
    let mut connection = Connection::start(|| {}).unwrap();
    connection.session.allow_modify = true;
    let failure = connection
        .session
        .call(
            &mut app,
            "plotium_execute",
            serde_json::json!({"plan_id":old_plan}),
        )
        .unwrap_err();
    assert_eq!(failure.code, "expired_plan");
    assert_eq!(app.doc.automation_revision, 0);
}

#[test]
fn closing_preferences_keeps_the_connection_and_permission() {
    let mut ui = AgentConnections::default();
    let mut app = PlotiumApp::new();
    let ctx = egui::Context::default();
    let mut connection = Connection::start(|| {}).unwrap();
    connection.session.allow_modify = true;
    let info = connection.connection_info();
    ui.connection = Some(connection);
    commands::execute_without_clipboard(CommandId::AgentConnections, &mut app, &ctx);
    app.session.ui.settings_dialog = None;
    ui.poll(&mut app, &ctx, false);
    assert!(app.session.ui.settings_dialog.is_none());
    let connection = ui.connection.as_ref().unwrap();
    assert_eq!(connection.connection_info(), info);
    assert!(connection.session.allow_modify);
}
