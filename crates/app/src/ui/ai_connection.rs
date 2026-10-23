//! Fixed-size, keyboard-accessible connection entry in the Ribbon chrome.
use super::{
    clipboard_table::ClipboardTablePaste,
    commands::{self, CommandId},
};
use crate::agent_connections::{self, Indicator};
use plotium_core::state::PlotiumApp;
pub(super) fn button(app: &mut PlotiumApp, clipboard: &mut ClipboardTablePaste, ui: &mut egui::Ui) {
    let view = agent_connections::indicator(ui.ctx());
    let running = running_tasks(
        &app.session.scientific_tasks.list(0, 128),
        app.session.project_context,
    );
    let color = match view.state {
        Indicator::Off => ui.visuals().weak_text_color(),
        Indicator::Starting | Indicator::Ready => ui.visuals().hyperlink_color,
        Indicator::Attention => ui.visuals().warn_fg_color,
        Indicator::Active => {
            if ui.visuals().dark_mode {
                egui::Color32::from_rgb(89, 201, 135)
            } else {
                egui::Color32::from_rgb(24, 128, 70)
            }
        }
    };
    let response = ui.add_sized(
        [
            super::ribbon_chrome::SIDEBAR_TOGGLE_WIDTH,
            ui.spacing().interact_size.y,
        ],
        egui::Button::new(egui::RichText::new(egui_phosphor::regular::ROBOT).color(color))
            .frame_when_inactive(false),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            format!(
                "AI connections (MCP). {}. {}",
                view.state.label(),
                running.join(". ")
            ),
        )
    });
    let mark = response.rect.right_bottom() + egui::vec2(-5.0, -5.0);
    if !running.is_empty() {
        egui::Spinner::new().size(10.0).color(color).paint_at(
            ui,
            egui::Rect::from_center_size(mark, egui::vec2(10.0, 10.0)),
        );
    } else {
        match view.state {
            Indicator::Off => {
                ui.painter().line_segment(
                    [mark + egui::vec2(-3.0, 3.0), mark + egui::vec2(3.0, -3.0)],
                    egui::Stroke::new(1.5_f32, color),
                );
            }
            Indicator::Starting | Indicator::Ready => {
                ui.painter()
                    .circle_stroke(mark, 2.5, egui::Stroke::new(1.2_f32, color));
            }
            Indicator::Active => {
                ui.painter().circle_filled(mark, 2.5, color);
            }
            Indicator::Attention => {
                ui.painter().text(
                    mark,
                    egui::Align2::CENTER_CENTER,
                    "!",
                    egui::FontId::proportional(12.0),
                    color,
                );
            }
        }
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect.shrink(1.0),
            3.0,
            ui.visuals().selection.stroke,
            egui::StrokeKind::Inside,
        );
    }
    let tooltip = if running.is_empty() {
        view.tooltip
    } else {
        format!(
            "{}\n\n{}\nEditing the project while a task runs can prevent its result from being applied. You can recompute using current data afterward.",
            view.tooltip,
            running.join("\n")
        )
    };
    let response = response.on_hover_text(tooltip);
    if response.clicked() {
        commands::execute(CommandId::AgentConnections, app, clipboard, ui.ctx());
    }
}

fn running_tasks(
    tasks: &[plotium_core::automation::TaskReceipt],
    context: plotium_core::state::ProjectContextId,
) -> Vec<String> {
    let tasks: Vec<_> = tasks
        .iter()
        .filter(|task| task.project_context == context && !task.status.terminal())
        .collect();
    if tasks.is_empty() {
        return Vec::new();
    }
    let registry = plotium_core::automation::ToolRegistry::built_in();
    tasks
        .iter()
        .map(|task| {
            let title = registry
                .get(&task.operation)
                .map_or(task.operation.as_str(), |tool| tool.title.as_str());
            let progress = match (task.completed, task.total) {
                (Some(done), Some(total)) => format!(" · {done}/{total}"),
                _ => String::new(),
            };
            format!("{title} · {}{progress}", task.status.label())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use plotium_core::automation::{DocumentRevision, TaskId, TaskReceipt, TaskStatus};
    use plotium_core::state::ProjectContextId;

    #[test]
    fn running_indicator_reports_real_progress_and_excludes_completed_or_other_project_tasks() {
        let context = ProjectContextId::default();
        let receipt = TaskReceipt {
            task_id: TaskId("running".into()),
            plan_id: None,
            operation: "processing.edit".into(),
            project_context: context,
            targets: Vec::new(),
            sources: Vec::new(),
            start_revision: DocumentRevision(0),
            end_revision: None,
            status: TaskStatus::Running,
            phase: "columns".into(),
            completed: Some(2),
            total: Some(5),
            failure_code: None,
            message: None,
            produced: Vec::new(),
            modified: Vec::new(),
            summary: serde_json::Value::Null,
        };
        let mut completed = receipt.clone();
        completed.status = TaskStatus::Succeeded;
        let mut other = receipt.clone();
        other.project_context = ProjectContextId::default();
        let lines = running_tasks(&[receipt, completed, other], context);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("Running · 2/5"));
        assert!(!lines[0].contains("Completed"));
    }
}
