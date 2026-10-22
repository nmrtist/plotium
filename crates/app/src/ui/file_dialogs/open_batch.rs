use std::path::PathBuf;

use plotium_core::state::{ImportDiscovery, PlotiumApp};

use super::recent::{self, ClassifiedOpenPath};

pub(crate) fn queue_data_paths(app: &mut PlotiumApp, paths: Vec<PathBuf>) {
    let Some(recent) = paths.first().cloned() else {
        return;
    };
    let discover = move || Ok(discover_paths(paths, super::discovery::discover_folder));
    if app.session.ui.table_import_preview.is_some() || super::image_import::has_active_jobs() {
        app.queue_data_import_discovery(recent, discover);
    } else {
        app.queue_user_data_import_discovery(recent, discover);
    }
}

fn discover_paths(
    paths: Vec<PathBuf>,
    mut folder: impl FnMut(&std::path::Path) -> Result<Vec<PathBuf>, String>,
) -> ImportDiscovery {
    let mut discovered = ImportDiscovery::default();
    for path in paths {
        if path.is_dir() {
            match folder(&path) {
                Ok(files) => discovered.paths.extend(files),
                Err(error) => discovered.failures.push((path, error)),
            }
        } else {
            discovered.paths.push(path);
        }
    }
    discovered
}

#[test]
fn folder_discovery_failure_retains_other_sources_and_its_own_path() {
    let folder = tempfile::tempdir().unwrap();
    let first = PathBuf::from("first.dx");
    let last = PathBuf::from("last.dx");
    let discovered = discover_paths(
        vec![first.clone(), folder.path().to_owned(), last.clone()],
        |_| Err("Permission denied".into()),
    );
    assert_eq!(discovered.paths, vec![first, last]);
    assert_eq!(
        discovered.failures,
        vec![(folder.path().to_owned(), "Permission denied".into())]
    );
}

pub(crate) fn open_paths(app: &mut PlotiumApp, paths: Vec<PathBuf>) {
    app.cancel_import_open_intent();
    let mut queued = Vec::new();
    for path in paths {
        match recent::classify_open_path(&path) {
            Ok(ClassifiedOpenPath::DataFile | ClassifiedOpenPath::Folder)
                if !plotium_io::archive::is_zip(&path) =>
            {
                queued.push(path);
            }
            Ok(classified) => recent::dispatch_classified_path(app, &path, classified),
            Err(error) => recent::record_open_path_failure(app, &path, error),
        }
    }
    queue_data_paths(app, queued);
}
