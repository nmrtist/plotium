//! Resolve an explicitly selected acquisition without discovering nearby inputs.
use crate::IoError;
use std::path::{Path, PathBuf};

/// Directory resolution is deliberately narrower than interactive discovery.
/// Resolve NMR bundles to a named entry point before invoking the general reader:
/// its ordinary-directory mode also searches standalone files in that directory.
pub fn resolve_explicit_input(path: &Path) -> Result<PathBuf, IoError> {
    if !path.is_dir() {
        return Ok(path.to_path_buf());
    }
    for marker in ["acqus", "procpar", "procs", "proc2s"] {
        let input = path.join(marker);
        if input.is_file() {
            return Ok(input);
        }
    }
    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("raw"))
        && crate::waters::is_masslynx_raw(path)
    {
        return Ok(path.to_path_buf());
    }
    Err(IoError::Unsupported(format!(
        "Select an explicit data file or atomic acquisition directory, not a discovery folder: {}",
        path.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_directories_resolve_without_discovering_standalone_neighbors() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/nmr");
        assert!(resolve_explicit_input(&root).is_err());
        assert_eq!(
            resolve_explicit_input(&root.join("bruker-1d")).unwrap(),
            root.join("bruker-1d/acqus")
        );
        assert_eq!(
            resolve_explicit_input(&root.join("bruker-1d/pdata/1")).unwrap(),
            root.join("bruker-1d/pdata/1/procs")
        );
        let file = root.join("jeol-diffusion.jdf");
        assert_eq!(resolve_explicit_input(&file).unwrap(), file);
    }
}
