//! Save the active canvas's look as a user style, and apply or delete saved
//! styles. Built-in themes stay in the Canvas Theme menu.

use plotium_core::state::{CanvasStyleDialogState, CanvasStyleEntry, PlotiumApp};
use plotium_core::theme;

pub(crate) fn open_save_dialog(app: &mut PlotiumApp) {
    if app.session.active_canvas.is_none() {
        return;
    }
    app.session.ui.canvas_style_dialog = Some(CanvasStyleDialogState::SaveAs {
        name: String::new(),
    });
}

pub(crate) fn open_browser(app: &mut PlotiumApp) {
    app.session.ui.canvas_style_dialog = Some(CanvasStyleDialogState::Browse {
        entries: browse_entries(),
        confirm_delete: None,
    });
}

fn browse_entries() -> Vec<CanvasStyleEntry> {
    let Some(dir) = theme::styles_dir() else {
        return Vec::new();
    };
    theme::list_user_styles(&dir)
        .into_iter()
        .map(|info| CanvasStyleEntry {
            theme: theme::load_user_style(&dir, &info.name).map_err(|e| e.to_string()),
            name: info.name,
        })
        .collect()
}

pub(crate) fn canvas_style_window(app: &mut PlotiumApp, ctx: &egui::Context) {
    let Some(mut dialog) = app.session.ui.canvas_style_dialog.take() else {
        return;
    };
    let keep = match &mut dialog {
        CanvasStyleDialogState::SaveAs { name } => save_as_window(app, ctx, name),
        CanvasStyleDialogState::Browse {
            entries,
            confirm_delete,
        } => browse_window(app, ctx, entries, confirm_delete),
    };
    if keep && app.session.ui.canvas_style_dialog.is_none() {
        app.session.ui.canvas_style_dialog = Some(dialog);
    }
}

fn save_as_window(app: &mut PlotiumApp, ctx: &egui::Context, name: &mut String) -> bool {
    let Some(dir) = theme::styles_dir() else {
        app.session.status = "No configuration folder is available for saving styles.".into();
        return false;
    };
    let mut save = false;
    let mut cancel = false;
    let modal =
        super::modal(ctx, "canvas_style_save_modal", super::ModalKind::Dialog).show(ctx, |ui| {
            ui.heading("Save canvas style");
            ui.separator();
            ui.label(
                "Save this canvas's background, text sizes, axis and tick strokes, curve \
                 colours, line width and marker size, fill, outline and hollow setting as \
                 a reusable style.",
            );
            ui.add(
                egui::TextEdit::singleline(name)
                    .hint_text("Style name")
                    .desired_width(280.0),
            );
            let validated = theme::validate_style_name(name);
            if let Err(error) = &validated
                && !name.trim().is_empty()
            {
                ui.colored_label(ui.visuals().warn_fg_color, error.to_string());
            }
            let exists = validated
                .as_ref()
                .is_ok_and(|n| theme::user_style_exists(&dir, n));
            if exists {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    "A style with this name already exists.",
                );
            }
            ui.separator();
            ui.horizontal(|ui| {
                let label = if exists { "Overwrite" } else { "Save" };
                if ui
                    .add_enabled(validated.is_ok(), egui::Button::new(label))
                    .clicked()
                {
                    save = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
    if save {
        commit_save(app, &dir, name);
        return false;
    }
    !(cancel || modal.should_close())
}

fn commit_save(app: &mut PlotiumApp, dir: &std::path::Path, name: &str) {
    let name = name.trim();
    let Some(style) = app
        .session
        .active_canvas
        .and_then(|canvas| app.theme_from_canvas(canvas, name))
    else {
        app.session.status = "Open a canvas before saving its style.".into();
        return;
    };
    app.session.status = match theme::save_user_style(dir, name, &style) {
        Ok(_) => format!("Saved style \"{name}\"."),
        Err(error) => format!("Could not save style \"{name}\": {error}"),
    };
}

enum RowAction {
    Apply(usize),
    Delete(usize),
}

fn browse_window(
    app: &mut PlotiumApp,
    ctx: &egui::Context,
    entries: &mut Vec<CanvasStyleEntry>,
    confirm_delete: &mut Option<String>,
) -> bool {
    let has_canvas = app.session.active_canvas.is_some();
    let mut action = None;
    let mut close = false;
    let modal =
        super::modal(ctx, "canvas_style_browse_modal", super::ModalKind::Dialog).show(ctx, |ui| {
            ui.set_width(520.0);
            ui.heading("Canvas styles");
            ui.separator();
            if !has_canvas {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    "Open a canvas to apply a style.",
                );
            }
            if entries.is_empty() {
                ui.weak("No styles saved yet. Use \"Save Canvas Style…\" to save one.");
            }
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    for (i, entry) in entries.iter().enumerate() {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(crate::typography::headline(&entry.name));
                            match &entry.theme {
                                Ok(style) => palette_swatches(ui, &style.trace_palette),
                                Err(reason) => {
                                    ui.colored_label(ui.visuals().warn_fg_color, "Unreadable");
                                    ui.weak(reason);
                                }
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let confirming =
                                        confirm_delete.as_deref() == Some(entry.name.as_str());
                                    let delete_label = if confirming {
                                        "Confirm delete"
                                    } else {
                                        "Delete"
                                    };
                                    if ui.button(delete_label).clicked() {
                                        if confirming {
                                            action = Some(RowAction::Delete(i));
                                        } else {
                                            *confirm_delete = Some(entry.name.clone());
                                        }
                                    }
                                    if ui
                                        .add_enabled(
                                            has_canvas && entry.theme.is_ok(),
                                            egui::Button::new("Apply"),
                                        )
                                        .clicked()
                                    {
                                        action = Some(RowAction::Apply(i));
                                    }
                                },
                            );
                        });
                        ui.separator();
                    }
                });
            if ui.button("Close").clicked() {
                close = true;
            }
        });

    match action {
        Some(RowAction::Apply(i)) => {
            if let Ok(style) = &entries[i].theme {
                app.apply_theme(style);
            }
            false
        }
        Some(RowAction::Delete(i)) => {
            let name = entries[i].name.clone();
            app.session.status = match theme::styles_dir() {
                Some(dir) => match theme::delete_user_style(&dir, &name) {
                    Ok(()) => format!("Deleted style \"{name}\"."),
                    Err(error) => format!("Could not delete style \"{name}\": {error}"),
                },
                None => format!("Could not delete style \"{name}\": no configuration folder."),
            };
            *entries = browse_entries();
            *confirm_delete = None;
            true
        }
        None => !(close || modal.should_close()),
    }
}

fn palette_swatches(ui: &mut egui::Ui, palette: &[plotium_figure::Color]) {
    for color in palette {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
        ui.painter().rect_filled(
            rect,
            2.0,
            egui::Color32::from_rgb(color.r, color.g, color.b),
        );
    }
}
