//! Whole-bundle replacement keeps code signatures, resources and MCP in sync.
#[cfg(any(target_os = "macos", test))]
use std::path::Path;

#[cfg(any(target_os = "macos", test))]
use super::UpdateError;

/// Shipping DMGs contain a single 64-bit Mach-O executable per binary.
/// Read its header directly: lipo is a developer-tool shim, not a runtime tool.
#[cfg(any(target_os = "macos", test))]
fn verify_architecture(binary: &Path, expected_cpu: u32) -> Result<(), UpdateError> {
    use std::io::Read;

    let mut header = [0_u8; 32];
    std::fs::File::open(binary)?.read_exact(&mut header)?;
    let cpu = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
    let file_type = u32::from_le_bytes([header[12], header[13], header[14], header[15]]);
    // MH_MAGIC_64 (little endian) and MH_EXECUTE; codesign validates the rest.
    if header[..4] != [0xcf, 0xfa, 0xed, 0xfe] || cpu != expected_cpu || file_type != 2 {
        return Err(UpdateError::Installation(format!(
            "Update executable {} is not a matching 64-bit macOS application",
            binary.display()
        )));
    }
    Ok(())
}

/// Both renames stay on the installation filesystem. Keep the old app until
/// the installed copy passes validation; never modify a running Mach-O in place.
#[cfg(any(target_os = "macos", test))]
fn replace(
    incoming: &Path,
    target: &Path,
    validate: impl FnOnce(&Path) -> Result<(), UpdateError>,
) -> Result<(), UpdateError> {
    let parent = target
        .parent()
        .ok_or_else(|| UpdateError::Installation("application has no parent directory".into()))?;
    let backup = tempfile::Builder::new()
        .prefix(".plotium-rollback-")
        .tempdir_in(parent)?;
    let old = backup.path().join("Plotium.app");
    std::fs::rename(target, &old)?;
    let result = std::fs::rename(incoming, target)
        .map_err(UpdateError::Io)
        .and_then(|()| validate(target));
    if let Err(error) = result {
        if target.exists() {
            // Preserve the rejected copy outside the installation before rollback.
            if let Err(move_error) = std::fs::rename(target, backup.path().join("rejected.app")) {
                let saved = backup.keep();
                return Err(UpdateError::Installation(format!(
                    "{error}; could not move rejected app: {move_error}; previous app is at {}",
                    saved.join("Plotium.app").display()
                )));
            }
        }
        if let Err(rollback) = std::fs::rename(&old, target) {
            let saved = backup.keep();
            return Err(UpdateError::Installation(format!(
                "{error}; rollback failed: {rollback}; previous app is at {}",
                saved.join("Plotium.app").display()
            )));
        }
        return Err(error);
    }
    if let Err(error) = backup.close() {
        eprintln!("Plotium update installed; could not remove the previous app backup: {error}");
    }
    Ok(())
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use std::process::Command;

    fn run(command: &mut Command) -> Result<String, UpdateError> {
        let output = command.output()?;
        if !output.status.success() {
            return Err(UpdateError::Installation(format!(
                "{} failed: {}",
                command.get_program().to_string_lossy(),
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }

    fn plist(app: &Path, key: &str) -> Result<String, UpdateError> {
        run(Command::new("/usr/libexec/PlistBuddy")
            .arg("-c")
            .arg(format!("Print :{key}"))
            .arg(app.join("Contents/Info.plist")))
    }

    pub(super) fn validate(app: &Path, version: &str) -> Result<(), UpdateError> {
        if plist(app, "CFBundleIdentifier")? != "space.nmrtist.plotium"
            || plist(app, "CFBundleExecutable")? != "plotium"
            || plist(app, "PlotiumCargoVersion")? != version
        {
            return Err(UpdateError::Installation(
                "DMG contains a different application or version".into(),
            ));
        }
        // CPU_ARCH_ABI64 combined with CPU_TYPE_ARM or CPU_TYPE_X86.
        let cpu = if cfg!(target_arch = "aarch64") {
            0x0100_000c
        } else {
            0x0100_0007
        };
        for name in ["plotium", "plotium-mcp"] {
            super::verify_architecture(&app.join("Contents/MacOS").join(name), cpu)?;
        }
        run(Command::new("/usr/bin/codesign")
            .args(["--verify", "--deep", "--strict"])
            .arg(app))?;
        Ok(())
    }

    pub(crate) fn prepare(
        dmg: &Path,
        executable: &Path,
        version: &str,
    ) -> Result<std::path::PathBuf, UpdateError> {
        let contents = crate::installation::macos_bundle_contents(executable).ok_or_else(|| {
            UpdateError::Installation("expected an installed application bundle".into())
        })?;
        let app = contents
            .parent()
            .ok_or_else(|| UpdateError::Installation("application has no parent".into()))?;
        let parent = app.parent().ok_or_else(|| {
            UpdateError::Installation("application has no installation directory".into())
        })?;
        if app.to_string_lossy().contains("/AppTranslocation/") || app.starts_with("/Volumes") {
            return Err(UpdateError::Installation("Move Plotium.app to Applications or a writable local folder and relaunch it before updating.".into()));
        }
        // Discover read-only/permission failures while the GUI can still explain them.
        tempfile::Builder::new().prefix(".plotium-write-check-").tempdir_in(parent)
            .map_err(|error| UpdateError::Installation(format!(
                "Allow Plotium to update apps in System Settings, or move it to a writable folder and retry: {error}"
            )))?.close()?;
        let staging = dmg
            .parent()
            .ok_or_else(|| UpdateError::Installation("DMG has no staging directory".into()))?;
        let mount = tempfile::Builder::new()
            .prefix("plotium-update-mount-")
            .tempdir()?;
        run(Command::new("/usr/bin/hdiutil")
            .args([
                "attach",
                "-readonly",
                "-nobrowse",
                "-noautoopen",
                "-mountpoint",
            ])
            .arg(mount.path())
            .arg(dmg))?;
        let incoming = staging.join("Plotium.app");
        let result = (|| {
            let source = mount.path().join("Plotium.app");
            validate(&source, version)?;
            run(Command::new("/usr/bin/ditto").arg(&source).arg(&incoming))?;
            validate(&incoming, version)?;
            std::fs::write(staging.join("bundle-version"), version)?;
            Ok(incoming)
        })();
        let detach = run(Command::new("/usr/bin/hdiutil")
            .arg("detach")
            .arg(mount.path()));
        // Do not remove a mount directory if the system could not unmount it.
        if let Err(error) = detach {
            let retained = mount.keep();
            return Err(UpdateError::Installation(format!(
                "{error}; mounted update retained at {}",
                retained.display()
            )));
        }
        result
    }

    pub(crate) fn apply(incoming: &Path, target: &Path, staging: &Path) -> Result<(), UpdateError> {
        let version = std::fs::read_to_string(staging.join("bundle-version"))?;
        validate(incoming, &version)?;
        let parent = target
            .parent()
            .ok_or_else(|| UpdateError::Installation("application has no parent".into()))?;
        let local = tempfile::Builder::new()
            .prefix(".plotium-incoming-")
            .tempdir_in(parent)?;
        let copy = local.path().join("Plotium.app");
        run(Command::new("/usr/bin/ditto").arg(incoming).arg(&copy))?;
        validate(&copy, &version)?;
        super::replace(&copy, target, |app| validate(app, &version))?;
        if let Err(error) = local.close() {
            eprintln!(
                "Plotium update installed; could not remove the incoming app directory: {error}"
            );
        }
        Ok(())
    }

    pub(crate) fn relaunch(app: &Path) -> Result<(), UpdateError> {
        run(Command::new("/usr/bin/open").arg("-n").arg(app))?;
        Ok(())
    }

    /// A Mach-O must not be replaced until the originating process fully exits.
    pub(crate) fn wait_for_exit(pid: &std::ffi::OsStr) -> Result<(), UpdateError> {
        let pid: u32 = pid
            .to_string_lossy()
            .parse()
            .map_err(|_| UpdateError::Installation("invalid updater parent process".into()))?;
        if pid == 0 {
            return Err(UpdateError::Installation(
                "invalid updater parent process".into(),
            ));
        }
        for _ in 0..300 {
            if !Command::new("/bin/kill")
                .arg("-0")
                .arg(pid.to_string())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()?
                .success()
            {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        Err(UpdateError::Installation(
            "Plotium did not exit; the installed app was left unchanged".into(),
        ))
    }
}

#[cfg(target_os = "macos")]
pub(crate) use macos::{apply, prepare, relaunch, wait_for_exit};

#[cfg(test)]
#[path = "bundle_tests.rs"]
mod tests;
