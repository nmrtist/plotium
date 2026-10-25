use super::*;

#[test]
fn executable_architecture_validation_needs_no_developer_tools() {
    let root = tempfile::tempdir().unwrap();
    let binary = root.path().join("plotium");
    for (cpu, other_cpu) in [(0x0100_0007_u32, 0x0100_000c), (0x0100_000c, 0x0100_0007)] {
        let mut header = [0_u8; 32];
        header[..4].copy_from_slice(&[0xcf, 0xfa, 0xed, 0xfe]);
        header[4..8].copy_from_slice(&cpu.to_le_bytes());
        header[12..16].copy_from_slice(&2_u32.to_le_bytes());
        std::fs::write(&binary, header).unwrap();
        verify_architecture(&binary, cpu).unwrap();
        let error = verify_architecture(&binary, other_cpu).unwrap_err();
        assert!(matches!(error, UpdateError::Installation(_)));
        assert!(!error.to_string().contains("server"));
        // A library with the right CPU is still not a launchable application.
        header[12..16].copy_from_slice(&6_u32.to_le_bytes());
        std::fs::write(&binary, header).unwrap();
        assert!(verify_architecture(&binary, cpu).is_err());
    }
    for bytes in [
        vec![],
        vec![0xcf, 0xfa, 0xed, 0xfe],
        vec![0; 32],
        [vec![0xca, 0xfe, 0xba, 0xbe], vec![0; 28]].concat(),
    ] {
        std::fs::write(&binary, bytes).unwrap();
        assert!(verify_architecture(&binary, 0x0100_000c).is_err());
    }
    std::fs::remove_file(&binary).unwrap();
    assert!(verify_architecture(&binary, 0x0100_000c).is_err());
}

#[test]
fn whole_app_swap_keeps_resources_and_restores_old_app_on_failure() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("安装 space/Plotium.app");
    let incoming = root.path().join("incoming/Plotium.app");
    for (app, content) in [(&target, "old"), (&incoming, "new")] {
        std::fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
        for name in ["plotium", "plotium-mcp"] {
            std::fs::write(app.join("Contents/MacOS").join(name), content).unwrap();
        }
        std::fs::write(app.join("Contents/Info.plist"), content).unwrap();
    }
    replace(&incoming, &target, |_| Err(UpdateError::Integrity)).unwrap_err();
    assert_eq!(
        std::fs::read(target.join("Contents/Info.plist")).unwrap(),
        b"old"
    );
    assert_eq!(
        std::fs::read(target.join("Contents/MacOS/plotium-mcp")).unwrap(),
        b"old"
    );
    // A failed rename must also restore the old bundle without validating it.
    replace(&incoming, &target, |_| {
        panic!("missing source cannot be validated")
    })
    .unwrap_err();
    assert_eq!(
        std::fs::read(target.join("Contents/Info.plist")).unwrap(),
        b"old"
    );
    std::fs::create_dir_all(&incoming).unwrap();
    std::fs::write(incoming.join("new-resource"), "resource").unwrap();
    replace(&incoming, &target, |app| {
        assert!(app.join("new-resource").is_file());
        Ok(())
    })
    .unwrap();
    assert!(!target.join("Contents/Info.plist").exists());
    assert!(target.join("new-resource").exists());
}

/// The release job supplies its actual signed DMG. This test is required there,
/// and skipped during ordinary builds that have no macOS packaging artifacts.
#[cfg(target_os = "macos")]
#[test]
fn packaged_dmg_can_replace_an_installed_bundle() {
    let Some(dmg) = std::env::var_os("PLOTIUM_TEST_UPDATE_DMG") else {
        return;
    };
    let root = tempfile::tempdir().unwrap();
    let staging = root.path().join("download");
    std::fs::create_dir(&staging).unwrap();
    let downloaded = staging.join("update.dmg");
    std::fs::copy(dmg, &downloaded).unwrap();
    let target = root.path().join("安装 space/Plotium.app");
    let executable = target.join("Contents/MacOS/plotium");
    std::fs::create_dir_all(executable.parent().unwrap()).unwrap();
    std::fs::write(&executable, b"previous build").unwrap();
    let version = env!("CARGO_PKG_VERSION");
    // An incorrect release version fails before touching the installed copy.
    assert!(prepare(&downloaded, &executable, "9999.0.0").is_err());
    assert_eq!(std::fs::read(&executable).unwrap(), b"previous build");
    let incoming = prepare(&downloaded, &executable, version).unwrap();
    assert_eq!(std::fs::read(&executable).unwrap(), b"previous build");
    let helper = staging.join("plotium-update-helper");
    std::fs::copy(incoming.join("Contents/MacOS/plotium"), &helper).unwrap();
    // Execute the shipping helper, not just the in-process directory swap.
    let output = std::process::Command::new(&helper)
        // Developer-tool shims fail with this unavailable toolchain selection.
        .env("DEVELOPER_DIR", root.path().join("no-developer-tools"))
        .arg("--plotium-apply-update")
        .arg(&incoming)
        .arg(&target)
        .arg(&staging)
        .arg("2147483647")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    super::macos::validate(&target, version).unwrap();
    assert!(target.join("Contents/Resources/plotium.mcpb").is_file());
    let smoke = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.github/scripts/package_mcp.py");
    let output = std::process::Command::new("python3")
        .arg(smoke)
        .arg("smoke")
        .arg(target.join("Contents/MacOS/plotium-mcp"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!staging.exists());
}
