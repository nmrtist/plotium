//! Stable, credential-free client configuration.
use std::path::Path;
pub fn stdio_config(connector: &Path) -> serde_json::Value {
    serde_json::json!({"mcpServers":{"plotium":{"command":connector,"args":[]}}})
}

#[cfg(test)]
#[path = "client_config_tests.rs"]
mod tests;
