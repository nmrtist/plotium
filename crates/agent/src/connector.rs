//! Stable stdio facade. Reconnect transport only; never retry a submitted call.
use crate::{
    Failure,
    discovery::{Discovery, runtime_dir},
};
use rmcp::{
    ErrorData, RoleServer, ServerHandler, ServiceExt,
    model::*,
    service::RequestContext,
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use std::{path::PathBuf, time::Duration};

#[derive(Clone)]
pub struct Connector {
    directory: PathBuf,
    client: reqwest::Client,
    identity: &'static str,
}
impl Connector {
    pub fn new(desktop: bool) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            directory: runtime_dir()?,
            client: reqwest::Client::builder()
                .no_proxy()
                .connect_timeout(Duration::from_secs(2))
                .build()?,
            identity: if desktop { "claude-desktop" } else { "stdio" },
        })
    }
    async fn discover(&self) -> Result<Discovery, String> {
        let entry = Discovery::read(&self.directory).map_err(|_| {
            "Plotium MCP is unavailable. Open Plotium and enable MCP in AI connections (MCP)."
                .to_owned()
        })?;
        let status_url = format!("{}/status", entry.url.trim_end_matches("/mcp"));
        let response = self
            .client
            .get(status_url)
            .bearer_auth(&entry.token)
            .header("x-plotium-client", self.identity)
            .timeout(Duration::from_secs(3))
            .send()
            .await
            .map_err(|_| {
                "Plotium is unavailable; reopen it or check AI connections (MCP).".to_owned()
            })?;
        if !response.status().is_success() {
            return Err("Plotium connection was revoked; check AI connections (MCP).".into());
        }
        let status: serde_json::Value = response
            .json()
            .await
            .map_err(|_| "Invalid Plotium status response".to_owned())?;
        if status["version"] != 1 || status["instance_id"] != entry.instance_id.to_string() {
            return Err("Stale Plotium discovery; wait for the application to recover.".into());
        }
        Ok(entry)
    }
    async fn wait_for_loss(&self, instance: uuid::Uuid) {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            match self.discover().await {
                Ok(entry) if entry.instance_id == instance => {}
                _ => return,
            }
        }
    }
    pub async fn heartbeat(&self) {
        if let Err(error) = self.discover().await {
            eprintln!("{error}");
        }
    }
}
impl ServerHandler for Connector {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("Plotium", env!("CARGO_PKG_VERSION")))
            .with_instructions("Open Plotium and enable MCP. Read plotium_get_state before project tools. Supply project_context as expected_project_context. After project_changed, reread state and replan without reusing targets. Never replay calls after a disconnection.")
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult {
            tools: crate::protocol::definitions(),
            ..Default::default()
        })
    }
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        if !crate::protocol::definitions()
            .iter()
            .any(|tool| tool.name == request.name)
        {
            return Err(ErrorData::invalid_params("Unknown tool", None));
        }
        let entry = match self.discover().await {
            Ok(entry) => entry,
            Err(message) => {
                return Ok(CallToolResult::structured_error(
                    Failure::new("unavailable", message).value(),
                )
                .into());
            }
        };
        let mut config = StreamableHttpClientTransportConfig::with_uri(entry.url);
        config.auth_header = Some(entry.token);
        // RMCP defaults to replaying a POST after an expired HTTP session.
        // Project operations are one-shot, so all recovery belongs to the next call.
        config.reinit_on_expired_session = false;
        config.retry_config =
            std::sync::Arc::new(rmcp::transport::common::client_side_sse::NeverRetry::default());
        let instance = entry.instance_id;
        let mut client = match tokio::time::timeout(Duration::from_secs(5), ().serve(StreamableHttpClientTransport::with_client(self.client.clone(), config))).await {
            Ok(Ok(client)) => client,
            _ => return Ok(CallToolResult::structured_error(Failure::new("unavailable", "Plotium disconnected before the call; check application status and reread state.").value()).into()),
        };
        let outcome = tokio::select! {
            _ = context.ct.cancelled() => Err("Request cancelled; inspect state before planning again"),
            _ = self.wait_for_loss(instance) => Err("Plotium connection was revoked or restarted. Outcome may be unknown; inspect state before replanning. The call was not replayed."),
            result = client.call_tool(request) => result.map_err(|_| "Plotium disconnected. Outcome may be unknown; inspect state before replanning. The call was not replayed."),
        };
        match client.close_with_timeout(Duration::from_millis(250)).await {
            Ok(Some(_)) => {}
            Ok(None) => eprintln!("Plotium MCP transport cleanup continues in background"),
            Err(error) => eprintln!("Plotium MCP transport cleanup failed: {error}"),
        }
        Ok(match outcome {
            Ok(result) => result.into(),
            Err(message) => {
                CallToolResult::structured_error(Failure::new("connection_lost", message).value())
                    .into()
            }
        })
    }
}
pub fn run_stdio(desktop: bool) -> Result<(), Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let connector = Connector::new(desktop)?;
        let heartbeat = connector.clone();
        let task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                interval.tick().await;
                heartbeat.heartbeat().await;
            }
        });
        let service = connector.serve(rmcp::transport::stdio()).await?;
        service.waiting().await?;
        task.abort();
        Ok(())
    })
}

#[cfg(test)]
#[path = "connector_tests.rs"]
mod tests;
