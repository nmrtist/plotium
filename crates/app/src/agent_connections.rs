//! Application-owned MCP lifetime; preferences are the only desired-state source.
mod view;
use crate::agent_install::{Client, InstallResult};
use plotium_agent::Connection;
use plotium_core::{settings::AgentAccess, state::PlotiumApp};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Indicator {
    Off,
    Starting,
    Ready,
    Active,
    Attention,
}
impl Indicator {
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "AI connection: Off",
            Self::Starting => "AI connection: Starting",
            Self::Ready => "AI connection: Ready",
            Self::Active => "AI connection: Active",
            Self::Attention => "AI connection: Needs attention",
        }
    }
    fn resolve(
        enabled: bool,
        fault: bool,
        ready: bool,
        last: Option<Instant>,
        now: Instant,
    ) -> Self {
        if !enabled {
            Self::Off
        } else if fault {
            Self::Attention
        } else if ready
            && last.is_some_and(|t| now.saturating_duration_since(t) < Duration::from_secs(30))
        {
            Self::Active
        } else if ready {
            Self::Ready
        } else {
            Self::Starting
        }
    }
}
#[derive(Clone)]
pub(crate) struct IndicatorView {
    pub state: Indicator,
    pub tooltip: String,
}
pub(crate) fn indicator(ctx: &egui::Context) -> IndicatorView {
    ctx.data(|d| d.get_temp(egui::Id::new("plotium_ai_indicator")))
        .unwrap_or(IndicatorView {
            state: Indicator::Off,
            tooltip: Indicator::Off.label().into(),
        })
}
#[derive(Default)]
struct ClientCard {
    executable: Option<std::path::PathBuf>,
    configured: bool,
    error: Option<String>,
    conflict: Option<serde_json::Value>,
    job: Option<std::thread::JoinHandle<Result<InstallResult, String>>>,
}
#[derive(Default)]
pub(crate) struct AgentConnections {
    connection: Option<Connection>,
    recent: String,
    error: Option<String>,
    fault: Option<String>,
    save_error: Option<String>,
    retry_at: Option<Instant>,
    attempts: usize,
    connector_job: Option<std::thread::JoinHandle<Result<std::path::PathBuf, String>>>,
    connector_error: Option<String>,
    connector_checked: bool,
    cards: [ClientCard; 2],
    inspected: bool,
    desktop_seen: bool,
}
impl AgentConnections {
    pub fn stop(&mut self) {
        self.connection = None;
        self.retry_at = None;
        self.attempts = 0;
    }
    pub fn poll(&mut self, app: &mut PlotiumApp, ctx: &egui::Context, blocked: bool) {
        self.poll_installation(app);
        app.session.external_saves.reconcile();
        if !app.settings.agent.enabled {
            self.stop();
        } else {
            if !self.connector_checked && self.connector_job.is_none() {
                self.connector_job =
                    Some(std::thread::spawn(crate::agent_install::install_connector));
            }
            if self.connection.is_none() && self.retry_at.is_none_or(|t| Instant::now() >= t) {
                let wake = ctx.clone();
                match Connection::start_published(move || wake.request_repaint()) {
                    Ok(connection) => {
                        self.connection = Some(connection);
                        self.fault = None;
                        self.attempts = 0;
                        self.retry_at = None;
                    }
                    Err(error) => self.failed(app, error.to_string()),
                }
            }
        }
        if let Some(connection) = &self.connection
            && (!connection.is_running()
                || connection
                    .take_error()
                    .map(|e| {
                        self.fault = Some(e);
                    })
                    .is_some())
        {
            let message = self
                .fault
                .clone()
                .unwrap_or_else(|| "MCP service stopped unexpectedly".into());
            self.connection = None;
            self.failed(app, message);
        }
        if let Some(connection) = &mut self.connection {
            connection
                .session
                .set_allow_modify(app.settings.agent.access == AgentAccess::ReadAndModify);
            connection.synchronize_project(app);
            app.session.background_activity.execution_source =
                Some(connection.session.activity_source());
            app.session.result_follow.synchronize(
                app.session.project_context,
                Some(connection.session.activity_source()),
            );
            if let Err(error) = connection.poll_diagnostic(app, blocked) {
                app.session.status =
                    format!("AI request failed ({}): {}", error.code, error.message);
                self.error = Some(app.session.status.clone());
            }
            if let Err(error) = connection.poll_preview_when_available(app, blocked) {
                app.session.status =
                    format!("AI request failed ({}): {}", error.code, error.message);
                self.error = Some(app.session.status.clone());
            }
            if let Err(error) = connection.poll_import(app, blocked) {
                app.session.status =
                    format!("AI request failed ({}): {}", error.code, error.message);
                self.error = Some(app.session.status.clone());
            }
            if let Err(error) = connection.poll_transform(app, blocked) {
                app.session.status =
                    format!("AI request failed ({}): {}", error.code, error.message);
                self.error = Some(app.session.status.clone());
            }
            for _ in 0..8 {
                let Some(request) = connection.next_request() else {
                    break;
                };
                if request.is_cancelled() {
                    continue;
                }
                self.recent = request.name.clone();
                match connection.dispatch_request(app, request, blocked) {
                    Ok(()) => self.error = None,
                    Err(error) => {
                        app.session.status =
                            format!("AI request failed ({}): {}", error.code, error.message);
                        self.error = Some(app.session.status.clone());
                    }
                }
            }
            self.desktop_seen |= connection.activity().desktop_seen;
        }
        app.session.background_activity.execution_source = None;
        let source = self
            .connection
            .as_ref()
            .map(|connection| connection.session.activity_source());
        app.session
            .result_follow
            .synchronize(app.session.project_context, source);
        self.update_indicator(app, ctx, Instant::now());
        if app.settings.agent.enabled
            || self.cards.iter().any(|c| c.job.is_some())
            || self.connector_job.is_some()
        {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
    fn failed(&mut self, app: &mut PlotiumApp, message: String) {
        app.session.status = format!("AI connection needs attention: {message}");
        self.fault = Some(message);
        let seconds = [1, 2, 5, 10, 30][self.attempts.min(4)];
        self.attempts = self.attempts.saturating_add(1);
        self.retry_at = Some(Instant::now() + Duration::from_secs(seconds));
    }
    fn update_indicator(&self, app: &PlotiumApp, ctx: &egui::Context, now: Instant) {
        let last = self.connection.as_ref().and_then(|c| c.activity().last);
        let fault = self
            .save_error
            .as_ref()
            .or(self.fault.as_ref())
            .or(self.connector_error.as_ref());
        let state = Indicator::resolve(
            app.settings.agent.enabled,
            fault.is_some(),
            self.connection.is_some(),
            last,
            now,
        );
        let project = app
            .doc
            .project_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "Untitled".into());
        let permission = if app.settings.agent.access == AgentAccess::ReadOnly {
            "Read only"
        } else {
            "Read and modify"
        };
        let activity = last
            .map(|t| format!("{} seconds ago", now.saturating_duration_since(t).as_secs()))
            .unwrap_or_else(|| "None".into());
        let mut tooltip = format!(
            "{}\nProject: {project}\nAccess: {permission}\nLast activity: {activity}",
            state.label()
        );
        if let Some(error) = fault {
            tooltip.push_str(&format!("\n{error}\nClick to view"));
        }
        ctx.data_mut(|d| {
            d.insert_temp(
                egui::Id::new("plotium_ai_indicator"),
                IndicatorView { state, tooltip },
            )
        });
    }
    fn save(&mut self, app: &mut PlotiumApp) {
        self.save_error = (!app.persist_settings()).then(|| app.session.status.clone());
        if !app.settings.agent.enabled {
            self.stop();
        }
        if let Some(connection) = &mut self.connection {
            connection
                .session
                .set_allow_modify(app.settings.agent.access == AgentAccess::ReadAndModify);
        }
    }
    fn poll_installation(&mut self, app: &mut PlotiumApp) {
        if self.connector_job.as_ref().is_some_and(|j| j.is_finished()) {
            let result = self
                .connector_job
                .take()
                .expect("finished job")
                .join()
                .unwrap_or_else(|_| Err("Connector installation worker failed".into()));
            self.connector_checked = true;
            self.connector_error = result.err();
            if let Some(error) = &self.connector_error {
                app.session.status = error.clone();
            }
        }
        for card in &mut self.cards {
            if card.job.as_ref().is_some_and(|j| j.is_finished()) {
                let result = card
                    .job
                    .take()
                    .expect("finished job")
                    .join()
                    .unwrap_or_else(|_| Err("Client registration worker failed".into()));
                match result {
                    Ok(InstallResult::Configured) => {
                        card.configured = true;
                        card.conflict = None;
                        card.error = None;
                        app.session.status = "AI client configured. Reload its MCP configuration or restart it once if needed.".into();
                    }
                    Ok(InstallResult::Checked(value)) => card.configured = value,
                    Ok(InstallResult::Conflict(entry)) => {
                        card.conflict = Some(entry);
                        card.configured = false;
                    }
                    Err(error) => {
                        card.configured = false;
                        card.error = Some(error.clone());
                        app.session.status = error;
                    }
                }
            }
        }
    }
    fn register(
        &mut self,
        index: usize,
        client: Client,
        replace: Option<serde_json::Value>,
        check: bool,
    ) {
        let card = &mut self.cards[index];
        let Some(executable) = card
            .executable
            .clone()
            .or_else(|| crate::agent_install::locate(client))
        else {
            card.error = Some(format!(
                "{} was not found. Install it or choose its executable below.",
                client.name()
            ));
            return;
        };
        card.executable = Some(executable.clone());
        card.error = None;
        card.job = Some(std::thread::spawn(move || {
            crate::agent_install::register(client, executable, replace, check)
        }));
    }
}
#[cfg(test)]
#[path = "agent_connections_tests.rs"]
mod tests;
