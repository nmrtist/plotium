//! Bounded, document-scoped background data preparation.

use super::{Dataset, PlotiumApp};
use std::{collections::VecDeque, path::PathBuf, sync::mpsc};

pub(super) struct PreparedImport {
    pub dataset: Dataset,
    pub source: String,
    pub format: plotium_io::DataFormat,
    pub warnings: Vec<plotium_io::LoadWarning>,
}

#[cfg(test)]
#[path = "data_import_tests.rs"]
mod tests;

impl PreparedImport {
    pub(super) fn new(loaded: plotium_io::LoadResult, equal_scale: bool) -> Result<Self, String> {
        let (dataset, source) = crate::workflow::dataset_from_loaded_acquisition(
            loaded.acquisition,
            loaded.acquisition_identity,
            equal_scale,
        )
        .map_err(|error| error.to_string())?;
        Ok(Self {
            dataset,
            source,
            format: loaded.format,
            warnings: loaded.warnings,
        })
    }
}

/// Identity of an import gesture within the current project session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImportBatchId(u64);

impl std::fmt::Display for ImportBatchId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportPhase {
    Queued,
    Scanning,
    Preparing,
    Complete,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportTarget {
    pub dataset: super::DatasetId,
    pub canvas: Option<super::CanvasId>,
}

#[derive(Clone, Debug)]
pub struct ImportItem {
    pub path: PathBuf,
    pub result: Result<ImportTarget, String>,
}

#[derive(Clone, Debug)]
pub struct ImportBatch {
    pub id: ImportBatchId,
    pub recent: PathBuf,
    pub phase: ImportPhase,
    pub total: Option<usize>,
    pub items: Vec<ImportItem>,
    pub retryable: bool,
}

impl ImportBatch {
    pub fn loaded(&self) -> usize {
        self.items.iter().filter(|item| item.result.is_ok()).count()
    }
    pub fn failed(&self) -> usize {
        self.items
            .iter()
            .filter(|item| item.result.is_err())
            .count()
    }
}

#[derive(Clone, Debug)]
pub enum ImportOpenTarget {
    Dataset(ImportTarget),
    Workspace(Vec<ImportTarget>),
    Batch(ImportBatchId),
}

#[derive(Default)]
pub struct ImportDiscovery {
    pub paths: Vec<PathBuf>,
    pub failures: Vec<(PathBuf, String)>,
}

type Discover = Box<dyn FnOnce() -> Result<ImportDiscovery, String> + Send>;
struct Request {
    id: ImportBatchId,
    recent: PathBuf,
    discover: Discover,
    equal_scale: bool,
}

enum Event {
    Started(PathBuf, usize),
    Item(PathBuf, Result<PreparedImport, String>),
    DiscoveryFailed(PathBuf, String),
    Finished,
}

struct Job {
    receiver: mpsc::Receiver<Event>,
    id: ImportBatchId,
}

/// Dropping a session disconnects the bounded channel, preventing the worker
/// from publishing results into another project. Completed batches stay bounded.
#[derive(Default)]
pub struct DataImports {
    pending: VecDeque<Request>,
    active: Option<Job>,
    batches: VecDeque<ImportBatch>,
    next_id: u64,
    operation_ids: VecDeque<crate::operation::OperationId>,
    open_intent: Option<ImportBatchId>,
    open_target: Option<ImportOpenTarget>,
}

impl DataImports {
    pub fn is_pending(&self) -> bool {
        self.active.is_some() || !self.pending.is_empty()
    }
    pub fn batches(&self) -> &VecDeque<ImportBatch> {
        &self.batches
    }
    /// Reports already represented by import notices remain associated after batch eviction.
    pub fn represents_operation(&self, id: crate::operation::OperationId) -> bool {
        self.operation_ids.contains(&id)
    }
    fn record_operations(&mut self, ids: impl IntoIterator<Item = crate::operation::OperationId>) {
        for id in ids {
            self.operation_ids.push_back(id);
            if self.operation_ids.len() > crate::operation::OPERATION_HISTORY_CAPACITY {
                self.operation_ids.pop_front();
            }
        }
    }
    fn add_batch(&mut self, recent: PathBuf, phase: ImportPhase, retryable: bool) -> ImportBatchId {
        self.next_id += 1;
        let id = ImportBatchId(self.next_id);
        self.batches.push_back(ImportBatch {
            id,
            recent,
            phase,
            total: None,
            items: Vec::new(),
            retryable,
        });
        self.prune();
        id
    }
    fn batch_mut(&mut self, id: ImportBatchId) -> &mut ImportBatch {
        self.batches
            .iter_mut()
            .find(|batch| batch.id == id)
            .expect("active and queued imports retain their batch")
    }
    fn prune(&mut self) {
        while self
            .batches
            .iter()
            .filter(|batch| batch.phase == ImportPhase::Complete)
            .count()
            > 32
        {
            let index = self
                .batches
                .iter()
                .position(|batch| batch.phase == ImportPhase::Complete)
                .expect("completed batch count exceeds the retention limit");
            self.batches.remove(index);
        }
    }
    fn finish(&mut self, id: ImportBatchId) {
        let batch = self.batch_mut(id);
        batch.phase = ImportPhase::Complete;
        if batch.total.is_none() && batch.items.is_empty() {
            batch.total = Some(0);
        }
        if self.open_intent == Some(id) {
            let batch = self.batch_mut(id);
            let targets: Vec<_> = batch
                .items
                .iter()
                .filter_map(|item| item.result.as_ref().ok().cloned())
                .collect();
            self.open_target = match targets.len() {
                0 => None,
                1 => Some(ImportOpenTarget::Dataset(
                    targets.into_iter().next().unwrap(),
                )),
                _ => Some(ImportOpenTarget::Workspace(targets)),
            };
            self.open_intent = None;
        }
        self.prune();
    }
}

impl PlotiumApp {
    pub fn cancel_import_open_intent(&mut self) {
        self.session.data_imports.open_intent = None;
        self.session.data_imports.open_target = None;
    }
    pub fn take_import_open_target(&mut self) -> Option<ImportOpenTarget> {
        self.session.data_imports.open_target.take()
    }

