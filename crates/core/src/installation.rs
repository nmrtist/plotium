//! Resolve the two shipping layouts without consulting the working directory.
use std::path::Path;

/// Contents of a standard macOS application, as distinct from a portable binary.
pub fn macos_bundle_contents(executable: &Path) -> Option<&Path> {
    let macos = executable.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;
    (macos.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && bundle.extension().is_some_and(|ext| ext == "app"))
    .then_some(contents)
}

/// Only macOS application bundles need whole-app replacement by DMG.
pub fn requires_manual_update(executable: &Path) -> bool {
    cfg!(target_os = "macos") && macos_bundle_contents(executable).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinguish_bundle_from_portable_layout_even_with_unicode_and_spaces() {
        let executable = Path::new("/tmp/验收 space/Plotium.app/Contents/MacOS/plotium");
        assert_eq!(
            macos_bundle_contents(executable),
            Some(Path::new("/tmp/验收 space/Plotium.app/Contents"))
        );
        for path in [
            "/tmp/plotium",
            "/tmp/Plotium.app/plotium",
            "/tmp/Contents/MacOS/plotium",
            "/tmp/Plotium.app/Contents/plotium",
        ] {
            assert_eq!(macos_bundle_contents(Path::new(path)), None);
            assert!(!requires_manual_update(Path::new(path)));
        }
        assert_eq!(
            requires_manual_update(executable),
            cfg!(target_os = "macos")
        );
    }
}
