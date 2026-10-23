use egui::{Context, Ui};
use plotium_core::automation::{ProjectResourceProvider, ResourceProvider, ResourceRef};
use plotium_core::operation::OperationOutcome;
use plotium_core::state::{
    ActivityChange, ActivityId, BoardFrameId, CanvasId, DatasetId, ObjectId, PlotiumApp,
    PrimaryView,
};

#[derive(Clone, Default)]
struct NoticeState {
    seen_at: Option<f64>,
    dismissed: bool,
    eligible: Option<bool>,
}

pub(super) struct ActivityNotice {
    pub id: ActivityId,
    pub outcome: OperationOutcome,
    pub summary: String,
}

pub(super) enum BannerNotice {
    Import(super::import_feedback::ImportNotice),
    Activity(ActivityNotice),
}

pub(super) fn banner_notice(app: &PlotiumApp, ctx: &Context) -> Option<BannerNotice> {
    let import = super::import_feedback::notice(app, ctx);
    let activity = notice(app, ctx);
    match (import, activity) {
        (_, Some(activity)) if activity.outcome != OperationOutcome::Success => {
            Some(BannerNotice::Activity(activity))
        }
        (Some(import), _) => Some(BannerNotice::Import(import)),
        (_, Some(activity)) => Some(BannerNotice::Activity(activity)),
        _ => None,
    }
}

pub(super) fn has_pending_warning(app: &PlotiumApp, ctx: &Context) -> bool {
    app.session
        .background_activity
        .operations()
        .iter()
        .any(|operation| {
            operation.outcome != OperationOutcome::Success
                && !ctx
                    .data(|data| data.get_temp::<NoticeState>(notice_id(app, operation.id)))
                    .is_some_and(|state| state.dismissed)
        })
}

fn notice_id(app: &PlotiumApp, id: ActivityId) -> egui::Id {
    egui::Id::new((
        "activity_notice",
        app.session.project_context.to_string(),
        id,
    ))
}

