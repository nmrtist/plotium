//! Authenticated, ephemeral local MCP connection and bounded desktop bridge.
mod capabilities;
pub mod client_config;
pub mod connector;
mod diagnostics;
pub mod discovery;
mod imports;
mod preview;
mod protocol;
mod session;
mod transforms;
#[cfg(windows)]
mod windows_private;

use axum::{
    Router,
    extract::{Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::Response,
};
pub use session::{Failure, Session};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

pub struct PendingRequest {
    pub name: String,
    pub arguments: serde_json::Value,
    pub response: oneshot::Sender<Result<serde_json::Value, Failure>>,
    pub cancellation: CancellationToken,
    pub transport: Option<rmcp::Peer<rmcp::RoleServer>>,
}

impl PendingRequest {
    /// Check on the desktop immediately before starting the atomic operation.
    /// The token also covers cancellation before the HTTP task gets rescheduled.
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled() || self.response.is_closed()
    }
}

pub struct Connection {
    pub url: String,
    token: String,
    requests: mpsc::Receiver<PendingRequest>,
    errors: std::sync::mpsc::Receiver<String>,
    cancel: CancellationToken,
    active: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    pub session: Session,
    instance_id: uuid::Uuid,
    lease: Option<discovery::Lease>,
    activity: Arc<std::sync::Mutex<Activity>>,
    preview: Option<preview::PreviewFlight>,
    retiring_preview: Option<
        std::thread::JoinHandle<
            Result<plotium_core::automation::ToolResult, plotium_core::automation::AutomationError>,
        >,
    >,
    diagnostic: Option<diagnostics::DiagnosticFlight>,
    import: Option<imports::ImportFlight>,
    transform: Option<transforms::TransformFlight>,
}
impl Connection {
    pub fn start(wake: impl Fn() + Send + Sync + 'static) -> std::io::Result<Self> {
        let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let authority = listener.local_addr()?.to_string();
        let origin = format!("http://{authority}");
        // Two independent v4 UUIDs supply 244 random bits. Never persisted or logged.
        let token = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let cancel = CancellationToken::new();
        let active = Arc::new(AtomicBool::new(true));
        let (send, requests) = mpsc::channel(32);
        let (error_send, errors) = std::sync::mpsc::channel();
        let service = protocol::Handler {
            send,
            cancel: cancel.clone(),
            wake: Arc::new(wake),
        };
        let instance_id = uuid::Uuid::new_v4();
        let activity = Arc::new(std::sync::Mutex::new(Activity::default()));
        let guard = Guard {
            instance_id,
            activity: activity.clone(),
            bearer: format!("Bearer {token}"),
            authority: authority.clone(),
            origin: origin.clone(),
            active: active.clone(),
        };
        let worker_cancel = cancel.clone();
        let worker_active = active.clone();
        let worker_wake = service.wake.clone();
        let url = format!("{origin}/mcp");
        let worker = std::thread::Builder::new()
            .name("plotium-agent".into())
            .spawn(move || {
                let run = || -> Result<(), Box<dyn std::error::Error>> {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()?;
                    runtime.block_on(async move {
                        use rmcp::transport::streamable_http_server::{
                            StreamableHttpService, session::local::LocalSessionManager,
                            tower::StreamableHttpServerConfig,
                        };
                        let transport = StreamableHttpService::new(
                            move || Ok(service.clone()),
                            Arc::new(LocalSessionManager::default()),
                            StreamableHttpServerConfig::default()
                                .with_allowed_hosts([authority])
                                .with_allowed_origins([origin])
                                .with_json_response(true)
                                .with_cancellation_token(worker_cancel.clone()),
                        );
                        let router = Router::new()
                            .nest_service("/mcp", transport)
                            .route("/status", axum::routing::get(status))
                            .with_state(guard.clone())
                            .layer(middleware::from_fn_with_state(guard, authenticate));
                        axum::serve(tokio::net::TcpListener::from_std(listener)?, router)
                            .with_graceful_shutdown(worker_cancel.cancelled_owned())
                            .await
                    })?;
                    Ok(())
                };
                let result = run();
                worker_active.store(false, Ordering::Release);
                if let Err(error) = result {
                    // Receiver can disappear on shutdown; there is then no UI to notify.
                    if error_send.send(error.to_string()).is_err() {
                        eprintln!(
                            "Plotium agent connection ended after the desktop disconnected: {error}"
                        );
                    }
                }
                worker_wake();
            })?;
        // Starting a connection explicitly grants reversible project edits.
        // An unconnected/default Session and a stopped connection stay read only.
        let mut session = Session::default();
        session.allow_modify = true;
        Ok(Self {
            url,
            token,
            instance_id,
            activity,
            lease: None,
            requests,
            errors,
            cancel,
            active,
            worker: Some(worker),
            session,
            preview: None,
            retiring_preview: None,
            diagnostic: None,
            import: None,
            transform: None,
        })
    }
    pub fn start_published(wake: impl Fn() + Send + Sync + 'static) -> std::io::Result<Self> {
        Self::start_in(discovery::runtime_dir()?, wake)
    }
    pub(crate) fn start_in(
        dir: std::path::PathBuf,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> std::io::Result<Self> {
        let mut lease = discovery::Lease::acquire(dir)?;
        let mut connection = Self::start(wake)?;
        lease.publish(&discovery::Discovery {
            version: 1,
            instance_id: connection.instance_id,
            url: connection.url.clone(),
            token: connection.token.clone(),
        })?;
        connection.lease = Some(lease);
        Ok(connection)
    }
    pub fn activity(&self) -> Activity {
        self.activity
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
    pub fn is_running(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }
    /// Drop all document-bound work before polling completions in a new context.
    pub fn synchronize_project(&mut self, app: &plotium_core::state::PlotiumApp) {
        if self.session.synchronize_context(app) || app.session.ui.project_transition.is_some() {
            if let Some(flight) = self.diagnostic.take() {
                flight.cancel("project_changed");
            }
            if let Some(flight) = self.preview.take() {
                flight.cancellation.cancel();
                self.retiring_preview = Some(flight.worker);
                if flight
                    .request
                    .response
                    .send(Err(Failure::new(
                        "project_changed",
                        "Project changed; reread state and replan",
                    )))
                    .is_err()
                { /* Client disconnected. */ }
            }
            if let Some(flight) = self.transform.take() {
                flight.cancel();
            }
            if let Some(flight) = self.import.take() {
                flight.cancel();
            }
            while let Ok(request) = self.requests.try_recv() {
                if request
                    .response
                    .send(Err(Failure::new(
                        "project_changed",
                        "Project changed; reread state and replan",
                    )))
                    .is_err()
                { /* Client disconnected. */ }
            }
        }
    }
    pub fn connection_info(&self) -> String {
        serde_json::json!({"url": self.url, "headers": {"Authorization": format!("Bearer {}", self.token)}}).to_string()
    }
    pub fn next_request(&mut self) -> Option<PendingRequest> {
        while let Ok(request) = self.requests.try_recv() {
            if !request.is_cancelled() {
                return Some(request);
            }
        }
        None
    }
    pub fn take_error(&self) -> Option<String> {
        self.errors.try_recv().ok()
    }
    pub fn stop(&mut self) {
        self.session.revoke_saves();
        self.session.revoke_exports();
        self.active.store(false, Ordering::Release);
        self.cancel.cancel();
        self.requests.close();
        while let Ok(request) = self.requests.try_recv() {
            if request
                .response
                .send(Err(Failure::new(
                    "connection_stopped",
                    "Connection stopped",
                )))
                .is_err()
            {
                // Client already disconnected.
            }
        }
        if let Some(flight) = self.diagnostic.take() {
            flight.cancel("connection_stopped");
        }
        if let Some(flight) = self.preview.take() {
            flight.cancellation.cancel();
        }
        self.retiring_preview = None;
        self.import = None;
        if let Some(flight) = self.transform.take() {
            flight.cancel.store(true, Ordering::Relaxed);
        }
        self.session = Session::default();
        self.token.clear();
        self.lease = None;
    }
}
impl Drop for Connection {
    fn drop(&mut self) {
        self.stop();
        // Never block the desktop waiting for an HTTP client to disconnect.
        if let Some(worker) = self.worker.take()
            && worker.is_finished()
            && worker.join().is_err()
        {
            eprintln!("Plotium agent connection worker panicked");
        }
    }
}
#[derive(Clone)]
struct Guard {
    bearer: String,
    instance_id: uuid::Uuid,
    activity: Arc<std::sync::Mutex<Activity>>,
    authority: String,
    origin: String,
    active: Arc<AtomicBool>,
}
async fn authenticate(
    State(guard): State<Guard>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if !guard.active.load(Ordering::Acquire) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let headers = request.headers();
    let value = |key| {
        let mut values = headers.get_all(key).iter();
        let first = values.next()?.to_str().ok()?;
        values.next().is_none().then_some(first)
    };
    if value("authorization") != Some(guard.bearer.as_str()) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    if value("host") != Some(guard.authority.as_str())
        || (headers.contains_key("origin") && value("origin") != Some(guard.origin.as_str()))
    {
        return Err(StatusCode::FORBIDDEN);
    }
    {
        let mut activity = guard.activity.lock().unwrap_or_else(|p| p.into_inner());
        activity.last = Some(std::time::Instant::now());
        if value("x-plotium-client") == Some("claude-desktop") {
            activity.desktop_seen = true;
        }
    }
    Ok(next.run(request).await)
}

#[cfg(test)]
mod display_tests;
#[cfg(test)]
mod http_tests;
#[cfg(test)]
mod session_tests;

#[cfg(test)]
mod component_tests;
#[cfg(test)]
mod property_batch_tests;

#[cfg(test)]
mod discovery_tests;

#[cfg(test)]
mod preview_tests;

#[cfg(test)]
mod binding_tests;

#[cfg(test)]
mod creation_tests;

#[derive(Clone, Default)]
pub struct Activity {
    pub last: Option<std::time::Instant>,
    pub desktop_seen: bool,
}
async fn status(
    State(guard): State<Guard>,
) -> ([(axum::http::header::HeaderName, &'static str); 1], String) {
    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        serde_json::json!({"version":1,"instance_id":guard.instance_id}).to_string(),
    )
}

#[cfg(test)]
mod context_tests;
#[cfg(test)]
mod precise_display_tests;
#[cfg(test)]
mod rendezvous_tests;

#[cfg(test)]
mod content_edit_tests;
#[cfg(test)]
mod figure_edit_tests;

#[cfg(test)]
mod table_update_tests;

#[cfg(test)]
mod export_tests;
#[cfg(test)]
mod save_tests;

#[cfg(test)]
mod guide_fixture;
#[cfg(test)]
mod guide_tests;

#[cfg(test)]
mod deletion_tests;

#[cfg(test)]
mod diagnostic_tests;

#[cfg(test)]
mod candidate_tests;

#[cfg(test)]
mod candidate_lifecycle_tests;

#[cfg(test)]
mod candidate_fixture;

#[cfg(test)]
mod native_import_tests;

#[cfg(test)]
mod science_tests;