    /// Continue an explicit synchronous import by showing its completed batch.
    pub fn request_import_results(&mut self, id: ImportBatchId) {
        if self
            .session
            .data_imports
            .batches
            .iter()
            .any(|batch| batch.id == id && batch.phase == ImportPhase::Complete)
        {
            self.cancel_import_open_intent();
            self.session.data_imports.open_target = Some(ImportOpenTarget::Batch(id));
        }
    }

    /// Continue a completed synchronous import by displaying its successful
    /// results together, just like an asynchronous foreground import.
    pub fn request_completed_import_open(&mut self, id: ImportBatchId) {
        if self
            .session
            .data_imports
            .batches
            .iter()
            .any(|batch| batch.id == id && batch.phase == ImportPhase::Complete)
        {
            self.cancel_import_open_intent();
            self.session.data_imports.open_intent = Some(id);
            self.session.data_imports.finish(id);
        }
    }
    /// Only an explicit first import may continue by opening its results. Any
    /// later intentional input cancels that continuation, even if selection returns.
    pub fn queue_user_data_import(
        &mut self,
        recent: PathBuf,
        discover: impl FnOnce() -> Result<Vec<PathBuf>, String> + Send + 'static,
    ) -> ImportBatchId {
        self.queue_user_data_import_discovery(recent, move || {
            discover().map(|paths| ImportDiscovery {
                paths,
                failures: Vec::new(),
            })
        })
    }
    pub fn queue_user_data_import_discovery(
        &mut self,
        recent: PathBuf,
        discover: impl FnOnce() -> Result<ImportDiscovery, String> + Send + 'static,
    ) -> ImportBatchId {
        self.cancel_import_open_intent();
        let open = self.doc.datasets.is_empty() && self.doc.canvases.is_empty();
        let id = self.enqueue_import(recent, Box::new(discover));
        if open {
            self.session.data_imports.open_intent = Some(id);
        }
        id
    }
    /// Background callers never request navigation when their import completes.
    pub fn queue_data_import(
        &mut self,
        recent: PathBuf,
        discover: impl FnOnce() -> Result<Vec<PathBuf>, String> + Send + 'static,
    ) {
        self.queue_data_import_discovery(recent, move || {
            discover().map(|paths| ImportDiscovery {
                paths,
                failures: Vec::new(),
            })
        });
    }
    pub fn queue_data_import_discovery(
        &mut self,
        recent: PathBuf,
        discover: impl FnOnce() -> Result<ImportDiscovery, String> + Send + 'static,
    ) {
        self.enqueue_import(recent, Box::new(discover));
    }
    fn enqueue_import(&mut self, recent: PathBuf, discover: Discover) -> ImportBatchId {
        let imports = &mut self.session.data_imports;
        let id = imports.add_batch(recent.clone(), ImportPhase::Queued, true);
        imports.pending.push_back(Request {
            id,
            recent,
            discover,
            equal_scale: self.settings.general.equal_scale_homonuclear_2d_imports,
        });
        self.session.status = "Data import queued; you can continue working.".into();
        id
    }
    /// Specialized synchronous importers retain feedback without implying that
    /// their results can be retried by the native acquisition loader.
    pub fn record_completed_user_import(
        &mut self,
        recent: PathBuf,
        items: Vec<ImportItem>,
    ) -> ImportBatchId {
        self.record_completed_user_import_with_operations(recent, items, Vec::new())
    }
    pub fn record_completed_user_import_with_operations(
        &mut self,
        recent: PathBuf,
        items: Vec<ImportItem>,
        operation_ids: Vec<crate::operation::OperationId>,
    ) -> ImportBatchId {
        let imports = &mut self.session.data_imports;
        let id = imports.add_batch(recent, ImportPhase::Preparing, false);
        let batch = imports.batch_mut(id);
        batch.total = Some(items.len());
        batch.items = items;
        imports.record_operations(operation_ids);
        imports.finish(id);
        id
    }
    pub fn retry_failed_import(&mut self, id: ImportBatchId) -> Option<ImportBatchId> {
        let batch = self
            .session
            .data_imports
            .batches
            .iter()
            .find(|batch| batch.id == id)?;
        if batch.phase != ImportPhase::Complete || !batch.retryable {
            return None;
        }
        let paths: Vec<_> = batch
            .items
            .iter()
            .filter(|item| item.result.is_err())
            .map(|item| item.path.clone())
            .collect();
        if paths.is_empty() {
            return None;
        }
        let recent = batch.recent.clone();
        self.cancel_import_open_intent();
        Some(self.enqueue_import(
            recent,
            Box::new(move || {
                Ok(ImportDiscovery {
                    paths,
                    failures: Vec::new(),
                })
            }),
        ))
    }
    /// Process at most one item per frame, bounding both preparation and UI work.
    pub fn poll_data_import(&mut self) -> bool {
        let mut imports = std::mem::take(&mut self.session.data_imports);
        if imports.active.is_none()
            && let Some(request) = imports.pending.pop_front()
        {
            let (sender, receiver) = mpsc::sync_channel(1);
            let id = request.id;
            imports.batch_mut(id).phase = ImportPhase::Scanning;
            self.session.status = format!("Scanning {}…", request.recent.display());
            match std::thread::Builder::new()
                .name("data-import".into())
                .spawn(move || {
                    let discovered = match (request.discover)() {
                        Ok(paths) => paths,
                        Err(error) => {
                            if sender
                                .send(Event::Item(request.recent, Err(error)))
                                .is_err()
                            {
                                return;
                            }
                            if sender.send(Event::Finished).is_err() {
                                return;
                            }
                            return;
                        }
                    };
                    let total = discovered.paths.len() + discovered.failures.len();
                    if sender.send(Event::Started(request.recent, total)).is_err() {
                        return;
                    }
                    for (path, error) in discovered.failures {
                        if sender.send(Event::DiscoveryFailed(path, error)).is_err() {
                            return;
                        }
                    }
                    let workers = std::thread::available_parallelism()
                        .map_or(1, usize::from)
                        .min(4);
                    if !prepare_paths(&discovered.paths, &sender, workers, total, &|path| {
                        plotium_io::load_path(path)
                            .map_err(|error| error.to_string())
                            .and_then(|loaded| PreparedImport::new(loaded, request.equal_scale))
                    }) {
                        return;
                    }
                    if sender.send(Event::Finished).is_err() {
                        // Document closure is normal cancellation, not an import failure.
                    }
                }) {
                Ok(_) => imports.active = Some(Job { receiver, id }),
                Err(error) => {
                    let path = imports.batch_mut(id).recent.clone();
                    self.record_import_item(
                        &mut imports,
                        id,
                        path,
                        Err(format!("Could not start import worker: {error}")),
                    );
                    imports.finish(id);
                }
            }
        }
        if let Some(job) = imports.active.as_ref() {
            let id = job.id;
            match job.receiver.try_recv() {
                Ok(Event::Started(path, total)) => {
                    let batch = imports.batch_mut(id);
                    batch.total = Some(total);
                    batch.phase = ImportPhase::Preparing;
                    self.session.status = format!(
                        "Preparing {}/{}: {} ({} loaded, {} failed)",
                        batch.items.len() + 1,
                        total,
                        path.display(),
                        batch.loaded(),
                        batch.failed()
                    );
                }
                Ok(Event::Item(path, result)) => {
                    self.record_import_item(&mut imports, id, path, result)
                }
                Ok(Event::DiscoveryFailed(path, error)) => {
                    imports.batch_mut(id).retryable = false;
                    self.record_import_item(&mut imports, id, path, Err(error));
                }
                Ok(Event::Finished) => {
                    let batch = imports.batch_mut(id);
                    self.session.status = format!(
                        "Import complete: {} loaded, {} failed.",
                        batch.loaded(),
                        batch.failed()
                    );
                    if batch.loaded() > 0 {
                        self.note_recent_file(&batch.recent);
                    }
                    imports.finish(id);
                    imports.active = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    let path = imports.batch_mut(id).recent.clone();
                    self.record_import_item(
                        &mut imports,
                        id,
                        path,
                        Err("The import worker stopped unexpectedly; retry the import.".into()),
                    );
                    imports.finish(id);
                    imports.active = None;
                }
            }
        }
        let busy = imports.is_pending();
        self.session.data_imports = imports;
        busy
    }
    fn record_import_item(
        &mut self,
        imports: &mut DataImports,
        id: ImportBatchId,
        path: PathBuf,
        result: Result<PreparedImport, String>,
    ) {
        // The insertion action changes only these three navigation fields. All
        // other UI state (including live editing drafts and camera) stays untouched.
        let active_canvas = self.session.active_canvas;
        let selection = self.session.ui.data_selection.clone();
        let view = self.session.view;
        let before = self.doc.datasets.len();
        let had_content = before > 0 || !self.doc.canvases.is_empty();
        let previous_order = self
            .session
            .operation_history
            .operations()
            .next_back()
            .map_or(0, |report| report.completion_order);
        let success = self.install_prepared_import(&path, result);
        imports.record_operations(
            self.session
                .operation_history
                .operations()
                .filter(|report| report.completion_order > previous_order)
                .map(|report| report.id),
        );
        let result = if success {
            let dataset = self.doc.datasets[before].resource_id();
            Ok(ImportTarget {
                dataset,
                canvas: self
                    .session
                    .active_canvas
                    .and_then(|index| self.doc.canvases.get(index))
                    .filter(|canvas| {
                        canvas.objects.iter().any(|object| {
                            object
                                .plot()
                                .is_some_and(|plot| plot.binding.contains_dataset(dataset))
                        })
                    })
                    .map(|canvas| canvas.resource_id),
            })
        } else {
            Err(self.session.status.clone())
        };
        self.session.active_canvas = active_canvas;
        self.session.ui.data_selection = selection;
        self.session.view = view;
        if success
            && had_content
            && self.session.viewport_mode
                == super::ViewportMode::Fit(super::BoardFitTarget::AllFrames)
        {
            // A persistent fit-to-all would otherwise move the camera on the
            // next UI frame simply because the background batch added a frame.
            self.session.viewport_mode = super::ViewportMode::Manual;
        }
        let batch = imports.batch_mut(id);
        if batch.total.is_none() && result.is_err() {
            // Discovery did not yield a concrete acquisition path. Retrying that
            // path with the acquisition loader would change the original request.
            batch.retryable = false;
        }
        let detail = result.as_ref().err().cloned();
        batch.items.push(ImportItem {
            path: path.clone(),
            result,
        });
        self.session.status = format!(
            "Importing: {} loaded, {} failed. {}",
            batch.loaded(),
            batch.failed(),
            path.display()
        );
        if let Some(error) = detail {
            self.session.status.push_str(&format!(" {error}"));
        }
    }
}

/// Small windows bound both active processing and completed results awaiting the
/// UI. Publish in discovery order so scheduling cannot change board/undo order.
fn prepare_paths(
    paths: &[PathBuf],
    sender: &mpsc::SyncSender<Event>,
    workers: usize,
    total: usize,
    prepare: &(impl Fn(&std::path::Path) -> Result<PreparedImport, String> + Sync),
) -> bool {
    for window in paths.chunks(workers.max(1)) {
        if sender
            .send(Event::Started(window[0].clone(), total))
            .is_err()
        {
            return false;
        }
        let connected = std::thread::scope(|scope| {
            let handles: Vec<_> = window
                .iter()
                .map(|path| {
                    let handle = std::thread::Builder::new()
                        .name("data-import-prepare".into())
                        .spawn_scoped(scope, move || prepare(path));
                    (path, handle)
                })
                .collect();
            let mut connected = true;
            for (path, handle) in handles {
                let result = match handle {
                    Ok(handle) => handle.join().unwrap_or_else(|_| {
                        Err("The import worker stopped unexpectedly; retry the import.".into())
                    }),
                    Err(error) => Err(format!("Could not start import worker: {error}")),
                };
                if connected && sender.send(Event::Item(path.clone(), result)).is_err() {
                    connected = false;
                }
            }
            connected
        });
        if !connected {
            return false;
        }
    }
    true
}