pub(super) fn notice(app: &PlotiumApp, ctx: &Context) -> Option<ActivityNotice> {
    let operations = app.session.background_activity.operations();
    for operation in operations {
        let key = notice_id(app, operation.id);
        let mut state = ctx.data_mut(|data| data.get_temp::<NoticeState>(key).unwrap_or_default());
        if state.eligible.is_none() {
            state.eligible = Some(
                operation.outcome != OperationOutcome::Success
                    || operation.created > 0
                    || app.session.background_activity.results().iter().any(|row| {
                        row.operation == operation.id
                            && (row.updates == 1
                                || (row.source.is_none_or(|source| {
                                    app.session.result_follow.source() == Some(source)
                                }) && currently_viewed(app, &row.resource)))
                    }),
            );
            ctx.data_mut(|data| data.insert_temp(key, state));
        }
    }
    let eligible = |operation: &&plotium_core::state::ActivityOperation| {
        ctx.data(|data| data.get_temp::<NoticeState>(notice_id(app, operation.id)))
            .is_some_and(|state| !state.dismissed && state.eligible == Some(true))
    };
    let operation = operations
        .iter()
        .rev()
        .filter(eligible)
        .find(|operation| operation.outcome != OperationOutcome::Success)
        .or_else(|| operations.iter().rev().find(eligible))?;
    let id = notice_id(app, operation.id);
    let now = ctx.input(|input| input.time);
    let mut state = ctx.data_mut(|data| data.get_temp::<NoticeState>(id).unwrap_or_default());
    let first_seen = *state.seen_at.get_or_insert(now);
    let expired = operation.outcome == OperationOutcome::Success && now - first_seen >= 3.0;
    ctx.data_mut(|data| data.insert_temp(id, state.clone()));
    if state.dismissed || expired {
        return None;
    }
    if operation.outcome == OperationOutcome::Success {
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
    Some(ActivityNotice {
        id: operation.id,
        outcome: operation.outcome,
        summary: super::scientific_task_feedback::notice_summary(app, operation.id).unwrap_or_else(
            || {
                let summary = format!(
                    "{} · {} created, {} updated, {} failed, {} skipped",
                    operation.title,
                    operation.created,
                    operation.updated,
                    operation.failed,
                    operation.skipped
                );
                let names: Vec<_> = app
                    .session
                    .background_activity
                    .results()
                    .iter()
                    .filter(|row| row.operation == operation.id)
                    .map(|row| row.name.as_str())
                    .take(2)
                    .collect();
                if names.is_empty() {
                    summary
                } else {
                    format!("{summary} · {}", names.join(", "))
                }
            },
        ),
    })
}

fn currently_viewed(app: &PlotiumApp, resource: &ResourceRef) -> bool {
    if !app.activity_result_available(resource) {
        return false;
    }
    let canvas = CanvasId::try_from(resource).ok().or_else(|| {
        (resource.kind.0 == plotium_core::automation::KIND_CANVAS_OBJECT)
            .then(|| resource.parent_id.as_deref()?.parse::<CanvasId>().ok())
            .flatten()
    });
    let dataset = DatasetId::try_from(resource)
        .ok()
        .or_else(|| resource.parent_id.as_deref()?.parse::<DatasetId>().ok());
    if app.session.view == PrimaryView::Data {
        return dataset.is_some_and(|id| {
            app.active_dataset()
                .is_some_and(|index| app.doc.datasets[index].resource_id() == id)
        });
    }
    let visible_canvas = |id| {
        app.session.active_canvas.is_some_and(|index| {
            app.doc
                .canvases
                .get(index)
                .is_some_and(|canvas| canvas.resource_id == id)
        }) || app
            .session
            .ui
            .frame_selection
            .contains(&BoardFrameId::Page(id))
    };
    canvas.is_some_and(visible_canvas)
        || dataset.is_some_and(|id| {
            app.session
                .ui
                .frame_selection
                .contains(&BoardFrameId::Sheet(id))
                || app.doc.canvases.iter().enumerate().any(|(index, canvas)| {
                    visible_canvas(canvas.resource_id)
                        && app
                            .doc
                            .page_dataset_indices(index)
                            .iter()
                            .any(|index| app.doc.datasets[*index].resource_id() == id)
                })
        })
}

pub(super) fn dismiss_notice(app: &PlotiumApp, ctx: &Context, id: ActivityId) {
    let key = notice_id(app, id);
    let mut state = ctx.data_mut(|data| data.get_temp::<NoticeState>(key).unwrap_or_default());
    state.dismissed = true;
    ctx.data_mut(|data| data.insert_temp(key, state));
}

pub(super) fn results(app: &mut PlotiumApp, ui: &mut Ui) {
    super::result_follow::controls(app, ui);
    let tasks = app.session.scientific_tasks.list(0, 128);
    let active: Vec<_> = tasks
        .iter()
        .filter(|task| {
            task.project_context == app.session.project_context && !task.status.terminal()
        })
        .collect();
    let has_active = !active.is_empty();
    if has_active {
        ui.label(crate::typography::headline("Scientific tasks"));
        let registry = plotium_core::automation::ToolRegistry::built_in();
        for task in active {
            ui.horizontal_wrapped(|ui| {
                ui.spinner();
                let state = task.status.label();
                let title = registry
                    .get(&task.operation)
                    .map_or(task.operation.as_str(), |tool| tool.title.as_str());
                ui.label(format!("{title} · {state}"));
                if let (Some(done), Some(total)) = (task.completed, task.total) {
                    ui.weak(format!("{done}/{total}"));
                }
            });
        }
        ui.weak("Editing the project while a task runs can prevent its result from being applied. You can recompute using current data afterward.");
        ui.separator();
    }
    let rows = app.session.background_activity.results().clone();
    if rows.is_empty() && app.session.background_activity.operations().is_empty() {
        ui.weak(if has_active {
            "No completed results yet. Wait for the running task to finish."
        } else {
            "No results yet. Run a scientific operation or ask your connected assistant to create or update data or a figure."
        });
        return;
    }
    for row in rows.iter().rev() {
        let descriptor = ProjectResourceProvider::new(app)
            .inspect(&row.resource.id)
            .filter(|descriptor| descriptor.resource == row.resource);
        let available = app.activity_result_available(&row.resource)
            && navigation_available(app, &row.resource);
        let name = descriptor
            .as_ref()
            .map_or_else(|| row.name.clone(), |descriptor| descriptor.name.clone());
        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(match row.change {
                    ActivityChange::Created => "Created",
                    ActivityChange::Updated => "Updated",
                });
                let response = ui.add_enabled(available, egui::Button::new(&name).frame(false));
                if response.clicked() {
                    super::result_follow::pause(app, "You opened a result.");
                    if open_result(app, ui, &row.resource) {
                        app.session
                            .result_follow
                            .acknowledge(row.resource.clone(), row.operation);
                    }
                }
                if !available {
                    response.on_disabled_hover_text(if descriptor.is_some() {
                        "This result has no content to open in the workspace."
                    } else {
                        "This result is no longer available in the current project."
                    });
                    ui.weak("Unavailable");
                }
                if row.updates > 1 {
                    ui.weak(format!("{} changes", row.updates));
                }
            });
            ui.weak(&row.title);
        });
        ui.add_space(4.0);
    }
    if !rows.is_empty() {
        ui.separator();
    }
    ui.label(crate::typography::headline("Recent operations"));
    let operations = app.session.background_activity.operations().clone();
    for operation in operations.iter().rev() {
        egui::CollapsingHeader::new(format!(
            "{} · {}",
            operation.title,
            super::scientific_task_feedback::status_label(app, operation.id)
                .unwrap_or(operation.outcome.as_str())
        ))
        .id_salt(operation.id)
        .default_open(operation.outcome != OperationOutcome::Success)
        .show(ui, |ui| {
            if super::scientific_task_feedback::receipt(app, operation.id).is_some() {
                super::scientific_task_feedback::details(app, ui, operation.id);
                return;
            }
            ui.weak(format!(
                "{} created · {} updated · {} failed · {} skipped",
                operation.created, operation.updated, operation.failed, operation.skipped
            ));
            for diagnostic in &operation.diagnostics {
                ui.add(egui::Label::new(diagnostic).wrap());
            }
        });
    }
}

