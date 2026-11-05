//! One coalesced, disposable preview. The document and field caches stay committed.
use plotium_core::state::*;
use plotium_processing::nmr_volume::VolumePhase;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct Request {
    project: ProjectContextId,
    id: DatasetId,
    plot: ObjectId,
    generation: u64,
    processed: std::sync::Weak<nmr::Dataset>,
    phases: [VolumePhase; 3],
    fixed: usize,
    index: usize,
}
impl Request {
    fn same_plane(&self, other: &Self) -> bool {
        self.plot == other.plot
            && self.generation == other.generation
            && self.project == other.project
            && self.id == other.id
            && self.processed.ptr_eq(&other.processed)
            && self.fixed == other.fixed
            && self.index == other.index
    }
    fn matches(&self, other: &Self) -> bool {
        self.same_plane(other) && self.phases == other.phases
    }
}
type PreviewOutput = Arc<Mutex<Option<Result<plotium_figure::Figure, String>>>>;
#[derive(Clone, Default)]
struct Preview {
    running: Option<(Request, PreviewOutput)>,
    shown: Option<(Request, Arc<plotium_figure::Figure>)>,
    levels: Option<(Request, ContourGeometryCacheKey)>,
    failed: Option<Request>,
    companions: Option<(Vec<plotium_figure::Contour>, Vec<plotium_figure::Contour>)>,
}
fn key() -> egui::Id {
    egui::Id::new("volume_local_phase_preview")
}
pub(super) fn canvas_feedback(app: &PlotiumApp, ctx: &egui::Context) -> Option<&'static str> {
    request(app)?;
    Some(feedback(app, ctx))
}
pub(super) fn feedback(app: &PlotiumApp, ctx: &egui::Context) -> &'static str {
    let Some(current) = request(app) else {
        return "Committed display · no active preview";
    };
    let cache = ctx
        .data(|data| data.get_temp::<Preview>(key()))
        .unwrap_or_default();
    if cache
        .failed
        .as_ref()
        .is_some_and(|failed| failed.matches(&current))
    {
        "Preview failed · see status"
    } else if cache
        .shown
        .as_ref()
        .is_some_and(|(shown, _)| shown.matches(&current))
    {
        "Unapplied · current plane preview"
    } else {
        "Unapplied · updating plane preview…"
    }
}
fn request(app: &PlotiumApp) -> Option<Request> {
    if !super::tools::task_card::is_active(app, TaskDockTab::Processing) {
        return None;
    }
    let id = super::nmr_volume::phase_target(app)?;
    let canvas = app.doc.canvases.get(app.session.active_canvas?)?;
    let plot_id = canvas
        .selected_plot_object_id()
        .or_else(|| canvas.active_plot_object_id())?;
    let plot = canvas.object(plot_id)?.plot()?;
    if canvas.object(plot_id)?.dataset() != Some(id) {
        return None;
    }
    if app.session.ui.processing_task_dataset != Some(id) {
        return None;
    }
    let volume = app.doc.dataset_by_id(id)?.as_nmr2d()?.volume.as_ref()?;
    let draft = app.session.ui.nmr_volume_phase_drafts.get(&id)?;
    if draft.baseline != volume.phases
        || !draft
            .source
            .ptr_eq(&Arc::downgrade(volume.source.dataset()))
        || draft.phases == volume.phases
    {
        return None;
    }
    Some(Request {
        plot: plot_id,
        generation: plot.figure_geometry_generation(),
        project: app.session.project_context,
        id,
        processed: Arc::downgrade(volume.processed.source().dataset()),
        phases: draft.phases,
        fixed: volume.fixed_axis,
        index: volume.index,
    })
}
pub(super) fn contours(
    app: &PlotiumApp,
    ctx: &egui::Context,
    id: DatasetId,
    object: ObjectId,
) -> Option<Arc<plotium_figure::Figure>> {
    let current = request(app)?;
    if current.id != id || current.plot != object {
        return None;
    }
    let cache = ctx.data(|data| data.get_temp::<Preview>(key()))?;
    let (shown, figure) = cache.shown?;
    if !shown.same_plane(&current) {
        return None;
    }
    let mut composed = (*figure).clone();
    let (mut before, after) = cache.companions?;
    before.append(&mut composed.contours);
    before.extend(after);
    composed.contours = before;
    Some(Arc::new(composed))
}
fn target_series(app: &PlotiumApp, current: &Request) -> Option<SeriesBinding> {
    app.doc
        .canvases
        .iter()
        .flat_map(|c| &c.objects)
        .find(|object| object.id == current.plot)
        .and_then(|o| o.plot())
        .into_iter()
        .flat_map(|plot| &plot.binding.series)
        .find(|series| {
            series.source.resource == current.id
                && series.visible
                && app
                    .doc
                    .dataset_by_id(current.id)
                    .and_then(|d| d.field_descriptor(series.source.field))
                    .is_some_and(|d| d.local_id == "nmr.real")
                && matches!(series.encoding, plotium_figure::SeriesEncoding::Contour(_))
        })
        .cloned()
}
fn companion_series(
    series: Vec<SeriesBinding>,
    target: Option<SeriesId>,
) -> (Vec<SeriesBinding>, Vec<SeriesBinding>) {
    let mut before = Vec::new();
    let mut after = Vec::new();
    let mut passed_target = false;
    for series in series {
        if Some(series.id) == target {
            passed_target = true;
        } else if passed_target {
            after.push(series);
        } else {
            before.push(series);
        }
    }
    (before, after)
}
pub(super) fn update(app: &mut PlotiumApp, ctx: &egui::Context) {
    let current = request(app);
    let mut cache = ctx
        .data(|data| data.get_temp::<Preview>(key()))
        .unwrap_or_default();
    if let Some((running, output)) = &cache.running {
        let result = match output.lock() {
            Ok(mut output) => output.take(),
            Err(error) => Some(Err(format!("Phase preview worker failed: {error}"))),
        };
        if let Some(result) = result {
            if current
                .as_ref()
                .is_some_and(|current| running.matches(current))
            {
                match result {
                    Ok(figure) => cache.shown = Some((running.clone(), Arc::new(figure))),
                    Err(error) => {
                        app.session.status = format!("Cannot preview phase: {error}");
                        cache.failed = Some(running.clone());
                    }
                }
            }
            cache.running = None;
        }
    }
    if let Some(current) = current {
        if cache.running.is_none()
            && !cache
                .shown
                .as_ref()
                .is_some_and(|(shown, _)| shown.matches(&current))
            && !cache
                .failed
                .as_ref()
                .is_some_and(|failed| failed.matches(&current))
        {
            let series = target_series(app, &current);
            if let Some(series) = series {
                let levels = cache
                    .levels
                    .as_ref()
                    .filter(|(previous, _)| previous.same_plane(&current))
                    .map(|(_, levels)| Ok(levels.clone()))
                    .unwrap_or_else(|| app.volume_preview_contour_key(&series));
                match levels {
                    Ok(levels) => {
                        cache.levels = Some((current.clone(), levels.clone()));
                        let dataset = app
                            .doc
                            .dataset_by_id(current.id)
                            .and_then(Dataset::as_nmr2d)
                            .expect("validated preview target")
                            .clone();
                        let output = Arc::new(Mutex::new(None));
                        cache.running = Some((current.clone(), output.clone()));
                        let ctx = ctx.clone();
                        let plotium_figure::SeriesEncoding::Contour(spec) = series.encoding else {
                            unreachable!()
                        };
                        std::thread::spawn(move || {
                            let volume = dataset.volume.as_ref().expect("validated preview volume");
                            let started = std::time::Instant::now();
                            let result = volume
                                .processed
                                .phase_preview_plane(
                                    current.fixed,
                                    current.index,
                                    volume.phases,
                                    current.phases,
                                    &mut nmr::ExecutionContext::default(),
                                )
                                .and_then(|plane| {
                                    dataset.phase_preview_contours(plane, levels, spec.style)
                                });
                            log::debug!(
                                "Volume phase plane {:?}: {:?}",
                                volume.processed.shape(),
                                started.elapsed()
                            );
                            match output.lock() {
                                Ok(mut output) => *output = Some(result),
                                Err(error) => log::error!("Cannot deliver phase preview: {error}"),
                            }
                            ctx.request_repaint();
                        });
                    }
                    Err(error) => {
                        app.session.status = format!("Cannot preview phase: {error}");
                        cache.failed = Some(current);
                    }
                }
            } else {
                app.session.status =
                    "Show the target spectrum's real contour field to preview phase.".into();
                cache.failed = Some(current);
            }
        }
    } else {
        cache.shown = None;
        cache.levels = None;
        cache.failed = None;
    }
    cache.companions = if let Some(current) = request(app) {
        let target = target_series(app, &current).map(|series| series.id);
        let (before, after) = app
            .doc
            .canvases
            .iter()
            .flat_map(|c| &c.objects)
            .find(|object| object.id == current.plot)
            .and_then(|o| o.plot())
            .map(|p| {
                companion_series(
                    app.display_binding(p.display_owner, &p.binding).series,
                    target,
                )
            })
            .unwrap_or_default();
        app.volume_preview_companion_contours(&before)
            .zip(app.volume_preview_companion_contours(&after))
    } else {
        None
    };
    if cache.running.is_some() {
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
    ctx.data_mut(|data| data.insert_temp(key(), cache));
}

#[cfg(test)]
#[path = "nmr_phase_preview_tests.rs"]
mod tests;
