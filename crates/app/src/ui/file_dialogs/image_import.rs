use plotium_core::actions::{Action, PanelState};
use plotium_core::operation::{
    Diagnostic, DiagnosticCode, OperationId, OperationKind, OperationReport, Severity,
};
use plotium_core::state::{
    AssetId, AssetRecord, CanvasDocument, CanvasId, CanvasObject, CanvasObjectKind, ContentId,
    ObjectFrame, PanelId, PlotiumApp, RasterImageContent,
};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};

mod consent;
pub(crate) use consent::large_image_consent_window;
mod clipboard;
pub(crate) use clipboard::paste_clipboard_image;
mod geometry;
use geometry::{image_frame, physical_image_size_mm};
#[cfg(test)]
use geometry::{initial_image_size, natural_vector_size};
mod prepare;
#[cfg(test)]
use prepare::prepare_bytes;
use prepare::worker;
mod jobs;
pub(crate) use jobs::poll;
mod new_pages;
mod picker;
pub(crate) use picker::{
    import_image_paths, import_images, import_images_first_frame, import_images_without_metadata,
    import_tiff_pages, replace_selected_image,
};
mod proxy;
use proxy::{insert_proxy, rebuild_preview};
pub(crate) use proxy::{poll_rebuilds, request_thumbnail};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ImportImageState {
    Queued,
    Probing,
    AwaitingLargeImageConsent,
    DecodingProxy,
    ReadyToCommit,
    Committed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug)]
