use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::Arc,
};

use plotium_core::{
    actions::Action,
    state::{
        BoardFitItem, BoardFitTarget, BoardFrameId, CanvasId, Dataset, DatasetId, ObjectId,
        PlotiumApp, SeriesId,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Destination {
    pub canvas: CanvasId,
    pub plot: ObjectId,
    pub series: SeriesId,
}

impl Destination {
    fn resolve(
        self,
        app: &PlotiumApp,
    ) -> Option<(
        usize,
        &plotium_core::state::CanvasObject,
        &plotium_core::state::SeriesBinding,
    )> {
        let ci = app.doc.canvas_index(self.canvas)?;
        let object = app.doc.canvases[ci].object(self.plot)?;
        let series = object
            .plot()?
            .binding
            .series
            .iter()
            .find(|series| series.id == self.series)?;
        Some((ci, object, series))
    }

    pub(super) fn hidden(self, app: &PlotiumApp) -> Option<bool> {
        let (ci, object, series) = self.resolve(app)?;
        let canvas = &app.doc.canvases[ci];
        Some(
            !series.visible
                || !object.visible
                || canvas
                    .parent_panel(self.plot)
                    .and_then(|id| canvas.panel(id))
                    .is_some_and(|p| !p.visible),
        )
    }

    pub(super) fn label(self, app: &PlotiumApp) -> Option<String> {
        let (ci, object, series) = self.resolve(app)?;
        let canvas = &app.doc.canvases[ci];
        let panel = canvas
            .parent_panel(self.plot)
            .and_then(|id| canvas.panel(id));
        let title = if object.name.is_empty() {
            "Plot"
        } else {
            &object.name
        };
        Some(format!(
            "Canvas {}: {} / {} / {} #{} / {} #{}{}",
            ci + 1,
            canvas.name,
            panel.map_or("Unpanelled", |p| &p.name),
            title,
            self.plot.get(),
            app.series_label(series),
            self.series.get(),
            if self.hidden(app)? { " (hidden)" } else { "" }
        ))
    }
}

#[derive(Default)]
pub(super) struct PlotIndex {
    occurrences: HashMap<DatasetId, Vec<Destination>>,
    pub summaries: HashMap<CanvasId, CanvasComposition>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct CanvasComposition {
    pub compact: String,
    pub details: String,
    pub datasets: Vec<DatasetId>,
}

impl PlotIndex {
    fn build(app: &PlotiumApp) -> Self {
        let mut result = Self::default();
        let datasets: HashMap<_, _> = app
            .doc
            .datasets
            .iter()
            .map(|d| (d.resource_id(), d))
            .collect();
        for canvas in &app.doc.canvases {
            let mut seen = BTreeSet::new();
            for object in &canvas.objects {
                let Some(plot) = object.plot() else { continue };
                for series in &plot.binding.series {
                    seen.insert(series.source.resource);
                    result
                        .occurrences
                        .entry(series.source.resource)
                        .or_default()
                        .push(Destination {
                            canvas: canvas.resource_id,
                            plot: object.id,
                            series: series.id,
                        });
                }
            }
            result.summaries.insert(
                canvas.resource_id,
                composition(seen.iter().filter_map(|id| datasets.get(id).copied())),
            );
        }
        result
    }

    pub fn occurrences(&self, dataset: DatasetId) -> &[Destination] {
        self.occurrences.get(&dataset).map_or(&[], Vec::as_slice)
    }
}

pub(super) fn index(app: &PlotiumApp, ctx: &egui::Context) -> Arc<PlotIndex> {
    let key = egui::Id::new((
        "dataset_plot_index",
        app.session.project_context.to_string(),
    ));
    let revision = app.doc.edit_generation;
    ctx.data_mut(|data| {
        if let Some((old, index)) = data.get_temp::<(u64, Arc<PlotIndex>)>(key)
            && old == revision
        {
            return index;
        }
        let index = Arc::new(PlotIndex::build(app));
        data.insert_temp(key, (revision, index.clone()));
        index
    })
}

fn composition<'a>(datasets: impl Iterator<Item = &'a Dataset>) -> CanvasComposition {
    let mut types = BTreeMap::<&str, BTreeMap<String, &str>>::new();
    let mut details = Vec::new();
    let mut ids = Vec::new();
    for dataset in datasets {
        ids.push(dataset.resource_id());
        let category = dataset.compact_category();
        let experiment = dataset.experiment_label();
        let experiments = types.entry(category).or_default();
        if let Some(label) = experiment {
            experiments.entry(label.to_lowercase()).or_insert(label);
        }
        details.push(format!(
            "{} — {}{}",
            dataset.browser_name(),
            category,
            experiment.map_or_else(String::new, |name| format!(" · {name}"))
        ));
    }
    details.sort_unstable();
    let count = details.len();
    let mut compact = match types.len() {
        0 => "No plotted data".to_owned(),
        1 => types
            .first_key_value()
            .map_or_else(String::new, |(category, labels)| {
                if (1..=2).contains(&labels.len()) {
                    format!(
                        "{category} · {}",
                        labels.values().copied().collect::<Vec<_>>().join(" / ")
                    )
                } else {
                    (*category).to_owned()
                }
            }),
        2 => types.keys().copied().collect::<Vec<_>>().join(" + "),
        n => format!("{n} data types"),
    };
    if count > 1 {
        compact.push_str(&format!(" · {count} datasets"));
    }
    CanvasComposition {
        compact,
        details: details.join("\n"),
        datasets: ids,
    }
}

fn key(app: &PlotiumApp, name: &str) -> egui::Id {
    egui::Id::new((name, app.session.project_context.to_string()))
}

pub(super) fn locate_dataset(app: &mut PlotiumApp, ctx: &egui::Context, dataset: DatasetId) {
    let index = index(app, ctx);
    let chooser = key(app, "plot_destinations");
    ctx.data_mut(|data| data.remove::<DatasetId>(chooser));
    match index.occurrences(dataset) {
        [] => app.session.status = "Not plotted.".into(),
        [one] if one.hidden(app) == Some(false) => navigate(app, ctx, *one),
        [one] => {
            ctx.data_mut(|data| data.insert_temp(chooser, dataset));
            navigate(app, ctx, *one);
        }
        _ => {
            ctx.data_mut(|data| data.insert_temp(chooser, dataset));
        }
    }
}

pub(super) fn destinations_window(app: &mut PlotiumApp, ctx: &egui::Context) {
    use super::commands::{self, CommandId};

    let id = key(app, "plot_destinations");
    let Some(dataset) = ctx.data(|data| data.get_temp::<DatasetId>(id)) else {
        return;
    };
    let Some(di) = app.doc.dataset_index(dataset) else {
        ctx.data_mut(|data| data.remove::<DatasetId>(id));
        return;
    };
    let index = index(app, ctx);
    let mut open = true;
    let mut chosen = None;
    egui::Window::new("Plot destinations")
        .id(id)
        .open(&mut open)
        .default_width(420.0)
        .show(ctx, |ui| {
            ui.label(app.doc.datasets[di].browser_name());
            let occurrences = index.occurrences(dataset);
            if occurrences.is_empty() {
                ui.weak("No plots remain. Close this window and choose another dataset.");
            }
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    for &target in occurrences {
                        ui.horizontal_wrapped(|ui| {
                            for command in [
                                Some(CommandId::GoToDatasetPlot(target)),
                                (target.hidden(app) == Some(true))
                                    .then_some(CommandId::RevealDatasetPlot(target)),
                            ]
                            .into_iter()
                            .flatten()
                            {
                                let descriptor = commands::describe(app, command);
                                if ui
                                    .add_enabled(
                                        descriptor.enabled,
                                        egui::Button::new(descriptor.label).wrap(),
                                    )
                                    .on_disabled_hover_text(
                                        descriptor.disabled_reason.unwrap_or_default(),
                                    )
                                    .clicked()
                                {
                                    chosen = Some(command);
                                }
                            }
                        });
                    }
                });
        });
    if let Some(command) = chosen {
        commands::execute_without_clipboard(command, app, ctx);
        // Keep Reveal available after locating a hidden series.
        open = matches!(command, CommandId::GoToDatasetPlot(target) if target.hidden(app) == Some(true));
    }
    if !open {
        ctx.data_mut(|data| data.remove::<DatasetId>(id));
    }
}

