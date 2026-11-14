use egui::{Align2, Color32, Rect, Response, Sense, Stroke, StrokeKind, Ui, vec2};
use plotium_core::state::PlotiumApp;

use crate::ui::{
    commands::{self, CommandId},
    data_navigation::CanvasComposition,
};

pub(super) fn canvas_row(
    app: &mut PlotiumApp,
    ci: usize,
    ui: &mut Ui,
    selected: bool,
    name: &str,
    summary: Option<&CanvasComposition>,
) -> Response {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 64.0), Sense::hover());
    let response = ui.interact(
        rect,
        ui.id()
            .with(("canvas_row", app.doc.canvases[ci].resource_id)),
        Sense::click(),
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            name,
        )
    });
    if !ui.is_rect_visible(rect) {
        return response;
    }
    crate::ui::file_dialogs::image_import::request_thumbnail(
        ui.ctx(),
        app.doc.canvases[ci].resource_id,
    );
    let visuals = ui.style().interact_selectable(&response, selected);
    let painter = ui.painter_at(rect);
    if selected || response.hovered() || response.has_focus() {
        painter.rect_filled(rect, 4.0, visuals.weak_bg_fill);
        painter.rect_stroke(rect, 4.0, visuals.bg_stroke, StrokeKind::Inside);
    }
    let preview = Rect::from_min_size(rect.min + vec2(6.0, 6.0), vec2(72.0, 52.0));
    painter.rect_filled(preview, 2.0, ui.visuals().faint_bg_color);
    let document = crate::ui::canvas::image_painting::canvas_document(app, ci);
    if document.width.is_finite()
        && document.height.is_finite()
        && document.width > 0.0
        && document.height > 0.0
    {
        let available = preview.shrink(3.0);
        let zoom = (available.width() / document.width).min(available.height() / document.height);
        let size = vec2(document.width, document.height) * zoom;
        let page = Rect::from_center_size(available.center(), size);
        // A thumbnail has its own viewport, independent of board navigation.
        plotium_render::screen::paint_document_for_editor_with_detail(
            &painter.with_clip_rect(preview.intersect(ui.clip_rect())),
            plotium_render::Rect::new(page.left(), page.top(), page.width(), page.height()),
            &document,
            plotium_render::DocumentViewport {
                zoom,
                pan: [0.0, 0.0],
            },
            plotium_render::screen::ScreenRenderDetail::Interactive,
        );
        painter.rect_stroke(
            page,
            0.0,
            Stroke::new(0.5_f32, Color32::from_gray(150)),
            StrokeKind::Inside,
        );
    }
    let text_rect = Rect::from_min_max(
        egui::pos2(preview.right() + 8.0, rect.top()),
        rect.max - vec2(6.0, 0.0),
    );
    let galley = egui::WidgetText::from(name).into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        text_rect.width().max(0.0),
        egui::TextStyle::Button,
    );
    let label = Align2::LEFT_CENTER.align_size_within_rect(galley.size(), text_rect);
    if let Some(summary) = summary {
        const EDIT_SIZE: f32 = 18.0;
        const EDIT_GAP: f32 = 4.0;
        let editable = !summary.datasets.is_empty();
        let summary_width = text_rect.width() - if editable { EDIT_SIZE + EDIT_GAP } else { 0.0 };
        let summary_rect = text_rect.translate(vec2(0.0, 9.0));
        painter.galley(label.min - vec2(0.0, 9.0), galley, visuals.text_color());
        let galley = egui::WidgetText::from(&summary.compact).into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            summary_width.max(0.0),
            egui::TextStyle::Small,
        );
        let line = Align2::LEFT_CENTER.align_size_within_rect(galley.size(), summary_rect);
        painter.galley(line.min, galley, ui.visuals().weak_text_color());
        if editable {
            let edit_left = (line.right() + EDIT_GAP).min(text_rect.right() - EDIT_SIZE);
            let edit_rect = Rect::from_min_size(
                egui::pos2(edit_left, line.center().y - EDIT_SIZE * 0.5),
                vec2(EDIT_SIZE, EDIT_SIZE),
            );
            edit_experiment_button(
                app,
                ui,
                response.id.with("edit_experiment"),
                edit_rect,
                summary,
            );
        }
        response.on_hover_text(format!("{name}\n{}\n{}", summary.compact, summary.details))
    } else {
        painter.galley(label.min, galley, visuals.text_color());
        response.on_hover_text(name)
    }
}

fn edit_experiment_button(
    app: &mut PlotiumApp,
    ui: &mut Ui,
    id: egui::Id,
    rect: Rect,
    summary: &CanvasComposition,
) {
    let label = "Edit experiment and tags";
    let response = ui.interact(rect, id, Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    let visuals = ui.style().interact(&response);
    let color =
        if response.hovered() || response.is_pointer_button_down_on() || response.has_focus() {
            ui.painter()
                .rect_filled(rect, visuals.corner_radius, visuals.weak_bg_fill);
            visuals.text_color()
        } else {
            ui.visuals().weak_text_color()
        };
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        egui_phosphor::regular::PENCIL_SIMPLE,
        egui::FontId::proportional(12.0),
        color,
    );
    let response = response.on_hover_text(label);
    if let [dataset] = summary.datasets.as_slice() {
        if response.clicked() {
            commands::execute_without_clipboard(
                CommandId::EditDatasetMetadata(Some(*dataset)),
                app,
                ui.ctx(),
            );
        }
    } else {
        egui::Popup::menu(&response).id(id.with("menu")).show(|ui| {
            ui.label("Choose a plotted dataset to edit");
            egui::ScrollArea::vertical()
                .max_height(240.0)
                .show(ui, |ui| {
                    for &dataset in &summary.datasets {
                        let Some(di) = app.doc.dataset_index(dataset) else {
                            continue;
                        };
                        let data = &app.doc.datasets[di];
                        let name = data.browser_name();
                        let label = data.experiment_label().map_or_else(
                            || name.clone(),
                            |experiment| format!("{name} · {experiment}"),
                        );
                        if ui
                            .button(label)
                            .on_hover_text(data.source_identifier())
                            .clicked()
                        {
                            commands::execute_without_clipboard(
                                CommandId::EditDatasetMetadata(Some(dataset)),
                                app,
                                ui.ctx(),
                            );
                            ui.close();
                        }
                    }
                });
        });
    }
}
