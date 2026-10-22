use egui::{Context, Ui};
use plotium_core::state::{
    ImportBatch, ImportBatchId, ImportOpenTarget, ImportPhase, ImportTarget, PlotiumApp,
    PrimaryView,
};

use super::commands::{self, CommandId};

fn window_id(app: &PlotiumApp) -> egui::Id {
    egui::Id::new(("import_results", app.session.project_context.to_string()))
}

#[derive(Clone, Default)]
struct ResultsState {
    open: bool,
    selected: Option<ImportBatchId>,
}

#[derive(Clone, Default)]
struct NoticeState {
    dismissed: bool,
    failure_dismissed: bool,
    completed_at: Option<f64>,
}

pub(super) struct ImportNotice {
    pub batch: ImportBatchId,
    pub outcome: plotium_core::operation::OperationOutcome,
    pub summary: String,
}

pub(super) fn notice(app: &PlotiumApp, ctx: &Context) -> Option<ImportNotice> {
    let now = ctx.input(|input| input.time);
    let mut visible = None;
    for batch in app.session.data_imports.batches() {
        let id = window_id(app).with(("notice", batch.id));
        let mut state = ctx.data_mut(|data| data.get_temp::<NoticeState>(id).unwrap_or_default());
        let failed = batch.failed() > 0;
        let complete = batch.phase == ImportPhase::Complete;
        if complete && state.completed_at.is_none() {
            state.completed_at = Some(now);
        }
        let expired = state.completed_at.is_some_and(|time| now - time >= 3.0);
        let show = if failed && complete {
            !state.failure_dismissed
        } else {
            !state.dismissed && (!complete || !expired)
        };
        ctx.data_mut(|data| data.insert_temp(id, state));
        if show {
            if complete && !failed {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
            visible = Some(ImportNotice {
                batch: batch.id,
                outcome: if failed {
                    plotium_core::operation::OperationOutcome::Failure
                } else {
                    plotium_core::operation::OperationOutcome::Success
                },
                summary: summary(batch),
            });
        }
    }
    visible
}

pub(super) fn dismiss_notice(app: &PlotiumApp, ctx: &Context, batch: ImportBatchId) {
    let id = window_id(app).with(("notice", batch));
    let mut state = ctx.data_mut(|data| data.get_temp::<NoticeState>(id).unwrap_or_default());
    state.dismissed = true;
    state.failure_dismissed = app
        .session
        .data_imports
        .batches()
        .iter()
        .any(|item| item.id == batch && item.failed() > 0);
    ctx.data_mut(|data| data.insert_temp(id, state));
}

pub(super) fn represents_operation(
    app: &PlotiumApp,
    id: plotium_core::operation::OperationId,
) -> bool {
    app.session.data_imports.represents_operation(id)
}

pub(crate) fn open_results(app: &PlotiumApp, ctx: &Context, batch: Option<ImportBatchId>) {
    let id = window_id(app);
    let mut state = ctx.data_mut(|data| data.get_temp::<ResultsState>(id).unwrap_or_default());
    state.open = true;
    state.selected = batch.or_else(|| app.session.data_imports.batches().back().map(|b| b.id));
    ctx.data_mut(|data| data.insert_temp(id, state));
}

pub(crate) fn cancel_on_input(app: &mut PlotiumApp, input: &egui::RawInput) {
    if !input.dropped_files.is_empty()
        || input.events.iter().any(|event| {
            matches!(
                event,
                egui::Event::PointerButton { pressed: true, .. }
                    | egui::Event::Key { pressed: true, .. }
                    | egui::Event::Text(_)
                    | egui::Event::Paste(_)
                    | egui::Event::Cut
                    | egui::Event::MouseWheel { .. }
                    | egui::Event::Zoom(_)
                    | egui::Event::Rotate(_)
                    | egui::Event::Touch {
                        phase: egui::TouchPhase::Start | egui::TouchPhase::Move,
                        ..
                    }
            )
        })
    {
        app.cancel_import_open_intent();
    }
}

pub(crate) fn summary(batch: &ImportBatch) -> String {
    match batch.phase {
        ImportPhase::Queued => "Data import queued".into(),
        ImportPhase::Scanning => "Scanning for data…".into(),
        ImportPhase::Preparing => match batch.total {
            Some(total) => format!(
                "Importing {}/{} · {} succeeded, {} failed",
                batch.items.len(),
                total,
                batch.loaded(),
                batch.failed()
            ),
            None => "Preparing data…".into(),
        },
        ImportPhase::Complete if batch.total == Some(0) => "No data files were found.".into(),
        ImportPhase::Complete => format!(
            "Import finished · {} succeeded, {} failed",
            batch.loaded(),
            batch.failed()
        ),
    }
}

pub(crate) fn apply_open_target(app: &mut PlotiumApp, ui: &Ui) {
    match app.take_import_open_target() {
        Some(ImportOpenTarget::Dataset(target)) => {
            open_target(app, ui, &target);
        }
        Some(ImportOpenTarget::Workspace(targets)) => {
            open_workspace(app, ui, &targets);
        }
        Some(ImportOpenTarget::Batch(batch)) => open_results(app, ui.ctx(), Some(batch)),
        None => {}
    }
}

fn open_workspace(app: &mut PlotiumApp, ui: &Ui, targets: &[ImportTarget]) {
    let datasets: Vec<_> = targets
        .iter()
        .filter_map(|target| app.doc.dataset_index(target.dataset))
        .collect();
    let canvas = targets.iter().find_map(|target| {
        app.doc.dataset_index(target.dataset)?;
        target.canvas.and_then(|id| app.doc.canvas_index(id))
    });
    if let Some(canvas) = canvas {
        super::primary_sidebar::selection::select_canvas(app, ui, canvas);
        app.focus_datasets(&datasets, None);
        app.session.view = PrimaryView::Canvas;
        app.session.ui.frame_selection.clear();
        app.session.ui.sheet_open = None;
        app.session.board_reveal = None;
        super::canvas::request_board_fit_all(app, ui.ctx());
    } else if let Some(target) = targets
        .iter()
        .find(|target| app.doc.dataset_index(target.dataset).is_some())
    {
        open_target(app, ui, target);
        app.focus_datasets(&datasets, None);
    }
}

fn open_target(app: &mut PlotiumApp, ui: &Ui, target: &ImportTarget) -> bool {
    let Some(dataset) = app.doc.dataset_index(target.dataset) else {
        return false;
    };
    if let Some(canvas) = target.canvas.and_then(|id| app.doc.canvas_index(id))
        && app.doc.datasets[dataset].as_table().is_none()
    {
        super::primary_sidebar::selection::select_canvas(app, ui, canvas);
    } else {
        let visible = (0..app.doc.datasets.len()).collect::<Vec<_>>();
        super::primary_sidebar::selection::select_dataset(
            app,
            ui,
            &visible,
            dataset,
            Default::default(),
        );
        app.session.view = PrimaryView::Data;
        app.session.primary_sidebar_visible = true;
        if app.doc.datasets[dataset].as_table().is_some() {
            app.session.ui.sheet_open = Some(dataset);
        }
    }
    true
}

pub(crate) fn results_window(app: &mut PlotiumApp, ctx: &Context) {
    let id = window_id(app);
    let mut state = ctx.data_mut(|data| data.get_temp::<ResultsState>(id).unwrap_or_default());
    if !state.open {
        return;
    }
    let batches = app.session.data_imports.batches().clone();
    if !batches.iter().any(|batch| Some(batch.id) == state.selected) {
        state.selected = batches.back().map(|batch| batch.id);
    }
    let mut retry = None;
    egui::Window::new("Import results")
        .id(id)
        .open(&mut state.open)
        .default_size([560.0, 360.0])
        .default_pos(ctx.content_rect().center() - egui::vec2(280.0, 180.0))
        .show(ctx, |ui| {
            if batches.is_empty() {
                ui.label("No imports yet. Open data to begin.");
                return;
            }
            egui::ComboBox::from_id_salt("import_batch")
                .selected_text(
                    state
                        .selected
                        .map_or_else(String::new, |id| format!("Batch {id}")),
                )
                .show_ui(ui, |ui| {
                    for batch in batches.iter().rev() {
                        ui.selectable_value(
                            &mut state.selected,
                            Some(batch.id),
                            format!(
                                "Batch {} — {}",
                                batch.id,
                                batch
                                    .recent
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                            ),
                        );
                    }
                });
            let Some(batch) = batches
                .iter()
                .find(|batch| Some(batch.id) == state.selected)
            else {
                return;
            };
            ui.label(summary(batch));
            if ui
                .link("Warnings and details in operation history")
                .clicked()
            {
                commands::execute_without_clipboard(CommandId::OperationHistory, app, ctx);
                super::activity::open_diagnostics(app, ctx);
            }
            if batch.retryable {
                let command = CommandId::RetryImportBatch(batch.id);
                let descriptor = commands::describe(app, command);
                let response =
                    ui.add_enabled(descriptor.enabled, egui::Button::new("Retry failed items"));
                let response = if let Some(reason) = descriptor.disabled_reason {
                    response.on_disabled_hover_text(reason)
                } else {
                    response
                };
                if response.clicked() {
                    retry = Some(command);
                }
            } else if batch.failed() > 0 {
                ui.weak("Open the source again to retry this import.");
            }
            ui.separator();
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (index, item) in batch.items.iter().enumerate() {
                        ui.push_id(index, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(
                                    item.path.file_name().unwrap_or_default().to_string_lossy(),
                                );
                                match &item.result {
                                    Ok(target) => {
                                        if let Some(dataset) = app.doc.dataset_index(target.dataset)
                                        {
                                            let name = app.doc.datasets[dataset].display_name();
                                            if ui.link(format!("View {name}")).clicked() {
                                                app.cancel_import_open_intent();
                                                open_target(app, ui, target);
                                            }
                                        } else {
                                            ui.weak("Result no longer available");
                                        }
                                    }
                                    Err(error) => {
                                        ui.colored_label(ui.visuals().error_fg_color, error);
                                    }
                                }
                            });
                            ui.separator();
                        });
                    }
                });
        });
    ctx.data_mut(|data| data.insert_temp(id, state));
    if let Some(command) = retry {
        commands::execute_without_clipboard(command, app, ctx);
    }
}

#[cfg(test)]
#[path = "import_feedback_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "import_notice_tests.rs"]
mod notice_tests;
