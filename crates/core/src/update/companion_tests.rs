use super::*;
use std::io::Write;
fn temp() -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("plotium-companions-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&path).unwrap();
    path
}
#[test]
fn verified_archive_extracts_native_companions_and_replaces_them_with_app() {
    let root = temp();
    let staging = root.join("staging");
    fs::create_dir(&staging).unwrap();
    let archive_path = staging.join("package.zip");
    let mut archive = zip::ZipWriter::new(fs::File::create(&archive_path).unwrap());
    for name in names() {
        archive
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        archive.write_all(format!("new {name}").as_bytes()).unwrap();
    }
    archive.finish().unwrap();
    extract(&archive_path, &staging).unwrap();
    let target = root.join("plotium");
    fs::write(&target, b"old app").unwrap();
    let new_app = staging.join("new-app");
    fs::write(&new_app, b"new app").unwrap();
    for name in names() {
        fs::write(root.join(name), format!("old {name}")).unwrap();
    }
    super::super::install::apply_staged_update(&new_app, &target, &staging).unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"new app");
    for name in names() {
        assert_eq!(
            fs::read(root.join(name)).unwrap(),
            format!("new {name}").as_bytes()
        );
    }
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn failed_companion_swap_restores_app_and_already_replaced_companions() {
    let root = temp();
    let staging = root.join("staging");
    fs::create_dir(&staging).unwrap();
    fs::create_dir(staging.join(DIRECTORY)).unwrap();
    let target = root.join("plotium");
    fs::write(&target, b"old app").unwrap();
    let new_app = staging.join("new-app");
    fs::write(&new_app, b"new app").unwrap();
    for name in names() {
        fs::write(root.join(name), format!("old {name}")).unwrap();
    }
    let connector = names()[0];
    fs::write(staging.join(DIRECTORY).join(connector), b"new connector").unwrap();
    assert!(super::super::install::apply_staged_update(&new_app, &target, &staging).is_err());
    assert_eq!(fs::read(&target).unwrap(), b"old app");
    for name in names() {
        assert_eq!(
            fs::read(root.join(name)).unwrap(),
            format!("old {name}").as_bytes()
        );
    }
    fs::remove_dir_all(root).unwrap();
}
