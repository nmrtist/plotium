//! Diagnostics alone retain an HTTP transport across stdio calls. The facade's
//! ordinary operations deliberately use a fresh, non-replayed call transport.
use super::*;
use rmcp::{RoleClient, service::RunningService};
use std::sync::Arc;
use tokio::sync::Mutex;

struct Client {
    instance: uuid::Uuid,
    service: RunningService<RoleClient, ()>,
}
#[derive(Clone, Default)]
pub(super) struct DiagnosticTransport(Arc<Mutex<Option<Client>>>);
impl DiagnosticTransport {
    pub(super) async fn close(&self) {
        if let Some(mut client) = self.0.lock().await.take() {
            match client
                .service
                .close_with_timeout(Duration::from_millis(250))
                .await
            {
                Ok(Some(_)) => {}
                Ok(None) => {
                    eprintln!("Plotium diagnostic transport cleanup continues in background")
                }
                Err(error) => eprintln!("Plotium diagnostic transport cleanup failed: {error}"),
            }
        }
    }
}
impl Connector {
    pub(super) async fn diagnostic_call(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
        entry: Discovery,
    ) -> Result<CallToolResponse, ErrorData> {
        let Ok(mut stored) = self.diagnostic.0.try_lock() else {
            return Ok(CallToolResult::structured_error(
                Failure::new("busy", "A diagnostic request is already in flight").value(),
            )
            .into());
        };
        let instance = entry.instance_id;
        if stored
            .as_ref()
            .is_some_and(|c| c.instance != instance || c.service.is_transport_closed())
        {
            *stored = None;
        }
        if stored.is_none() {
            let service = match self.connect(entry).await {
                Ok(service) => service,
                Err(message) => {
                    return Ok(CallToolResult::structured_error(
                        Failure::new("unavailable", message).value(),
                    )
                    .into());
                }
            };
            *stored = Some(Client { instance, service });
        }
        let client = stored.as_ref().expect("initialized above");
        let outcome = tokio::select! {
            _ = context.ct.cancelled() => Err("Diagnostic cancelled; read state and begin a new scan"),
            _ = self.wait_for_loss(instance) => Err("Plotium connection was revoked or restarted; read state and begin a new scan. No call was replayed."),
            result = client.service.call_tool(request) => result.map_err(|_| "Plotium disconnected; read state and begin a new scan. No call was replayed."),
        };
        Ok(match outcome {
            Ok(result) => relayed(result),
            Err(message) => {
                // Dropping the service cancels the transport and invalidates its
                // retained server cursor, including when the stdio caller cancels.
                *stored = None;
                CallToolResult::structured_error(Failure::new("connection_lost", message).value())
                    .into()
            }
        })
    }
}
