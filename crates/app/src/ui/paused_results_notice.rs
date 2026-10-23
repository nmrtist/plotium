use egui::{Context, Ui};
use plotium_core::automation::ResourceRef;
use plotium_core::state::{PlotiumApp, ResultFollowMode};

use super::commands::{self, CommandId};

const VISIBLE_SECONDS: f64 = 8.0;

#[derive(Clone, Default)]
struct Reminder {
    resources: Vec<ResourceRef>,
    dismissed: bool,
    visible_seconds: f64,
    last_visible: Option<f64>,
}

fn key(app: &PlotiumApp) -> egui::Id {
    egui::Id::new((
        "paused_results_reminder",
        app.session.project_context.to_string(),
        app.session.result_follow.source(),
        app.session.result_follow.pause_epoch(),
    ))
}

fn unseen_resources(app: &PlotiumApp) -> Vec<ResourceRef> {
    if app.session.result_follow.mode() != ResultFollowMode::Paused {
        return Vec::new();
    }
    let Some(source) = app.session.result_follow.source() else {
        return Vec::new();
    };
    app.session
        .background_activity
        .results()
        .iter()
        .filter(|row| {
            row.source == Some(source)
                && app
                    .session
                    .result_follow
                    .is_unseen(&row.resource, row.operation)
                && app.activity_result_available(&row.resource)
                && super::result_follow::navigable(app, &row.resource)
        })
        .map(|row| row.resource.clone())
        .collect()
}

pub(super) fn gate(app: &PlotiumApp, command: CommandId) -> Result<(), &'static str> {
    if command == CommandId::DisablePausedResultReminders
        && !app.settings.general.paused_result_reminders
    {
        return Err("Enable paused-result reminders in Preferences before disabling them.");
    }
    if unseen_resources(app).is_empty() {
        return Err("Pause following and wait for a new or updated result.");
    }
    Ok(())
}

// Observing a pending reminder never starts its timer: errors can occupy the banner.
pub(super) fn candidate(app: &PlotiumApp, ctx: &Context) -> Option<usize> {
    if !app.settings.general.paused_result_reminders {
        return None;
    }
    let resources = unseen_resources(app);
    if resources.is_empty() {
        return None;
    }
    let id = key(app);
    let mut reminder = ctx.data(|data| data.get_temp::<Reminder>(id).unwrap_or_default());
    if resources
        .iter()
        .any(|resource| !reminder.resources.contains(resource))
    {
        reminder.resources = resources.clone();
        reminder.dismissed = false;
        reminder.visible_seconds = 0.0;
        reminder.last_visible = None;
    }
    let visible = !reminder.dismissed && reminder.visible_seconds < VISIBLE_SECONDS;
    ctx.data_mut(|data| data.insert_temp(id, reminder));
    visible.then_some(resources.len())
}

pub(super) fn execute(app: &mut PlotiumApp, ctx: &Context, command: CommandId) {
    if gate(app, command).is_err() {
        return;
    }
    let _ = candidate(app, ctx);
    let id = key(app);
    let mut reminder = ctx.data(|data| data.get_temp::<Reminder>(id).unwrap_or_default());
    reminder.dismissed = true;
    ctx.data_mut(|data| data.insert_temp(id, reminder));
    if command == CommandId::DisablePausedResultReminders {
        app.settings.general.paused_result_reminders = false;
        if !app.persist_settings() {
            let mut dialog = plotium_core::state::SettingsDialog::new(app.settings.clone());
            dialog.last_error = Some(app.session.status.clone());
            app.session.ui.settings_dialog = Some(dialog);
        }
    }
}

pub(super) fn suppress_success(
    app: &PlotiumApp,
    ctx: &Context,
    operation: plotium_core::state::ActivityId,
) -> bool {
    let resources = unseen_resources(app);
    if resources.is_empty() {
        return false;
    }
    let suppressed = if !app.settings.general.paused_result_reminders {
        resources
    } else {
        let Some(reminder) = ctx.data(|data| data.get_temp::<Reminder>(key(app))) else {
            return false;
        };
        if !reminder.dismissed && reminder.visible_seconds < VISIBLE_SECONDS {
            return false;
        }
        reminder.resources
    };
    app.session.background_activity.results().iter().any(|row| {
        row.operation == operation
            && row.source == app.session.result_follow.source()
            && suppressed.contains(&row.resource)
    })
}

pub(super) fn hidden(app: &PlotiumApp, ctx: &Context) {
    let id = key(app);
    if let Some(mut reminder) = ctx.data(|data| data.get_temp::<Reminder>(id)) {
        reminder.last_visible = None;
        ctx.data_mut(|data| data.insert_temp(id, reminder));
    }
}

pub(super) fn show(app: &mut PlotiumApp, ui: &mut Ui, dark: bool, count: usize) {
    let color = ui.visuals().selection.stroke.color;
    let mut hovered = false;
    let response = egui::Panel::top("operation_feedback")
        .frame(
            egui::Frame::new()
                .fill(color.linear_multiply(if dark { 0.16 } else { 0.10 }))
                .inner_margin(egui::Margin::symmetric(12, 8))
                .stroke(egui::Stroke::new(1.0_f32, color.linear_multiply(0.55))),
        )
        .show_separator_line(false)
        .show_inside(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(
                    color,
                    egui::RichText::new(egui_phosphor::regular::PAUSE_CIRCLE).size(18.0),
                );
                ui.label(format!("Following paused · {count} new or updated results"));
                for (command, label, help) in [
                    (
                        CommandId::FollowResults,
                        "Resume following",
                        "Show the latest results and follow future results.",
                    ),
                    (
                        CommandId::DismissPausedResults,
                        "Dismiss",
                        "Hide this reminder. Unseen results stay counted; the AI continues.",
                    ),
                    (
                        CommandId::DisablePausedResultReminders,
                        "Don't remind again",
                        "Turn off paused-result reminders. Restore them in Preferences > General.",
                    ),
                ] {
                    let info = commands::describe(app, command);
                    let response = ui
                        .add_enabled(info.enabled, egui::Button::new(label))
                        .on_hover_text(help);
                    hovered |= response.hovered() || response.has_focus();
                    if response.clicked() {
                        commands::execute_without_clipboard(command, app, ui.ctx());
                    }
                }
            });
        });
    hovered |= response.response.contains_pointer();
    let id = key(app);
    let now = ui.ctx().input(|input| input.time);
    let mut reminder = ui
        .ctx()
        .data(|data| data.get_temp::<Reminder>(id).unwrap_or_default());
    if !hovered
        && ui.ctx().input(|input| input.focused)
        && let Some(last) = reminder.last_visible
    {
        // Do not count time spent behind a higher-priority report or an inactive app.
        reminder.visible_seconds += (now - last).clamp(0.0, 0.2);
    }
    reminder.last_visible = Some(now);
    ui.ctx().data_mut(|data| data.insert_temp(id, reminder));
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(100));
}

#[cfg(test)]
#[path = "paused_results_notice_tests.rs"]
mod tests;
