//! Fixed-size, keyboard-accessible connection entry in the Ribbon chrome.
use super::{
    clipboard_table::ClipboardTablePaste,
    commands::{self, CommandId},
};
use crate::agent_connections::{self, Indicator};
use plotium_core::state::PlotiumApp;
pub(super) fn button(app: &mut PlotiumApp, clipboard: &mut ClipboardTablePaste, ui: &mut egui::Ui) {
    let view = agent_connections::indicator(ui.ctx());
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
            format!("AI connections (MCP). {}", view.state.label()),
        )
    });
    let mark = response.rect.right_bottom() + egui::vec2(-5.0, -5.0);
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
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect.shrink(1.0),
            3.0,
            ui.visuals().selection.stroke,
            egui::StrokeKind::Inside,
        );
    }
    let response = response.on_hover_text(view.tooltip);
    if response.clicked() {
        commands::execute(CommandId::AgentConnections, app, clipboard, ui.ctx());
    }
}