pub(super) fn navigate(app: &mut PlotiumApp, ctx: &egui::Context, target: Destination) {
    let Some((ci, _, _)) = target.resolve(app) else {
        return;
    };
    let item = BoardFitItem::Content {
        canvas: target.canvas,
        content: target.plot,
    };
    let page = &app.doc.canvases[ci];
    let fit = if plotium_core::state::board_fit_item_rect(app, item).is_some() {
        BoardFitTarget::Items(vec![item])
    } else if let Some(frame) = page.content_page_frame(target.plot) {
        let rect = frame.rect();
        let [x, y] = page.board_pos;
        BoardFitTarget::Region([
            x + rect.left,
            y + rect.top,
            x + rect.right(),
            y + rect.bottom(),
        ])
    } else {
        BoardFitTarget::Frame(BoardFrameId::Page(target.canvas))
    };
    app.session.active_canvas = Some(ci);
    app.select_object(ci, target.plot);
    app.session.ui.frame_selection = vec![BoardFrameId::Page(target.canvas)];
    super::canvas::request_board_fit_target(app, ctx, fit);
    let until = ctx.input(|input| input.time) + 2.5;
    ctx.data_mut(|data| data.insert_temp(key(app, "located_series"), (target, until)));
    app.session.status = if target.hidden(app) == Some(true) {
        "Located a hidden series. Use Reveal to show it."
    } else {
        "Located the plot series."
    }
    .into();
}

