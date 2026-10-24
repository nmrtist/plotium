//! Inspector section for the plot annotation selected on the canvas.

use super::*;
use crate::ui::plot_annotations;
use plotium_core::state::{
    ARROW_LINE_WIDTH_PT, LabelPlacement, MAX_REFERENCE_LINE_WIDTH_PT, MIN_REFERENCE_LINE_WIDTH_PT,
};
use plotium_figure::{ArrowHead, LineStyle};

fn color_edit(ui: &mut Ui, color: &mut Color) -> bool {
    let mut rgb = [color.r, color.g, color.b];
    let changed = ui
        .horizontal(|ui| {
            ui.label("Color");
            egui::color_picker::color_edit_button_srgb(ui, &mut rgb).changed()
        })
        .inner;
    if changed {
        *color = Color::rgb(rgb[0], rgb[1], rgb[2]);
    }
    changed
}

fn text_row(ui: &mut Ui, label: &str, text: &str) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        let shown = if text.is_empty() { "None" } else { text };
        ui.add(egui::Label::new(shown).truncate());
        ui.button("Edit…").clicked()
    })
    .inner
}

/// Controls of the annotation selected in plot `object`, edited live and
/// committed as one undo step when the gesture ends.
pub(super) fn annotation_section(app: &mut PlotiumApp, ci: usize, object: ObjectId, ui: &mut Ui) {
    let Some((selected_ci, selected_object, id)) = plot_annotations::selected(app) else {
        return;
    };
    if selected_ci != ci || selected_object != object {
        return;
    }
    let Some(before) = app.doc.canvases[ci]
        .object(object)
        .filter(|item| !item.locked)
        .and_then(|item| item.plot())
        .map(|plot| plot.axis_overrides.clone())
    else {
        return;
    };
    let mut after = before.clone();
    let annotations = &mut after.annotations;
    let mut edit_text = false;
    let mut auto = false;
    ui.separator();
    if let Some(text) = annotations.get_mut(id) {
        ui.label(crate::typography::headline("Text annotation"));
        edit_text = text_row(ui, "Text", &text.text);
        ui.horizontal(|ui| {
            ui.label("Size");
            ui.add(
                DragValue::new(&mut text.font_size)
                    .range(1.0..=72.0)
                    .speed(0.1)
                    .suffix(" pt"),
            );
        });
        color_edit(ui, &mut text.color);
        auto = text.placement != LabelPlacement::Auto && ui.button("Automatic position").clicked();
    } else if let Some(line) = annotations.reference_line_mut(id) {
        ui.label(crate::typography::headline("Reference line"));
        edit_text = text_row(ui, "Label", &line.label);
        color_edit(ui, &mut line.color);
        ui.horizontal(|ui| {
            ui.label("Width");
            ui.add(
                DragValue::new(&mut line.width)
                    .range(MIN_REFERENCE_LINE_WIDTH_PT..=MAX_REFERENCE_LINE_WIDTH_PT)
                    .speed(0.05)
                    .suffix(" pt"),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Style");
            egui::ComboBox::from_id_salt(("reference_line_style", id))
                .selected_text(style_name(line.line_style))
                .show_ui(ui, |ui| {
                    for style in [
                        LineStyle::Solid,
                        LineStyle::Dashed,
                        LineStyle::Dotted,
                        LineStyle::DashDot,
                    ] {
                        ui.selectable_value(&mut line.line_style, style, style_name(style));
                    }
                });
        });
        auto = !line.label.is_empty()
            && line.label_placement != LabelPlacement::Auto
            && ui.button("Automatic label position").clicked();
    } else if let Some(arrow) = annotations.arrow_mut(id) {
        ui.label(crate::typography::headline("Arrow"));
        color_edit(ui, &mut arrow.color);
        ui.horizontal(|ui| {
            ui.label("Width");
            ui.add(
                DragValue::new(&mut arrow.line_width)
                    .range(ARROW_LINE_WIDTH_PT)
                    .speed(0.05)
                    .suffix(" pt"),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Head");
            egui::ComboBox::from_id_salt(("arrow_head", id))
                .selected_text(head_name(arrow.head))
                .show_ui(ui, |ui| {
                    for head in [
                        ArrowHead::Filled,
                        ArrowHead::Open,
                        ArrowHead::Double,
                        ArrowHead::None,
                    ] {
                        ui.selectable_value(&mut arrow.head, head, head_name(head));
                    }
                });
        });
    } else {
        return;
    }
    let delete = ui.button("Delete annotation").clicked();

    if after != before {
        if app.session.ui.axis_overrides_before.is_none() {
            app.session.ui.axis_overrides_before = Some((ci, object, before));
        }
        app.set_axis_overrides_value(ci, object, &after);
    }
    let gesture_over = !ui.input(|input| input.pointer.any_down());
    if gesture_over
        && app
            .session
            .ui
            .axis_overrides_before
            .as_ref()
            .is_some_and(|(canvas, target, _)| (*canvas, *target) == (ci, object))
    {
        app.finish_axis_overrides_edit();
    }
    if edit_text {
        plot_annotations::open_text_editor(app, ci, object, id);
    } else if auto {
        plot_annotations::place_automatically(app, ci, object, id);
    } else if delete {
        plot_annotations::delete(app, ci, object, id);
    }
}

fn style_name(style: LineStyle) -> &'static str {
    match style {
        LineStyle::Solid => "Solid",
        LineStyle::Dashed => "Dashed",
        LineStyle::Dotted => "Dotted",
        LineStyle::DashDot => "Dash-dot",
    }
}

fn head_name(head: ArrowHead) -> &'static str {
    match head {
        ArrowHead::Filled => "Filled",
        ArrowHead::Open => "Open",
        ArrowHead::Double => "Double",
        ArrowHead::None => "None",
    }
}
