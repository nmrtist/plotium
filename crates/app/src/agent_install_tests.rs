use super::*;

#[test]
fn windows_npm_codex_discovery_handles_architectures_and_package_layouts() {
    for (architecture, package, target) in [
        ("x86_64", "codex-win32-x64", "x86_64-pc-windows-msvc"),
        ("aarch64", "codex-win32-arm64", "aarch64-pc-windows-msvc"),
    ] {
        for relative in [
            format!("codex/node_modules/@openai/{package}"),
            package.to_owned(),
            "codex".to_owned(),
        ] {
            let temp = tempfile::tempdir().unwrap();
            let prefix = temp.path().join("custom npm \u{7528}\u{6237} path");
            fs::create_dir_all(&prefix).unwrap();
            fs::write(prefix.join("codex.cmd"), "npm launcher").unwrap();
            fs::write(prefix.join("codex.ps1"), "npm launcher").unwrap();
            assert_eq!(windows_codex_in(&prefix, architecture), None);

            let native = prefix
                .join("node_modules/@openai")
                .join(relative)
                .join("vendor")
                .join(target)
                .join("bin/codex.exe");
            fs::create_dir_all(native.parent().unwrap()).unwrap();
            fs::write(&native, "native payload").unwrap();
            assert_eq!(windows_codex_in(&prefix, architecture), Some(native));
            let other = if architecture == "x86_64" {
                "aarch64"
            } else {
                "x86_64"
            };
            assert_eq!(windows_codex_in(&prefix, other), None);
            assert_eq!(windows_codex_in(&prefix, "unsupported"), None);
        }
    }
}

#[cfg(windows)]
#[test]
fn windows_client_discovery_preserves_path_order_and_prefers_direct_executables() {
    let temp = tempfile::tempdir().unwrap();
    let prefix = temp.path().join("npm");
    let standalone = temp.path().join("standalone");
    fs::create_dir_all(&prefix).unwrap();
    fs::create_dir_all(&standalone).unwrap();
    fs::write(prefix.join("codex.cmd"), "npm launcher").unwrap();
    let direct = standalone.join("codex.exe");
    fs::write(&direct, "standalone payload").unwrap();
    let directories = [prefix.clone(), standalone.clone()];
    assert_eq!(locate_in(Client::Codex, &directories), Some(direct.clone()));
    assert_eq!(locate_in(Client::ClaudeCode, &directories), None);

    let (package, target) = match std::env::consts::ARCH {
        "x86_64" => ("codex-win32-x64", "x86_64-pc-windows-msvc"),
        "aarch64" => ("codex-win32-arm64", "aarch64-pc-windows-msvc"),
        _ => return,
    };
    let native = prefix
        .join("node_modules/@openai/codex/node_modules/@openai")
        .join(package)
        .join("vendor")
        .join(target)
        .join("bin/codex.exe");
    fs::create_dir_all(native.parent().unwrap()).unwrap();
    fs::write(&native, "native payload").unwrap();
    assert_eq!(locate_in(Client::Codex, &directories), Some(native));
    assert_eq!(
        locate_in(Client::Codex, &[standalone, prefix.clone()]),
        Some(direct)
    );
    let direct = prefix.join("codex.exe");
    fs::write(&direct, "direct payload").unwrap();
    assert_eq!(locate_in(Client::Codex, &directories), Some(direct));
}

#[test]
fn installing_and_updating_connector_preserves_unicode_paths_and_previous_copy_on_failure() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let target = temp.path().join("\u{7528}\u{6237} path/bin/plotium-mcp");
    fs::write(&source, b"v1").unwrap();
    install_copy(&source, &target).unwrap();
    install_copy(&source, &target).unwrap();
    fs::write(&source, b"v2").unwrap();
    install_copy(&source, &target).unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"v2");
    assert!(install_copy(&temp.path().join("missing"), &target).is_err());
    assert_eq!(fs::read(&target).unwrap(), b"v2");
}
#[cfg(unix)]
fn executable(path: &Path, script: &str) {
    use std::os::unix::fs::PermissionsExt;
    fs::write(path, script).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}