pub(super) fn reveal(app: &mut PlotiumApp, ctx: &egui::Context, target: Destination) {
    let Some((ci, object, _)) = target.resolve(app) else {
        return;
    };
    let canvas = &app.doc.canvases[ci];
    let mut actions = Vec::new();
    if let Some(panel) = canvas.parent_panel(target.plot)
        && canvas.panel(panel).is_some_and(|p| !p.visible)
    {
        let mut after = plotium_core::actions::PanelState::of(canvas);
        if let Some(p) = after.panels.iter_mut().find(|p| p.id == panel) {
            p.visible = true;
        }
        actions.push(Action::ReplacePanelState {
            canvas: ci,
            before: plotium_core::actions::PanelState::of(canvas),
            after,
        });
    }
    if !object.visible {
        actions.push(Action::set_object_flags(
            ci,
            target.plot,
            (false, object.locked),
            (true, object.locked),
        ));
    }
    if let Some(plot) = object.plot() {
        let mut after = plot.binding.clone();
        if let Some(series) = after.series.iter_mut().find(|s| s.id == target.series)
            && !series.visible
        {
            series.visible = true;
            actions.push(Action::set_series_presentation(
                target.canvas,
                target.plot,
                plot.binding.clone(),
                after,
            ));
        }
    }
    if !actions.is_empty() {
        app.execute_action(Action::Composite(actions));
    }
    navigate(app, ctx, target);
}

pub(super) fn highlight(
    app: &PlotiumApp,
    ctx: &egui::Context,
    canvas: CanvasId,
) -> Option<Destination> {
    let (target, until) =
        ctx.data(|data| data.get_temp::<(Destination, f64)>(key(app, "located_series")))?;
    let now = ctx.input(|input| input.time);
    if now >= until || target.canvas != canvas || target.hidden(app) != Some(false) {
        return None;
    }
    ctx.request_repaint_after(std::time::Duration::from_secs_f64(until - now));
    Some(target)
}

#[cfg(test)]
#[path = "data_navigation_tests.rs"]
mod tests;
