use super::*;
impl AgentConnections {
    pub(crate) fn show(&mut self, ui: &mut egui::Ui, app: &mut PlotiumApp) {
        ui.heading("AI connections (MCP)");
        if ui
            .checkbox(&mut app.settings.agent.enabled, "Enable MCP")
            .changed()
        {
            self.save(app);
        }
        ui.label("Starts with Plotium until you turn it off. Exiting Plotium stops access.");
        let project = app
            .doc
            .project_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| {
                if app.session.project_present {
                    "Untitled".into()
                } else {
                    "No project — open a project first".into()
                }
            });
        ui.label(format!("Current project: {project}"));
        let before = app.settings.agent.access;
        ui.horizontal_wrapped(|ui| {
            ui.label("Access:");
            ui.radio_value(
                &mut app.settings.agent.access,
                AgentAccess::ReadAndModify,
                "Read and modify",
            );
            ui.radio_value(
                &mut app.settings.agent.access,
                AgentAccess::ReadOnly,
                "Read only",
            );
        });
        if before != app.settings.agent.access {
            self.save(app);
        }
        ui.small("Remembered for every project. Changes can be undone in Plotium.");
        if !app.settings.agent.enabled {
            ui.label("Enable MCP before adding a client.");
        }
        if !self.inspected {
            self.inspected = true;
            for (i, client) in [Client::Codex, Client::ClaudeCode].into_iter().enumerate() {
                if crate::agent_install::locate(client).is_some() {
                    self.register(i, client, None, true);
                }
            }
        }
        for (index, client) in [Client::Codex, Client::ClaudeCode].into_iter().enumerate() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.strong(client.name());
                    ui.label(if self.cards[index].configured { "Configured" } else if self.cards[index].error.is_some() || self.cards[index].conflict.is_some() { "Needs attention" } else { "Not configured" });
                    let busy = self.cards[index].job.is_some();
                    if ui.add_enabled(app.settings.agent.enabled && !busy, egui::Button::new(if self.cards[index].configured { "Verify" } else { "Add" })).clicked() {
                        self.register(index, client, None, self.cards[index].configured);
                    }
                    if busy { ui.spinner(); }
                });
                ui.small("Configuration and actual client activity are shown separately. Reload client configuration once after the first installation.");
                if let Some(conflict) = self.cards[index].conflict.clone() {
                    ui.label("A different server named plotium exists. Replacing backs up that entry.");
                    ui.monospace(serde_json::to_string_pretty(&conflict).unwrap_or_else(|e| e.to_string()));
                    if let Ok(path) = plotium_agent::discovery::connector_path() { ui.label(format!("Requested command: {}", path.display())); }
                    if ui.add_enabled(self.cards[index].job.is_none(), egui::Button::new("Back up and replace this entry")).clicked() { self.register(index, client, Some(conflict), false); }
                }
                if let Some(error) = &self.cards[index].error { ui.colored_label(ui.visuals().error_fg_color, error); }
                ui.horizontal(|ui| {
                    if ui.add_enabled(self.cards[index].job.is_none(), egui::Button::new("Choose executable…")).clicked()
                        && let Some(path) = rfd::FileDialog::new().pick_file() { self.cards[index].executable = Some(path); self.cards[index].error = None; }
                    ui.hyperlink_to("Installation help", client.help());
                });
            });
        }
        ui.group(|ui| {
            ui.strong("Claude Desktop");
            #[cfg(any(target_os = "macos", windows))] {
                ui.label(if self.desktop_seen { "Connected — connector activity observed" } else { "Not connected — awaiting connector activity" });
                if ui.add_enabled(app.settings.agent.enabled, egui::Button::new("Add to Claude Desktop…")).clicked() {
                    match desktop_bundle().and_then(open_bundle) {
                        Ok(()) => self.recent = "Opened extension package. Confirm installation in Claude Desktop; awaiting connector activity.".into(),
                        Err(error) => self.error = Some(error),
                    }
                }
            }
            #[cfg(not(any(target_os = "macos", windows)))] ui.label("The dedicated Claude Desktop extension is available on macOS and Windows. Use the stdio configuration below for other clients.");
        });
        self.update_indicator(app, ui.ctx(), Instant::now());
        ui.label(indicator(ui.ctx()).state.label());
        if !self.recent.is_empty() {
            ui.label(format!("Recent operation: {}", self.recent));
        }
        let last = self.connection.as_ref().and_then(|c| c.activity().last);
        ui.label(
            last.map(|t| {
                format!(
                    "Authenticated client activity: {} seconds ago",
                    t.elapsed().as_secs()
                )
            })
            .unwrap_or_else(|| "No authenticated client activity".into()),
        );
        for error in [
            &self.save_error,
            &self.fault,
            &self.connector_error,
            &self.error,
        ]
        .into_iter()
        .flatten()
        {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        if (self.save_error.is_some() || self.fault.is_some() || self.connector_error.is_some())
            && ui.button("Retry").clicked()
        {
            self.save(app);
            self.retry_at = None;
            self.connector_checked = false;
        }
        ui.collapsing("Manual configuration and diagnostics", |ui| {
            if let Ok(path) = plotium_agent::discovery::connector_path() {
                let config = plotium_agent::client_config::stdio_config(&path);
                if ui.button("Copy stable stdio configuration").clicked() { ui.ctx().copy_text(config.to_string()); }
                ui.monospace(config.to_string());
            }
            ui.label("Project reads create saved run records. Handshake, status and heartbeat do not.");
            ui.label("Temporary HTTP diagnostics expire whenever the service restarts. Keep tokens private.");
            if let Some(connection) = &self.connection {
                ui.label(format!("Listening: {}", connection.url));
                if ui.button("Copy temporary HTTP diagnostics").clicked() { ui.ctx().copy_text(connection.connection_info()); }
            }
        });
    }
}
#[cfg(any(target_os = "macos", windows))]
fn desktop_bundle() -> Result<std::path::PathBuf, String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let path = desktop_bundle_path(&executable);
    if !path.is_file() {
        return Err("The Claude Desktop extension is missing. Reinstall the complete Plotium release package.".into());
    }
    Ok(path)
}

#[cfg(any(target_os = "macos", windows))]
fn desktop_bundle_path(executable: &std::path::Path) -> std::path::PathBuf {
    #[cfg(target_os = "macos")]
    if let Some(contents) = plotium_core::installation::macos_bundle_contents(executable) {
        return contents.join("Resources/plotium.mcpb");
    }
    executable.with_file_name("plotium.mcpb")
}

#[cfg(all(test, target_os = "macos"))]
#[test]
fn extension_resolves_from_bundle_resources_or_existing_portable_sibling() {
    use std::path::{Path, PathBuf};
    assert_eq!(
        desktop_bundle_path(Path::new("/tmp/Plotium.app/Contents/MacOS/plotium")),
        PathBuf::from("/tmp/Plotium.app/Contents/Resources/plotium.mcpb")
    );
    assert_eq!(
        desktop_bundle_path(Path::new("/tmp/portable/plotium")),
        PathBuf::from("/tmp/portable/plotium.mcpb")
    );
}
#[cfg(any(target_os = "macos", windows))]
fn open_bundle(path: std::path::PathBuf) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(path).spawn();
    #[cfg(windows)]
    let result = std::process::Command::new("explorer.exe").arg(path).spawn();
    result
        .map(|_| ())
        .map_err(|e| format!("Could not open Claude Desktop extension: {e}"))
}
