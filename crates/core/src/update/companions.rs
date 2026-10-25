//! Fixed release companions travel with the desktop executable on self-update.
use super::UpdateError;
use std::{fs, path::Path};
const DIRECTORY: &str = "companions";
fn names() -> Vec<&'static str> {
    let mut names = vec![
        if cfg!(windows) {
            "plotium-mcp.exe"
        } else {
            "plotium-mcp"
        },
        "MCP-THIRD-PARTY-LICENSES.html",
        "THIRD-PARTY-LICENSES.html",
        "LICENSE",
        "COPYRIGHT.md",
        "README.md",
    ];
    if cfg!(any(windows, target_os = "macos")) {
        names.push("plotium.mcpb");
    }
    names
}
pub(super) fn extract(downloaded: &Path, staging: &Path) -> Result<(), UpdateError> {
    if downloaded
        .extension()
        .is_none_or(|e| !e.eq_ignore_ascii_case("zip"))
    {
        return Ok(());
    }
    let file = fs::File::open(downloaded)?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| UpdateError::Protocol(e.to_string()))?;
    let directory = staging.join(DIRECTORY);
    fs::create_dir_all(&directory)?;
    for name in names() {
        let mut entry = archive
            .by_name(name)
            .map_err(|e| UpdateError::Protocol(format!("Missing release companion {name}: {e}")))?;
        let mut file = fs::File::create(directory.join(name))?;
        std::io::copy(&mut entry, &mut file)?;
        file.sync_all()?;
        #[cfg(unix)]
        if name == "plotium-mcp" {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(0o755))?;
        }
    }
    Ok(())
}
pub(super) fn apply(staging: &Path, target: &Path) -> Result<(), UpdateError> {
    let directory = staging.join(DIRECTORY);
    if !directory.exists() {
        return Ok(());
    }
    let parent = target
        .parent()
        .ok_or_else(|| UpdateError::Protocol("Executable has no parent".into()))?;
    let backups = staging.join("companion-backups");
    fs::create_dir_all(&backups)?;
    let mut installed = Vec::new();
    for name in names() {
        let destination = parent.join(name);
        let existed = destination.exists();
        let outcome = (|| -> std::io::Result<()> {
            if existed {
                fs::copy(&destination, backups.join(name))?;
            }
            let temporary = parent.join(format!(".{name}.update"));
            fs::copy(directory.join(name), &temporary)?;
            // On Windows a running connector cannot be replaced. Fail with the
            // previous file retained, allowing a retry after clients exit.
            fs::rename(&temporary, &destination)?;
            Ok(())
        })();
        if let Err(error) = outcome {
            for (name, existed) in installed.into_iter().rev() {
                let result = if existed {
                    fs::copy(backups.join(name), parent.join(name)).map(|_| ())
                } else {
                    fs::remove_file(parent.join(name))
                };
                if let Err(rollback) = result {
                    eprintln!("Plotium update companion rollback failed: {rollback}");
                }
            }
            return Err(error.into());
        }
        installed.push((name, existed));
    }
    Ok(())
}

#[cfg(test)]
#[path = "companion_tests.rs"]
mod tests;
