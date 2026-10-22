use egui::Ui;
use egui_phosphor::regular as icon;
use plotium_core::state::PlotiumApp;

use super::clipboard_table::ClipboardTablePaste;
use super::commands::{self, CommandId};

pub(super) fn messages_tab_id() -> egui::Id {
    egui::Id::new("operation_history_messages_tab")
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum HistoryTab {
    Results,
    Messages,
    #[default]
    Diagnostics,
}

pub(super) fn history_tab(ctx: &egui::Context) -> HistoryTab {
    ctx.data(|data| data.get_temp(messages_tab_id()).unwrap_or_default())
}

pub(super) fn set_history_tab(ctx: &egui::Context, tab: HistoryTab) {
    ctx.data_mut(|data| data.insert_temp(messages_tab_id(), tab));
}

pub(super) fn observe(app: &mut PlotiumApp) {
    app.session.status_history.observe(&app.session.status);
}

pub(crate) fn open_diagnostics(app: &mut PlotiumApp, ctx: &egui::Context) {
    app.session.ui.diagnostics_open = true;
    set_history_tab(ctx, HistoryTab::Diagnostics);
}

pub(crate) fn open_history(app: &mut PlotiumApp, ctx: &egui::Context) {
    if has_results_or_active_tasks(app) {
        open_results(app, ctx);
    } else {
        open_diagnostics(app, ctx);
    }
}

fn has_results_or_active_tasks(app: &PlotiumApp) -> bool {
    !app.session.background_activity.results().is_empty()
        || !app.session.background_activity.operations().is_empty()
        || app
            .session
            .scientific_tasks
            .list(0, 128)
            .iter()
            .any(|task| {
                task.project_context == app.session.project_context && !task.status.terminal()
            })
}

pub(super) fn open_results(app: &mut PlotiumApp, ctx: &egui::Context) {
    app.session.ui.diagnostics_open = true;
    set_history_tab(ctx, HistoryTab::Results);
}

pub(super) fn history_button(
    app: &mut PlotiumApp,
    clipboard: &mut ClipboardTablePaste,
    ui: &mut Ui,
) {
    let count = app
        .session
        .background_activity
        .results()
        .iter()
        .filter(|row| app.activity_result_available(&row.resource))
        .count();
    let response = ui
        .add_sized(
            [
                super::ribbon_chrome::SIDEBAR_TOGGLE_WIDTH,
                ui.spacing().interact_size.y,
            ],
            egui::Button::new(icon::CLOCK_COUNTER_CLOCKWISE)
                .frame_when_inactive(false)
                .selected(app.session.ui.diagnostics_open),
        )
        .on_hover_text(format!(
            "Operation history · {count} available results\n{}",
            app.session.status
        ));
    if count > 0 {
        ui.painter().text(
            response.rect.right_bottom() + egui::vec2(-2.0, -1.0),
            egui::Align2::RIGHT_BOTTOM,
            if count > 99 {
                "99+".into()
            } else {
                count.to_string()
            },
            egui::FontId::proportional(9.0),
            ui.visuals().text_color(),
        );
    }
    if super::pending_feedback(app).is_some()
        || super::activity_results::has_pending_warning(app, ui.ctx())
    {
        ui.painter().circle_filled(
            response.rect.right_top() + egui::vec2(-5.0, 5.0),
            2.5,
            ui.visuals().warn_fg_color,
        );
    }
    if response.clicked() {
        commands::execute(CommandId::OperationHistory, app, clipboard, ui.ctx());
        set_history_tab(
            ui.ctx(),
            if has_results_or_active_tasks(app) {
                HistoryTab::Results
            } else {
                HistoryTab::Messages
            },
        );
    }
}

pub(super) fn messages(app: &PlotiumApp, ui: &mut Ui) {
    let count = app.session.status_history.messages().len();
    ui.vertical(|ui| {
        if count == 0 {
            ui.weak("No messages yet. Open data to begin.");
        }
        for message in app.session.status_history.messages().rev() {
            let elapsed = message.recorded_at.elapsed().unwrap_or_default().as_secs();
            ui.horizontal_top(|ui| {
                ui.add_sized(
                    [56.0, ui.spacing().interact_size.y],
                    egui::Label::new(
                        crate::typography::caption(format!(
                            "{}:{:02} ago",
                            elapsed / 60,
                            elapsed % 60
                        ))
                        .color(ui.visuals().weak_text_color()),
                    ),
                );
                ui.add(egui::Label::new(&message.text).wrap().selectable(true));
            });
            ui.separator();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_links_reveal_details_after_browsing_messages() {
        let ctx = egui::Context::default();
        let mut app = PlotiumApp::new_with_settings(plotium_core::settings::Settings::default());
        set_history_tab(&ctx, HistoryTab::Messages);
        open_diagnostics(&mut app, &ctx);
        assert!(app.session.ui.diagnostics_open);
        assert_eq!(history_tab(&ctx), HistoryTab::Diagnostics);
    }

    #[test]
    fn pending_only_current_project_task_opens_results_but_foreign_task_does_not() {
        use plotium_core::automation::{
            CallerType, DocumentRevision, ExecutionAuthority, TargetSelector, ToolRequest,
            execute_tool, plan_tool,
        };
        let mut app = PlotiumApp::new_with_settings(Default::default());
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../io/tests/fixtures/nmr/jcamp-ppm.dx");
        app.load_from(&path);
        assert_eq!(app.doc.datasets.len(), 1);
        let plan = plan_tool(&app, ToolRequest {
            tool_id: "processing.edit".into(),
            tool_version: 1,
            parameters: serde_json::json!({"edits": [{"op": "add", "axis": "direct", "kind": "Invert"}]}),
            targets: TargetSelector::Explicit { ids: vec![app.doc.datasets[0].resource_id().to_string()] },
            expected_revision: DocumentRevision(app.doc.automation_revision),
            caller: CallerType::Human,
        }).unwrap();
        execute_tool(&mut app, plan, ExecutionAuthority::ReversibleModify).unwrap();
        assert!(app.session.background_activity.operations().is_empty());
        assert!(app.session.background_activity.results().is_empty());
        assert!(
            app.session
                .scientific_tasks
                .list(0, 128)
                .iter()
                .any(|task| !task.status.terminal())
        );
        let ctx = egui::Context::default();
        assert!(has_results_or_active_tasks(&app));
        open_history(&mut app, &ctx);
        assert_eq!(history_tab(&ctx), HistoryTab::Results);
        app.session.project_context = Default::default();
        assert!(!has_results_or_active_tasks(&app));
        open_history(&mut app, &ctx);
        assert_eq!(history_tab(&ctx), HistoryTab::Diagnostics);
    }
}
