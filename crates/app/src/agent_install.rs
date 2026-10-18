//! User-initiated installation. No shell interpolation or client auto-launch.
use serde_json::{Value, json};
use std::{
    ffi::OsString,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Client {
    Codex,
    ClaudeCode,
}
impl Client {
    pub fn name(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::ClaudeCode => "Claude Code",
        }
    }
    pub fn executable(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::ClaudeCode => "claude",
        }
    }
    pub fn help(self) -> &'static str {
        match self {
            Self::Codex => "https://developers.openai.com/codex/cli/",
            Self::ClaudeCode => "https://code.claude.com/docs/en/overview",
        }
    }
}
#[derive(Debug)]
pub(crate) enum InstallResult {
    Configured,
    Conflict(Value),
    Checked(bool),
}
fn search_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<_> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    if let Some(home) = home() {
        dirs.extend([
            home.join(".local/bin"),
            home.join(".cargo/bin"),
            home.join(".npm-global/bin"),
        ]);
        if let Ok(versions) = fs::read_dir(home.join(".nvm/versions/node")) {
            let mut versions = versions
                .filter_map(|e| e.ok())
                .map(|e| e.path().join("bin"))
                .collect::<Vec<_>>();
            versions.sort();
            versions.reverse();
            dirs.extend(versions);
        }
        #[cfg(windows)]
        {
            let npm = home.join("AppData/Roaming/npm/node_modules/@openai/codex");
            for base in [&npm, &npm.join("node_modules/@openai/codex-win32-x64")] {
                dirs.push(base.join("vendor/x86_64-pc-windows-msvc/codex"));
            }
        }
    }
    dirs.extend([
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ]);
    dirs
}
pub(crate) fn locate(client: Client) -> Option<PathBuf> {
    for dir in search_dirs() {
        let path = dir.join(format!(
            "{}{}",
            client.executable(),
            if cfg!(windows) { ".exe" } else { "" }
        ));
        if path.is_file() {
            return Some(path);
        }
    }
    None
}
fn home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}
pub(crate) fn install_connector() -> Result<PathBuf, String> {
    let target = plotium_agent::discovery::connector_path().map_err(|e| e.to_string())?;
    let source = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .with_file_name(if cfg!(windows) {
            "plotium-mcp.exe"
        } else {
            "plotium-mcp"
        });
    install_copy(&source, &target)?;
    Ok(target)
}
fn install_copy(source: &Path, target: &Path) -> Result<(), String> {
    let bytes = fs::read(source).map_err(|e| {
        format!(
            "Connector missing: {} ({e}). Reinstall the complete Plotium release package.",
            source.display()
        )
    })?;
    if fs::read(target).ok().as_deref() == Some(bytes.as_slice()) {
        return Ok(());
    }
    let parent = target.parent().ok_or("Invalid connector location")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temporary = parent.join(format!(".plotium-mcp-{}", uuid::Uuid::new_v4()));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o700);
    }
    let result = (|| -> std::io::Result<()> {
        let mut file = options.open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, target)
    })();
    if let Err(error) = result {
        if let Err(cleanup) = fs::remove_file(&temporary)
            && cleanup.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!("Connector temporary cleanup: {cleanup}");
        }
        return Err(format!(
            "Could not update connector ({error}). Exit the AI clients using it and retry; the installed copy is retained."
        ));
    }
    Ok(())
}
fn command(executable: &Path, args: &[OsString]) -> Result<std::process::Output, String> {
    command_with_timeout(executable, args, Duration::from_secs(20))
}
fn client_command(executable: &Path, directories: &[PathBuf]) -> Result<Command, String> {
    // Desktop launchers often supply a minimal PATH. A CLI found outside that
    // PATH may use /usr/bin/env node, so expose its sibling runtime as well.
    // Resolve symlinks for clients selected through a separate launch directory.
    let resolved = fs::canonicalize(executable).ok();
    let mut paths = Vec::new();
    for directory in executable
        .parent()
        .into_iter()
        .chain(resolved.as_deref().and_then(Path::parent))
        .chain(directories.iter().map(PathBuf::as_path))
    {
        if !directory.as_os_str().is_empty() && !paths.iter().any(|p| p == directory) {
            paths.push(directory.to_path_buf());
        }
    }
    let path = std::env::join_paths(paths)
        .map_err(|e| format!("Cannot prepare client executable search path: {e}"))?;
    let mut command = Command::new(executable);
    command.env("PATH", path);
    Ok(command)
}
fn command_with_timeout(
    executable: &Path,
    args: &[OsString],
    timeout: Duration,
) -> Result<std::process::Output, String> {
    let mut child = client_command(executable, &search_dirs())?
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Cannot run {}: {e}", executable.display()))?;
    let stdout = child.stdout.take().ok_or("Missing client stdout")?;
    let stderr = child.stderr.take().ok_or("Missing client stderr")?;
    let read = |stream: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            stream
                .take(1024 * 1024)
                .read_to_end(&mut bytes)
                .map(|_| bytes)
        })
    };
    let out = read(Box::new(stdout));
    let err = read(Box::new(stderr));
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().map_err(|e| e.to_string())?;
            child.wait().map_err(|e| e.to_string())?;
            return Err("Client command timed out after 20 seconds. Check the client installation and retry.".into());
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    Ok(std::process::Output {
        status,
        stdout: out
            .join()
            .map_err(|_| "Client output worker failed")?
            .map_err(|e| e.to_string())?,
        stderr: err
            .join()
            .map_err(|_| "Client error worker failed")?
            .map_err(|e| e.to_string())?,
    })
}
fn run(exe: &Path, args: &[&str]) -> Result<std::process::Output, String> {
    command(exe, &args.iter().map(OsString::from).collect::<Vec<_>>())
}
fn require_success(output: std::process::Output) -> Result<String, String> {
    if !output.status.success() {
        return Err(format!(
            "Client registration failed: {}",
            failure_detail(&output)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
fn failure_detail(output: &std::process::Output) -> String {
    let detail = format!(
        "{} {}",
        String::from_utf8_lossy(&output.stderr).trim(),
        String::from_utf8_lossy(&output.stdout).trim()
    );
    let detail = detail.trim();
    if detail.contains("env:")
        && detail.contains("node")
        && (detail.contains("No such file") || detail.contains("not found"))
    {
        format!(
            "Node.js could not be found while starting the client. Check the Node.js installation or choose a standalone client executable, then retry. ({detail})"
        )
    } else {
        detail.to_owned()
    }
}
fn entry(client: Client, exe: &Path) -> Result<Option<Value>, String> {
    if client == Client::Codex {
        let result = run(exe, &["mcp", "get", "plotium", "--json"])?;
        if result.status.success() {
            return serde_json::from_slice(&result.stdout)
                .map(Some)
                .map_err(|e| e.to_string());
        }
        let error = failure_detail(&result);
        if error.contains("No MCP server named") {
            return Ok(None);
        }
        return Err(format!(
            "Cannot read Codex registration: {error}. Check the client executable or installation and retry."
        ));
    }
    let path = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(|p| PathBuf::from(p).join(".claude.json"))
        .or_else(|| home().map(|p| p.join(".claude.json")))
        .ok_or("Cannot locate Claude Code user configuration")?;
    match fs::read(path) {
        Ok(bytes) => {
            let config: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            Ok(config
                .get("mcpServers")
                .and_then(|s| s.get("plotium"))
                .cloned())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("Cannot read Claude Code configuration: {e}")),
    }
}
fn matches(entry: &Value, connector: &Path) -> bool {
    let transport = entry.get("transport").unwrap_or(entry);
    transport["command"]
        .as_str()
        .is_some_and(|s| Path::new(s) == connector)
        && transport
            .get("args")
            .is_none_or(|v| v.as_array().is_some_and(Vec::is_empty))
        && transport
            .get("env")
            .is_none_or(|v| v.is_null() || v.as_object().is_some_and(serde_json::Map::is_empty))
        && transport
            .get("env_vars")
            .is_none_or(|v| v.is_null() || v.as_array().is_some_and(Vec::is_empty))
        && transport.get("cwd").is_none_or(Value::is_null)
        && entry.get("enabled") != Some(&json!(false))
}
pub(crate) fn register(
    client: Client,
    executable: PathBuf,
    replace: Option<Value>,
    check_only: bool,
) -> Result<InstallResult, String> {
    let connector = if check_only {
        plotium_agent::discovery::connector_path().map_err(|e| e.to_string())?
    } else {
        install_connector()?
    };
    register_at(
        client,
        executable,
        connector,
        replace,
        check_only,
        &plotium_agent::discovery::runtime_dir().map_err(|e| e.to_string())?,
    )
}
fn register_at(
    client: Client,
    executable: PathBuf,
    connector: PathBuf,
    replace: Option<Value>,
    check_only: bool,
    backup_dir: &Path,
) -> Result<InstallResult, String> {
    let existing = entry(client, &executable)?;
    if check_only {
        return Ok(InstallResult::Checked(
            existing.as_ref().is_some_and(|e| matches(e, &connector)),
        ));
    }
    if let Some(existing) = &existing {
        if matches(existing, &connector) {
            return Ok(InstallResult::Configured);
        }
        if replace.as_ref() != Some(existing) {
            return Ok(InstallResult::Conflict(existing.clone()));
        }
        if existing.get("enabled") == Some(&json!(false)) {
            return Err("Plotium is disabled in the client. Enable it in the client settings before adding it.".into());
        }
        let dir = backup_dir;
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let backup = dir.join(format!(
            "{}-plotium-{}.json",
            client.executable(),
            uuid::Uuid::new_v4()
        ));
        let mut opts = fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let file = opts.open(&backup).map_err(|e| e.to_string())?;
        serde_json::to_writer_pretty(file, existing).map_err(|e| e.to_string())?;
        let args = if client == Client::Codex {
            vec!["mcp", "remove", "plotium"]
        } else {
            vec!["mcp", "remove", "--scope", "user", "plotium"]
        };
        require_success(run(&executable, &args)?)?;
    }
    let args = if client == Client::Codex {
        vec!["mcp", "add", "plotium", "--"]
    } else {
        vec![
            "mcp",
            "add",
            "--transport",
            "stdio",
            "--scope",
            "user",
            "plotium",
            "--",
        ]
    };
    let mut args: Vec<_> = args.into_iter().map(OsString::from).collect();
    args.push(connector.as_os_str().to_owned());
    require_success(command(&executable, &args)?)?;
    if !entry(client, &executable)?
        .as_ref()
        .is_some_and(|e| matches(e, &connector))
    {
        return Err("Registration did not match the requested connector. Inspect client policy and configuration.".into());
    }
    if client == Client::ClaudeCode {
        require_success(run(&executable, &["mcp", "get", "plotium"])?)?;
    }
    Ok(InstallResult::Configured)
}

#[cfg(test)]
#[path = "agent_install_tests.rs"]
mod tests;
