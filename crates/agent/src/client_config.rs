//! Copyable client setup for the current connection; never writes client files.
use crate::Connection;

impl Connection {
    /// Start an isolated CLI session so an existing daemon cannot retain a stale
    /// environment. Startup overrides keep the saved connection URL untouched.
    pub fn codex_launch_command(&self) -> String {
        let config = format!(
            "mcp_servers.plotium={{url={},bearer_token_env_var=\"PLOTIUM_MCP_TOKEN\",enabled=true}}",
            serde_json::to_string(&self.url).expect("a URL string serializes as JSON"),
        );
        launch_command(&self.token, &config, cfg!(windows))
    }
}

fn launch_command(token: &str, config: &str, powershell: bool) -> String {
    if powershell {
        // A script scope keeps the previous-value variable out of the user's
        // session, and finally restores the environment even on Ctrl-C/error.
        format!(
            "& {{ $previousPlotiumToken = $env:PLOTIUM_MCP_TOKEN; try {{ $env:PLOTIUM_MCP_TOKEN = {}; codex --no-daemon -c {} }} finally {{ $env:PLOTIUM_MCP_TOKEN = $previousPlotiumToken }} }}",
            quote_powershell(token),
            quote_powershell(config),
        )
    } else {
        format!(
            "PLOTIUM_MCP_TOKEN={} codex --no-daemon -c {}",
            quote_posix(token),
            quote_posix(config),
        )
    }
}

fn quote_posix(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn quote_powershell(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(test)]
#[path = "client_config_tests.rs"]
mod tests;
