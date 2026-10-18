//! Ephemeral connection controls and desktop-only project access.
use plotium_agent::Connection;
use plotium_core::state::PlotiumApp;

#[derive(Default)]
pub(crate) struct AgentConnections {
    connection: Option<Connection>,
    recent: String,
    error: Option<String>,
}
impl AgentConnections {
    pub fn stop(&mut self) {
        self.connection = None;
    }
    /// Process requests even when eframe skips painting a hidden window.
    pub fn poll(&mut self, app: &mut PlotiumApp, ctx: &egui::Context, blocked: bool) {
        if app.session.ui.project_transition.is_some() {
            if self.connection.is_some() {
                self.recent = "Connection stopped for project transition.".into();
            }
            self.stop();
        }
        let Some(connection) = &mut self.connection else {
            return;
        };
        if let Some(error) = connection.take_error() {
            app.session.status = format!("Agent connection failed: {error}");
            self.error = Some(error);
            self.stop();
            return;
        }
        // Bound work per frame so a connected client cannot starve rendering.
        for _ in 0..8 {
            let Some(request) = connection.next_request() else {
                break;
            };
            if request.is_cancelled() {
                continue;
            }
            let result = connection.session.call_when_available(
                app,
                &request.name,
                request.arguments,
                blocked,
            );
            self.recent = request.name;
            if let Err(error) = &result {
                app.session.status =
                    format!("Agent request failed ({}): {}", error.code, error.message);
                self.error = Some(app.session.status.clone());
            } else {
                self.error = None;
            }
            if request.response.send(result).is_err() {
                self.recent.push_str(" (client disconnected)");
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
    pub(crate) fn show(&mut self, ui: &mut egui::Ui) {
        ui.heading("Agent connections");
        ui.label("Connect an external agent to the current Plotium project.");
        ui.label("Connections and permissions apply only until stopped and are never saved.");
        ui.add_space(8.0);
        if let Some(connection) = &mut self.connection {
            ui.label(format!("Listening: {}", connection.url));
            ui.horizontal_wrapped(|ui| {
                ui.label("Access:");
                ui.radio_value(
                    &mut connection.session.allow_modify,
                    true,
                    "Read and modify",
                );
                ui.radio_value(&mut connection.session.allow_modify, false, "Read only");
            });
            ui.label(
                "Changes can be undone in Plotium. Switch to Read only to prevent further edits.",
            );
            ui.label(
                "Permission applies to all clients holding this connection token until stopped.",
            );
            ui.add_space(8.0);
            if ui.button("Copy Codex launch command").clicked() {
                ui.ctx().copy_text(connection.codex_launch_command());
            }
            ui.label("Paste into a terminal to start a new Codex session. No editing is needed.");
            #[cfg(windows)]
            ui.label("Use PowerShell for the launch command.");
            #[cfg(not(windows))]
            ui.label("Use zsh or bash for the launch command.");
            ui.label("After restarting the connection, copy a new launch command.");
            if ui.button("Copy connection information").clicked() {
                ui.ctx().copy_text(connection.connection_info());
            }
            if ui.button("Stop connection").clicked() {
                self.stop();
            }
        } else {
            ui.label("Stopped · default access: Read and modify");
            ui.label("Start a connection, then copy the Codex launch command. Changes can be undone in Plotium.");
            if ui.button("Start connection").clicked() {
                let wake = ui.ctx().clone();
                match Connection::start(move || wake.request_repaint()) {
                    Ok(connection) => {
                        self.connection = Some(connection);
                        self.error = None;
                    }
                    Err(error) => self.error = Some(format!("Could not start connection: {error}")),
                }
            }
        }
        ui.add_space(8.0);
        ui.label("Agent project reads save run records and mark the project as unsaved.");
        if !self.recent.is_empty() {
            ui.label(format!("Recent operation: {}", self.recent));
        }
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    }
}

#[cfg(test)]
#[path = "agent_connections_tests.rs"]
mod tests;