pub(super) fn open_result(app: &mut PlotiumApp, ui: &Ui, resource: &ResourceRef) -> bool {
    if !app.activity_result_available(resource) {
        return false;
    }
    if let Ok(id) = CanvasId::try_from(resource) {
        let Some(index) = app.doc.canvas_index(id) else {
            return false;
        };
        super::primary_sidebar::selection::select_canvas(app, ui, index);
        app.session.view = PrimaryView::Canvas;
        return true;
    }
    if resource.kind.0 == plotium_core::automation::KIND_CANVAS_OBJECT {
        let Some(canvas) = resource
            .parent_id
            .as_deref()
            .and_then(|id| id.parse::<CanvasId>().ok())
            .and_then(|id| app.doc.canvas_index(id))
        else {
            return false;
        };
        let Some(object) = resource
            .local_id
            .as_deref()
            .and_then(|id| id.parse::<ObjectId>().ok())
        else {
            return false;
        };
        if app.doc.canvases[canvas].object(object).is_none() {
            return false;
        }
        super::primary_sidebar::selection::select_canvas(app, ui, canvas);
        app.select_object(canvas, object);
        app.focus_object_datasets(canvas, object);
        app.session.view = PrimaryView::Canvas;
        return true;
    }
    let dataset = DatasetId::try_from(resource)
        .ok()
        .or_else(|| resource.parent_id.as_deref()?.parse::<DatasetId>().ok());
    let Some(dataset) = dataset.and_then(|id| app.doc.dataset_index(id)) else {
        return false;
    };
    let visible = (0..app.doc.datasets.len()).collect::<Vec<_>>();
    super::primary_sidebar::selection::select_dataset(
        app,
        ui,
        &visible,
        dataset,
        Default::default(),
    );
    app.session.primary_sidebar_visible = true;
    app.session.view = PrimaryView::Data;
    if app.doc.datasets[dataset].as_table().is_some() {
        app.session.ui.sheet_open = Some(dataset);
    }
    true
}

fn navigation_available(app: &PlotiumApp, resource: &ResourceRef) -> bool {
    CanvasId::try_from(resource).is_ok()
        || DatasetId::try_from(resource).is_ok()
        || resource.kind.0 == plotium_core::automation::KIND_CANVAS_OBJECT
        || resource
            .parent_id
            .as_deref()
            .and_then(|id| id.parse::<DatasetId>().ok())
            .is_some_and(|id| app.doc.dataset_index(id).is_some())
}

#[cfg(test)]
#[path = "activity_results_tests.rs"]
mod tests;