pub(crate) struct ImportImageRequest {
    pub paths: Vec<PathBuf>,
    pub payloads: Vec<(String, Vec<u8>)>,
    pub target: ImportImageTarget,
    pub allow_first_frame: bool,
    pub strip_metadata: bool,
    pub import_all_tiff_pages: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ImportImageTarget {
    NewPages,
    Canvas {
        canvas: CanvasId,
        position: Option<[f32; 2]>,
    },
    Panel {
        canvas: CanvasId,
        panel: PanelId,
        position: Option<[f32; 2]>,
    },
    Replace {
        canvas: CanvasId,
        content: ContentId,
    },
}

struct Candidate {
    basename: String,
    bytes: Arc<Vec<u8>>,
    sha256: [u8; 32],
    format: String,
    pixel_size: [u32; 2],
    auto_page_size_mm: [f32; 2],
    page_index: u32,
    warning: Option<String>,
    preview: plotium_io::image::ProxyImage,
}

impl Candidate {
    fn display_name(&self) -> &str {
        if self.page_index > 0 {
            &self.basename
        } else {
            Path::new(&self.basename)
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("Image")
        }
    }
}

#[derive(Clone, Copy)]
struct ImportOptions {
    allow_first_frame: bool,
    strip_metadata: bool,
    import_all_tiff_pages: bool,
}

#[derive(Debug)]
struct Failure {
    basename: String,
    detected_format: String,
    stage: &'static str,
    reason: String,
    remedy: &'static str,
}

enum WorkerEvent {
    State(ImportImageState),
    Candidate(Result<Candidate, Failure>),
    Finished,
}

struct ImportJob {
    project_context: plotium_core::state::ProjectContextId,
    request: ImportImageRequest,
    operation: OperationId,
    state: ImportImageState,
    receiver: mpsc::Receiver<WorkerEvent>,
    cancelled: Arc<AtomicBool>,
    consent: mpsc::Sender<bool>,
    staged: Vec<Result<Candidate, Failure>>,
}

#[derive(Default)]
struct ImageImportManager {
    jobs: Vec<ImportJob>,
}

thread_local! {
    static MANAGER: RefCell<ImageImportManager> = RefCell::new(ImageImportManager::default());
}

pub(crate) fn enqueue(app: &mut PlotiumApp, mut request: ImportImageRequest) {
    let operation = app.session.begin_operation();
    let (sender, receiver) = mpsc::sync_channel(2);
    let (consent, worker_consent) = mpsc::channel();
    let cancelled = Arc::new(AtomicBool::new(false));
    let worker_cancelled = Arc::clone(&cancelled);
    let paths = std::mem::take(&mut request.paths);
    let payloads = std::mem::take(&mut request.payloads);
    let options = ImportOptions {
        allow_first_frame: request.allow_first_frame,
        strip_metadata: request.strip_metadata,
        import_all_tiff_pages: request.import_all_tiff_pages,
    };
    std::thread::spawn(move || {
        worker(
            paths,
            payloads,
            worker_cancelled,
            sender,
            worker_consent,
            options,
        )
    });
    MANAGER.with(|manager| {
        manager.borrow_mut().jobs.push(ImportJob {
            project_context: app.session.project_context,
            request,
            operation,
            state: ImportImageState::Queued,
            receiver,
            cancelled,
            consent,
            staged: Vec::new(),
        });
    });
    app.session.status = "Image import queued.".to_owned();
}

pub(crate) fn has_active_jobs() -> bool {
    MANAGER.with(|manager| !manager.borrow().jobs.is_empty())
}

pub(crate) fn cancel_all(app: &mut PlotiumApp) {
    MANAGER.with(|manager| {
        for job in &mut manager.borrow_mut().jobs {
            job.cancelled.store(true, Ordering::Relaxed);
            let _ = job.consent.send(false);
            job.state = ImportImageState::Cancelled;
        }
    });
    app.session.status = "Cancelling image import…".to_owned();
}

fn commit(app: &mut PlotiumApp, job: &mut ImportJob, results: Vec<Result<Candidate, Failure>>) {
    if job.request.target == ImportImageTarget::NewPages {
        new_pages::commit(app, job, results);
        return;
    }

    let target_canvas = match job.request.target {
        ImportImageTarget::Canvas { canvas, .. }
        | ImportImageTarget::Panel { canvas, .. }
        | ImportImageTarget::Replace { canvas, .. } => canvas,
        ImportImageTarget::NewPages => unreachable!("new-page imports returned above"),
    };
    let Some(ci) = app.doc.canvas_index(target_canvas) else {
        job.state = ImportImageState::Cancelled;
        record_failures(
            app,
            job.operation,
            0,
            vec![failure(
                "<batch>",
                "unknown",
                "commit",
                "target page was deleted".to_owned(),
                "Choose a page and import the images again.",
            )],
        );
        return;
    };
    let (target_panel, drop_position, replace_content) = match job.request.target {
        ImportImageTarget::Canvas { position, .. } => (None, position, None),
        ImportImageTarget::Panel {
            panel, position, ..
        } => {
            let Some(target) = app.doc.canvases[ci].panel(panel) else {
                fail_stale_panel(app, job, "target panel was deleted");
                return;
            };
            if target.locked {
                fail_stale_panel(app, job, "target panel became locked");
                return;
            }
            (Some(panel), position, None)
        }
        ImportImageTarget::Replace { content, .. } => (None, None, Some(content)),
        ImportImageTarget::NewPages => unreachable!("new-page imports returned above"),
    };
    let page_before = app.doc.canvases[ci].clone();
    let before = PanelState::of(&page_before);
    let mut page = page_before.clone();
    let mut failures = Vec::new();
    let mut imported = 0usize;
    let mut warnings = Vec::new();
    let mut staged_assets: Vec<AssetRecord> = Vec::new();
    for result in results {
        let candidate = match result {
            Ok(candidate) => candidate,
            Err(error) => {
                failures.push(error);
                continue;
            }
        };
        let asset = app
            .doc
            .assets
            .iter()
            .find_map(|(id, asset)| (asset.sha256 == candidate.sha256).then_some(*id))
            .or_else(|| {
                staged_assets
                    .iter()
                    .find_map(|asset| (asset.sha256 == candidate.sha256).then_some(asset.id))
            })
            .unwrap_or_else(AssetId::new);
        if let Some(content) = replace_content {
            if let Some(item) = page.object_mut(content)
                && let CanvasObjectKind::RasterImage(image) = &mut item.kind
            {
                image.asset = asset;
                image.page_index = candidate.page_index;
                if !app.doc.assets.contains_key(&asset)
                    && !staged_assets.iter().any(|record| record.id == asset)
                {
                    staged_assets.push(AssetRecord {
                        id: asset,
                        sha256: candidate.sha256,
                        format: candidate.format.clone(),
                        pixel_size: candidate.pixel_size,
                        bytes: candidate.bytes.as_ref().clone(),
                    });
                }
                insert_proxy(
                    app,
                    candidate.sha256,
                    candidate.page_index,
                    candidate.preview,
                );
                imported += 1;
                if let Some(warning) = candidate.warning {
                    warnings.push((candidate.basename, candidate.format, warning));
                }
                continue;
            }
            failures.push(failure(
                &candidate.basename,
                &candidate.format,
                "commit",
                "the image being replaced no longer exists".to_owned(),
                "Select an image and choose Replace Image again.",
            ));
            continue;
        }
        let id = page.allocate_object_id();
        let base = drop_position.unwrap_or([12.0, 12.0]);
        let offset = imported as f32 * 6.0;
        let position = [base[0] + offset, base[1] + offset];
        let frame = image_frame(&page, target_panel, &candidate, position);
        page.objects.push(CanvasObject {
            id,
            name: candidate.display_name().to_owned(),
            frame,
            locked: false,
            visible: true,
            kind: CanvasObjectKind::RasterImage({
                let mut image = RasterImageContent::new(asset);
                image.page_index = candidate.page_index;
                image
            }),
        });
        if let Some(panel) = target_panel.and_then(|panel| page.panel_mut(panel)) {
            panel.item_order.push(id);
        }
        if !app.doc.assets.contains_key(&asset)
            && !staged_assets.iter().any(|record| record.id == asset)
        {
            staged_assets.push(AssetRecord {
                id: asset,
                sha256: candidate.sha256,
                format: candidate.format.clone(),
                pixel_size: candidate.pixel_size,
                bytes: candidate.bytes.as_ref().clone(),
            });
        }
        insert_proxy(
            app,
            candidate.sha256,
            candidate.page_index,
            candidate.preview,
        );
        if let Some(warning) = candidate.warning {
            warnings.push((candidate.basename, candidate.format, warning));
        }
        imported += 1;
    }
    if imported > 0 {
        let mut actions = Vec::new();
        actions.extend(staged_assets.into_iter().map(|asset| Action::SetAsset {
            id: asset.id,
            before: None,
            after: Some(asset),
        }));
        actions.push(Action::ReplacePanelState {
            canvas: ci,
            before,
            after: PanelState::of(&page),
        });
        match app.try_execute_action(Action::Composite(actions)) {
            Ok(()) => job.state = ImportImageState::Committed,
            Err(error) => {
                imported = 0;
                job.state = ImportImageState::Failed;
                failures.push(failure(
                    "<batch>",
                    "unknown",
                    "commit",
                    error.to_string(),
                    "Retry the import; if it still fails, review the diagnostic history.",
                ));
            }
        }
    } else {
        job.state = ImportImageState::Failed;
    }
    record_result(app, job.operation, imported, failures, warnings);
}

fn fail_stale_panel(app: &mut PlotiumApp, job: &mut ImportJob, reason: &str) {
    job.state = ImportImageState::Cancelled;
    record_failures(
        app,
        job.operation,
        0,
        vec![failure(
            "<batch>",
            "unknown",
            "commit",
            reason.to_owned(),
            "Select an unlocked Panel and import the images again.",
        )],
    );
}

fn failure(
    basename: &str,
    detected: &str,
    stage: &'static str,
    reason: String,
    remedy: &'static str,
) -> Failure {
    Failure {
        basename: basename.to_owned(),
        detected_format: detected.to_owned(),
        stage,
        reason,
        remedy,
    }
}

fn record_result(
    app: &mut PlotiumApp,
    operation: OperationId,
    imported: usize,
    failures: Vec<Failure>,
    warnings: Vec<(String, String, String)>,
) {
    let failed = failures.len();
    let summary = format!("Imported {imported} image(s); {failed} failed.");
    if imported == 0 {
        record_failures(app, operation, imported, failures);
        return;
    }
    let mut report = if failed == 0 && warnings.is_empty() {
        OperationReport::success(operation, OperationKind::ImageImport, summary, ())
    } else {
        OperationReport::warning(operation, OperationKind::ImageImport, summary, ())
    };
    for failure in failures {
        report = report.with_diagnostic(diagnostic(failure));
    }
    for (file, detected_format, warning) in warnings {
        report = report.with_diagnostic(
            Diagnostic::new(
                Severity::Warning,
                DiagnosticCode::ImageImportWarning,
                warning,
            )
            .with_source("app.image_import")
            .with_context("file", file)
            .with_context("detected_format", detected_format)
            .with_context("stage", "proxy_or_commit")
            .with_context("reason", "safe fallback used")
            .with_context(
                "next_step",
                "Review the placed image and retry with a smaller source if needed.",
            ),
        );
    }
    app.session.record_operation(report);
}

fn record_failures(
    app: &mut PlotiumApp,
    operation: OperationId,
    imported: usize,
    failures: Vec<Failure>,
) {
    let mut failures = failures.into_iter();
    let first = failures.next().unwrap_or_else(|| {
        failure(
            "<batch>",
            "unknown",
            "commit",
            "no image was ready to commit".to_owned(),
            "Choose another image and retry.",
        )
    });
    let mut report = OperationReport::<()>::failure(
        operation,
        OperationKind::ImageImport,
        format!("Imported {imported} image(s); import failed."),
        diagnostic(first),
    );
    for failure in failures {
        report = report.with_diagnostic(diagnostic(failure));
    }
    app.session.record_operation(report);
}

fn diagnostic(error: Failure) -> Diagnostic {
    Diagnostic::new(
        Severity::Error,
        DiagnosticCode::ImageImportFailed,
        "Image import failed; the document and undo history were not changed for this file.",
    )
    .with_source("app.image_import")
    .with_context("file", error.basename)
    .with_context("detected_format", error.detected_format)
    .with_context("stage", error.stage)
    .with_context("reason", error.reason)
    .with_context("next_step", error.remedy)
}

#[cfg(test)]
#[path = "image_import_tests.rs"]
mod tests;
