//! Authenticated, ephemeral local MCP connection and bounded desktop bridge.
mod client_config;
mod protocol;
mod session;

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
        let guard = Guard {
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
            requests,
            errors,
            cancel,
            active,
            worker: Some(worker),
            session,
        })
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
        self.session = Session::default();
        self.token.clear();
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
    Ok(next.run(request).await)
}

#[cfg(test)]
mod display_tests;
#[cfg(test)]
mod http_tests;
#[cfg(test)]
mod session_tests;
