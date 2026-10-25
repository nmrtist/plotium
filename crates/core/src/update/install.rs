//! Preparing and applying a verified update after the GUI process exits.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use super::UpdateError;

const OLD_SUFFIX: &str = ".old";
const HELPER_ARG: &str = "--plotium-apply-update";
const RELAUNCH_MARKER: &str = "relaunch";

/// Everything the shell needs to hand the swap to a temporary helper after
/// the GUI has fully exited. The helper is a copy of the current executable,
/// so it does not lock the installed target on Windows.
#[derive(Clone, Debug, PartialEq)]
pub struct InstallPlan {
    helper: PathBuf,
    new_binary: PathBuf,
    target: PathBuf,
    staging: PathBuf,
}

/// Extract the verified artifact and copy a standalone helper into its unique
/// staging directory. No installed file is touched while Plotium is running.
pub fn prepare(downloaded: &Path, version: &str) -> Result<InstallPlan, UpdateError> {
    let executable = std::env::current_exe()?;
    let staging = downloaded
        .parent()
        .ok_or_else(|| UpdateError::Protocol("update has no staging directory".into()))?
        .to_owned();
    #[cfg(target_os = "macos")]
    let (target, new_binary) = if crate::installation::uses_bundle_updates(&executable) {
        let incoming = super::bundle::prepare(downloaded, &executable, version)?;
        let target = crate::installation::macos_bundle_contents(&executable)
            .and_then(Path::parent)
            .ok_or_else(|| UpdateError::Protocol("invalid application bundle".into()))?;
        (target.to_owned(), incoming)
    } else {
        prepare_portable(downloaded, &executable, &staging)?
    };
    #[cfg(not(target_os = "macos"))]
    let (target, new_binary) = {
        let _ = version;
        prepare_portable(downloaded, &executable, &staging)?
    };
    let helper_name = if cfg!(windows) {
        "plotium-update-helper.exe"
    } else {
        "plotium-update-helper"
    };
    let helper = staging.join(helper_name);
    std::fs::copy(&executable, &helper)?;
    #[cfg(unix)]
    set_executable(&helper)?;
    Ok(InstallPlan {
        helper,
        new_binary,
        target,
        staging,
    })
}

fn prepare_portable(
    downloaded: &Path,
    target: &Path,
    staging: &Path,
) -> Result<(PathBuf, PathBuf), UpdateError> {
    require_portable_target(target)?;
    let new_binary = extract_binary(downloaded, target)?;
    super::companions::extract(downloaded, staging)?;
    Ok((target.to_owned(), new_binary))
}

impl InstallPlan {
    /// Start the detached helper as the final action after the GUI loop ends.
    pub fn launch(self, relaunch: bool) -> Result<(), UpdateError> {
        if relaunch {
            std::fs::write(self.staging.join(RELAUNCH_MARKER), [])?;
        }
        Command::new(&self.helper)
            .arg(HELPER_ARG)
            .arg(&self.new_binary)
            .arg(&self.target)
            .arg(&self.staging)
            .arg(std::process::id().to_string())
            .spawn()
            .map(|_| ())
            .map_err(UpdateError::Io)
    }

    /// Remove a prepared operation that is no longer eligible to install.
    pub(super) fn discard(self) {
        let _ = std::fs::remove_dir_all(self.staging);
    }
}

/// Run helper mode when invoked by [`InstallPlan::launch`]. Returns `None` for
/// a normal GUI invocation and an exit code after a helper invocation.
pub fn run_helper_from_args() -> Option<i32> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new(HELPER_ARG)) {
        return None;
    }
    let Some(new_binary) = args.next().map(PathBuf::from) else {
        return Some(2);
    };
    let Some(target) = args.next().map(PathBuf::from) else {
        return Some(2);
    };
    let Some(staging) = args.next().map(PathBuf::from) else {
        return Some(2);
    };

    #[cfg(target_os = "macos")]
    let result = args
        .next()
        .ok_or_else(|| UpdateError::Protocol("missing updater parent process".into()))
        .and_then(|pid| super::bundle::wait_for_exit(&pid))
        .and_then(|()| apply_staged_update(&new_binary, &target, &staging));
    #[cfg(not(target_os = "macos"))]
    let result = apply_staged_update(&new_binary, &target, &staging);

    Some(match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("Plotium update failed: {error}");
            record_failure(&error);
            1
        }
    })
}

