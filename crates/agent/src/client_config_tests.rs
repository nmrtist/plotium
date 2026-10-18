use super::*;

#[cfg(unix)]
#[test]
fn posix_launch_scopes_token_and_preserves_literal_config() {
    // Exercise the copied command in a real shell without launching Codex or
    // accessing a network. Metacharacters must remain data, including quotes.
    let token = "test-'$(exit 91)`exit 92`";
    let config = "mcp_servers.plotium={url=\"http://127.0.0.1:1234/mcp\",bearer_token_env_var=\"PLOTIUM_MCP_TOKEN\",enabled=true}";
    let command = launch_command(token, config, false);
    use std::os::unix::fs::PermissionsExt;
    let directory = std::env::temp_dir().join(format!("plotium-codex-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    let executable = directory.join("codex");
    std::fs::write(
        &executable,
        "#!/bin/sh\nprintf '%s\\n' \"$PLOTIUM_MCP_TOKEN\" \"$@\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = format!("{}:{}", directory.display(), std::env::var("PATH").unwrap());
    for shell in ["sh", "bash", "zsh"] {
        let output = match std::process::Command::new(shell)
            .arg("-c")
            .arg(format!("{command}; printf '%s\\n' \"$PLOTIUM_MCP_TOKEN\""))
            .env("PATH", &path)
            .env("PLOTIUM_MCP_TOKEN", "previous")
            .output()
        {
            Ok(output) => output,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => panic!("Could not run {shell}: {error}"),
        };
        assert!(output.status.success(), "{shell}: {:?}", output.stderr);
        assert_eq!(
            String::from_utf8(output.stdout)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            [token, "--no-daemon", "-c", config, "previous"],
            "{shell}",
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn powershell_command_restores_environment_and_quotes_literals() {
    assert_eq!(quote_powershell("a'b$(value)"), "'a''b$(value)'");
    let command = launch_command("test-token", "server={url=\"test\"}", true);
    assert!(command.starts_with("& { $previousPlotiumToken = $env:PLOTIUM_MCP_TOKEN; try {"));
    assert!(command.contains("codex --no-daemon -c 'server={url=\"test\"}'"));
    assert!(command.ends_with("finally { $env:PLOTIUM_MCP_TOKEN = $previousPlotiumToken } }"));
}