#[cfg(unix)]
fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}
#[cfg(unix)]
#[test]
fn registration_is_idempotent_requires_matching_conflict_confirmation_and_backs_up_entry() {
    let temp = tempfile::tempdir().unwrap();
    let executable_path = temp.path().join("client path \u{4e2d}\u{6587}");
    let state = temp.path().join("state.json");
    let connector = temp.path().join("\u{5de5}\u{5177} path/plotium-mcp");
    let requested =
        json!({"transport":{"type":"stdio","command":connector,"args":[]},"enabled":true});
    executable(
        &executable_path,
        &format!(
            "#!/bin/sh\ncase \"$2\" in\nget) if test -f {state}; then cat {state}; else echo 'No MCP server named plotium' >&2; exit 1; fi;;\nadd) printf '%s' {requested} > {state};;\nremove) rm {state};;\nesac\n",
            state = quote(state.to_str().unwrap()),
            requested = quote(&requested.to_string())
        ),
    );
    let register = |replace| {
        register_at(
            Client::Codex,
            executable_path.clone(),
            connector.clone(),
            replace,
            false,
            temp.path(),
        )
    };
    assert!(matches!(register(None).unwrap(), InstallResult::Configured));
    assert!(matches!(register(None).unwrap(), InstallResult::Configured));
    let other = json!({"transport":{"type":"stdio","command":"other","args":[]}});
    fs::write(&state, other.to_string()).unwrap();
    assert!(matches!(register(None).unwrap(), InstallResult::Conflict(value) if value==other));
    assert!(matches!(
        register(Some(json!({}))).unwrap(),
        InstallResult::Conflict(_)
    ));
    assert!(matches!(
        register(Some(other.clone())).unwrap(),
        InstallResult::Configured
    ));
    let backup = fs::read_dir(temp.path())
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("codex-plotium-")
        })
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(backup).unwrap()).unwrap(),
        other
    );
    let disabled =
        json!({"transport":{"type":"stdio","command":connector,"args":[]},"enabled":false});
    fs::write(&state, disabled.to_string()).unwrap();
    assert!(register(Some(disabled)).unwrap_err().contains("disabled"));
}
#[cfg(unix)]
#[test]
fn client_timeout_and_registration_failure_report_useful_errors() {
    let temp = tempfile::tempdir().unwrap();
    let script = temp.path().join("slow");
    executable(&script, "#!/bin/sh\nexec sleep 10\n");
    assert!(
        command_with_timeout(&script, &[], Duration::from_millis(50))
            .unwrap_err()
            .contains("timed out")
    );
    executable(
        &script,
        "#!/bin/sh\necho 'configuration is read only' >&2\nexit 1\n",
    );
    assert!(
        require_success(command(&script, &[]).unwrap())
            .unwrap_err()
            .contains("read only")
    );
}

#[cfg(unix)]
#[test]
fn desktop_client_finds_node_with_minimal_path_and_symlinked_launcher() {
    let temp = tempfile::tempdir().unwrap();
    let runtime = temp.path().join("node runtime \u{4e2d}\u{6587}");
    let launchers = temp.path().join("launchers");
    fs::create_dir(&runtime).unwrap();
    fs::create_dir(&launchers).unwrap();
    let script = runtime.join("codex");
    executable(&script, "#!/usr/bin/env node\n");
    executable(&runtime.join("node"), "#!/bin/sh\nprintf '%s\\n' \"$@\"\n");
    let inherited = std::env::var_os("PATH");
    let minimal = [PathBuf::from("/usr/bin"), PathBuf::from("/bin")];
    for launcher in [&script, &launchers.join("codex")] {
        if launcher != &script {
            std::os::unix::fs::symlink(&script, launcher).unwrap();
        }
        let output = client_command(launcher, &minimal)
            .unwrap()
            .arg("argument with spaces")
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output.stderr);
        assert!(
            String::from_utf8(output.stdout)
                .unwrap()
                .ends_with("argument with spaces\n")
        );
    }
    // A globally installed launcher may keep its runtime in another known bin.
    let global = launchers.join("global-client");
    executable(&global, "#!/usr/bin/env node\n");
    assert!(
        client_command(&global, &[runtime])
            .unwrap()
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(std::env::var_os("PATH"), inherited);
}

#[cfg(unix)]
#[test]
fn missing_node_reports_runtime_recovery_instead_of_unsupported_mcp() {
    let temp = tempfile::tempdir().unwrap();
    let script = temp.path().join("codex");
    executable(&script, "#!/usr/bin/env node\n");
    let output = client_command(&script, &[]).unwrap().output().unwrap();
    assert!(!output.status.success());
    let error = require_success(output).unwrap_err();
    assert!(error.contains("Node.js could not be found"), "{error}");
    assert!(!error.contains("MCP commands are unsupported"));
}
