use egui::Ui;
use plotium_core::automation::{TaskReceipt, TaskStatus};
use plotium_core::state::{ActivityId, PlotiumApp};

use super::commands::{self, CommandId};

pub(super) fn commands(app: &PlotiumApp) -> impl Iterator<Item = CommandId> + '_ {
    app.session
        .background_activity
        .operations()
        .iter()
        .filter_map(|operation| {
            receipt(app, operation.id).map(|_| CommandId::RecomputeScientificTask(operation.id))
        })
}

pub(super) fn receipt(app: &PlotiumApp, id: ActivityId) -> Option<&TaskReceipt> {
    let task = app
        .session
        .background_activity
        .operations()
        .iter()
        .find(|operation| operation.id == id)?
        .task_id
        .as_ref()?;
    app.session
        .scientific_tasks
        .get(task)
        .filter(|receipt| receipt.project_context == app.session.project_context)
}

pub(super) fn operation_title(app: &PlotiumApp, id: ActivityId) -> &str {
    app.session
        .background_activity
        .operations()
        .iter()
        .find(|operation| operation.id == id)
        .map_or("Scientific Task", |operation| operation.title.as_str())
}

pub(super) fn recompute_gate(app: &PlotiumApp, id: ActivityId) -> Result<(), &'static str> {
    let receipt = receipt(app, id)
        .ok_or("Open a failed, cancelled, or unapplied task in the current project's Results.")?;
    app.scientific_task_recompute_unavailable(&receipt.task_id)
        .map_or(Ok(()), Err)
}

pub(super) fn recompute(app: &mut PlotiumApp, id: ActivityId) {
    let Some(task) = receipt(app, id).map(|receipt| receipt.task_id.clone()) else {
        return;
    };
    match app.recompute_scientific_task(&task) {
        Ok(_) => app.session.status = "Recompute started using current data.".into(),
        Err(error) => app.session.status = format!("Could not recompute: {error}"),
    }
}

pub(super) fn status_label(app: &PlotiumApp, id: ActivityId) -> Option<&'static str> {
    let receipt = receipt(app, id)?;
    let warning = app
        .session
        .background_activity
        .operations()
        .iter()
        .any(|operation| {
            operation.id == id
                && operation.outcome == plotium_core::operation::OperationOutcome::Warning
        });
    Some(if receipt.status == TaskStatus::Succeeded && warning {
        "Completed · Review needed"
    } else {
        receipt.status.label()
    })
}

pub(super) fn notice_summary(app: &PlotiumApp, id: ActivityId) -> Option<String> {
    let receipt = receipt(app, id)?;
    let title = operation_title(app, id);
    let status = status_label(app, id)?;
    Some(
        match receipt.scientific_warning().or(receipt.message.as_deref()) {
            Some(message) => format!("{title} · {status}: {message}"),
            None => format!("{title} · {status}"),
        },
    )
}

pub(super) fn details(app: &mut PlotiumApp, ui: &mut Ui, id: ActivityId) {
    let Some(receipt) = receipt(app, id).cloned() else {
        return;
    };
    if let Some(message) = &receipt.message {
        ui.add(egui::Label::new(message).wrap().selectable(true));
    }
    if let Some(warning) = receipt.scientific_warning() {
        ui.add(egui::Label::new(warning).wrap().selectable(true));
    }
    if receipt.status == TaskStatus::Succeeded {
        ui.weak(
            "Execution completed. Review the result and its scientific interpretation before use.",
        );
        return;
    }
    if matches!(
        receipt.status,
        TaskStatus::Stale | TaskStatus::Failed | TaskStatus::Cancelled
    ) {
        let id = CommandId::RecomputeScientificTask(id);
        let descriptor = commands::describe(app, id);
        let response = ui.add_enabled(descriptor.enabled, egui::Button::new("Recompute"))
            .on_hover_text("Run the submitted request again using current data. Unspecified settings use current defaults. The previous result is not applied.");
        if let Some(reason) = descriptor.disabled_reason {
            response.clone().on_disabled_hover_text(reason);
            ui.weak(reason);
        }
        if response.clicked() {
            commands::execute_without_clipboard(id, app, ui.ctx());
        }
    }
}

#[cfg(test)]
#[path = "scientific_task_feedback_tests.rs"]
mod tests;