pub(super) fn apply_staged_update(
    new_binary: &Path,
    target: &Path,
    staging: &Path,
) -> Result<(), UpdateError> {
    #[cfg(target_os = "macos")]
    if target
        .extension()
        .is_some_and(|extension| extension == "app")
    {
        super::bundle::apply(new_binary, target, staging)?;
        let relaunch = staging.join(RELAUNCH_MARKER).exists();
        if let Some(message) = finish_bundle_update(staging, || {
            if relaunch {
                super::bundle::relaunch(target)?;
            }
            Ok(())
        }) {
            record_notice("update-warning.txt", &message);
        }
        return Ok(());
    }
    // The parent launched us only after its GUI loop returned, but Windows
    // can retain the image lock briefly while the process finishes unwinding.
    let result = (0..100).find_map(|_| match swap_binary(new_binary, target) {
        Ok(()) => Some(Ok(())),
        Err(error) => {
            std::thread::sleep(Duration::from_millis(100));
            if target.exists() {
                None
            } else {
                Some(Err(error))
            }
        }
    });
    result.unwrap_or_else(|| {
        Err(UpdateError::Protocol(
            "timed out waiting to replace the running executable".into(),
        ))
    })?;

    if let Err(error) = super::companions::apply(staging, target) {
        if let Err(rollback) =
            std::fs::remove_file(target).and_then(|()| std::fs::rename(old_path(target), target))
        {
            eprintln!("Plotium executable rollback failed: {rollback}");
        }
        return Err(error);
    }
    let relaunch = staging.join(RELAUNCH_MARKER).exists();
    // Remove everything except a still-running Windows helper. That final
    // file and directory are removed by `cleanup_after_restart` next launch.
    let running_helper = std::env::current_exe().ok();
    if let Ok(entries) = std::fs::read_dir(staging) {
        for entry in entries.flatten() {
            let path = entry.path();
            if running_helper.as_ref() == Some(&path) {
                continue;
            }
            if path.is_dir() {
                let _ = std::fs::remove_dir_all(path);
            } else {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    let _ = std::fs::remove_dir(staging);
    if relaunch {
        Command::new(target).spawn().map_err(UpdateError::Io)?;
    }
    Ok(())
}

/// Replacement has committed. Cleanup must not prevent a requested relaunch,
/// and neither failure means the installed version should be downloaded again.
#[cfg(any(target_os = "macos", test))]
fn finish_bundle_update(
    staging: &Path,
    relaunch: impl FnOnce() -> Result<(), UpdateError>,
) -> Option<String> {
    let mut warnings = Vec::new();
    if let Err(error) = std::fs::remove_dir_all(staging) {
        warnings.push(format!("could not clean temporary files: {error}"));
    }
    if let Err(error) = relaunch() {
        warnings.push(format!(
            "could not reopen Plotium; open it manually: {error}"
        ));
    }
    (!warnings.is_empty()).then(|| format!("Update installed, but {}", warnings.join("; ")))
}

#[cfg(test)]
#[test]
fn committed_bundle_update_relaunches_despite_cleanup_failure_and_reports_warnings() {
    let root = tempfile::tempdir().unwrap();
    let staging = root.path().join("staging");
    // A non-directory forces cleanup to fail on every supported platform.
    std::fs::write(&staging, b"blocked cleanup").unwrap();
    let mut relaunched = false;
    let warning = finish_bundle_update(&staging, || {
        relaunched = true;
        Ok(())
    })
    .unwrap();
    assert!(relaunched);
    assert!(warning.starts_with("Update installed, but"));
    assert!(warning.contains("could not clean temporary files"));
    std::fs::remove_file(&staging).unwrap();
    std::fs::create_dir(&staging).unwrap();
    let warning = finish_bundle_update(&staging, || {
        Err(UpdateError::Installation("launch blocked".into()))
    })
    .unwrap();
    assert!(!staging.exists());
    assert!(warning.contains("open it manually"));
    std::fs::create_dir(&staging).unwrap();
    assert!(finish_bundle_update(&staging, || Ok(())).is_none());
}

/// Transactionally replace `target`, rolling back if the new copy fails.
pub(super) fn swap_binary(new_binary: &Path, target: &Path) -> Result<(), UpdateError> {
    require_portable_target(target)?;
    let old = old_path(target);
    let _ = std::fs::remove_file(&old);
    std::fs::rename(target, &old)?;
    let install = install_binary(new_binary, target);
    if let Err(error) = install {
        let _ = std::fs::remove_file(target);
        let _ = std::fs::rename(&old, target);
        return Err(error.into());
    }
    Ok(())
}

fn install_binary(source: &Path, target: &Path) -> std::io::Result<()> {
    std::fs::copy(source, target)?;
    #[cfg(unix)]
    set_executable(target)?;
    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
    Ok(())
}

fn old_path(target: &Path) -> PathBuf {
    let name = target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "plotium".to_owned());
    target.with_file_name(format!("{name}{OLD_SUFFIX}"))
}

/// Remove the old executable and helper-only directories left by completed
/// updates. Active downloads and prepared updates contain additional files,
/// so another running Plotium instance can never lose its staging directory.
pub fn cleanup_after_restart() {
    if let Ok(current) = std::env::current_exe()
        && !crate::installation::uses_bundle_updates(&current)
    {
        let _ = std::fs::remove_file(old_path(&current));
    }
    cleanup_completed_staging(&std::env::temp_dir());
}

fn require_portable_target(target: &Path) -> Result<(), UpdateError> {
    if crate::installation::macos_bundle_contents(target).is_some() {
        return Err(UpdateError::Protocol(
            "A Plotium.app update must replace the complete bundle from a verified DMG.".into(),
        ));
    }
    Ok(())
}

/// Post-exit failures cannot reach the closed GUI; retain them for next launch.
pub fn record_failure(error: &UpdateError) {
    record_notice("update-failure.txt", &error.to_string());
}

fn record_notice(file: &str, message: &str) {
    eprintln!("Plotium update: {message}");
    let result = (|| -> std::io::Result<()> {
        let directory = crate::settings::config_dir()
            .ok_or_else(|| std::io::Error::other("no settings directory"))?;
        std::fs::create_dir_all(&directory)?;
        std::fs::write(directory.join(file), message)
    })();
    if let Err(error) = result {
        eprintln!("Could not record Plotium update result: {error}");
    }
}

pub(super) fn take_failure() -> Option<String> {
    take_notice("update-failure.txt")
}

pub(super) fn take_warning() -> Option<String> {
    take_notice("update-warning.txt")
}

fn take_notice(file: &str) -> Option<String> {
    let path = crate::settings::config_dir()?.join(file);
    match std::fs::read_to_string(&path) {
        Ok(message) => {
            if let Err(error) = std::fs::remove_file(path) {
                eprintln!("Could not clear Plotium update failure: {error}");
            }
            Some(message)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => Some(format!(
            "Could not read the previous update result: {error}"
        )),
    }
}

#[cfg(all(test, target_os = "macos"))]
#[test]
fn bundle_swap_is_rejected_before_touching_the_installed_executable() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("Plotium.app/Contents/MacOS/plotium");
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    std::fs::write(&target, b"installed executable").unwrap();
    let source = temp.path().join("update");
    std::fs::write(&source, b"new executable").unwrap();
    assert!(swap_binary(&source, &target).is_err());
    assert_eq!(std::fs::read(&target).unwrap(), b"installed executable");
    assert!(!old_path(&target).exists());
}

pub(super) fn cleanup_completed_staging(temp_dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(temp_dir) {
        for entry in entries.flatten() {
            let is_staging = entry
                .file_name()
                .to_string_lossy()
                .starts_with("plotium-update-");
            if !is_staging {
                continue;
            }
            let path = entry.path();
            let Ok(mut contents) = std::fs::read_dir(&path) else {
                continue;
            };
            let Some(Ok(helper)) = contents.next() else {
                continue;
            };
            let helper_name = helper.file_name();
            let is_helper = helper_name == "plotium-update-helper"
                || helper_name == "plotium-update-helper.exe";
            if is_helper && contents.next().is_none() && std::fs::remove_file(helper.path()).is_ok()
            {
                let _ = std::fs::remove_dir(path);
            }
        }
    }
}

/// Produce a plain executable from a downloaded archive.
pub(super) fn extract_binary(downloaded: &Path, installed: &Path) -> Result<PathBuf, UpdateError> {
    if downloaded
        .extension()
        .is_none_or(|extension| !extension.eq_ignore_ascii_case("zip"))
    {
        return Ok(downloaded.to_owned());
    }
    let wanted = installed
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let file = std::fs::File::open(downloaded)?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|error| UpdateError::Protocol(error.to_string()))?;
    let names: Vec<String> = archive
        .file_names()
        .map(|name| name.map(|name| name.into_owned()))
        .collect::<Result<_, _>>()
        .map_err(|error| UpdateError::Protocol(error.to_string()))?;
    let entry_name = names
        .iter()
        .find(|name| {
            Path::new(name)
                .file_name()
                .is_some_and(|entry| entry.to_string_lossy() == wanted)
        })
        .or_else(|| (names.len() == 1).then(|| &names[0]))
        .ok_or_else(|| UpdateError::Protocol(format!("update archive doesn't contain {wanted:?}")))?
        .clone();
    let mut entry = archive
        .by_name(&entry_name)
        .map_err(|error| UpdateError::Protocol(error.to_string()))?;
    let out_path = downloaded.with_extension("extracted");
    let mut out = std::fs::File::create(&out_path)?;
    std::io::copy(&mut entry, &mut out)?;
    Ok(out_path)
}
