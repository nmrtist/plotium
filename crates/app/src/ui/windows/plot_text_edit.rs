//! Editing the text of a plot's text annotation or reference-line label.

use super::*;

pub(in crate::ui) fn plot_text_edit_window(app: &mut PlotiumApp, ctx: &egui::Context) {
    let Some(edit) = app.session.ui.plot_text_edit.as_ref() else {
        return;
    };
    let (object, annotation) = (edit.object, edit.annotation);
    let Some((ci, is_text)) = app.doc.canvas_index(edit.canvas).and_then(|ci| {
        let annotations = &app.doc.canvases[ci]
            .object(object)?
            .plot()?
            .axis_overrides
            .annotations;
        if annotations.get(annotation).is_some() {
            Some((ci, true))
        } else {
            annotations.reference_line(annotation).map(|_| (ci, false))
        }
    }) else {
        app.session.ui.plot_text_edit = None;
        return;
    };

    let mut open = true;
    let mut save = false;
    let mut delete = false;
    let mut cancel = false;
    egui::Window::new(if is_text {
        "Edit text"
    } else {
        "Edit line label"
    })
    .collapsible(false)
    .resizable(false)
    .open(&mut open)
    .show(ctx, |ui| {
        crate::ui::result_follow::register_editor(ui);
        let Some(edit) = app.session.ui.plot_text_edit.as_mut() else {
            return;
        };
        // Text annotations take several lines: Enter starts a new line and
        // Command-Enter saves. Line labels stay on one line.
        let response = if is_text {
            ui.add(
                egui::TextEdit::multiline(&mut edit.buffer)
                    .desired_rows(2)
                    .desired_width(320.0),
            )
        } else {
            ui.add(egui::TextEdit::singleline(&mut edit.buffer).desired_width(320.0))
        };
        if edit.focus {
            response.request_focus();
            edit.focus = false;
        }
        // A text annotation needs some text; an empty line label removes it.
        let valid = !is_text || !edit.buffer.trim().is_empty();
        let submitted = if is_text {
            response.has_focus()
                && ui.input(|input| input.modifiers.command && input.key_pressed(egui::Key::Enter))
        } else {
            response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter))
        };
        if valid && submitted {
            save = true;
        }
        if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add_enabled(valid, egui::Button::new("Save")).clicked() {
                save = true;
            }
            if ui.button("Delete").clicked() {
                delete = true;
            }
            if ui.button("Cancel").clicked() {
                cancel = true;
            }
        });
    });

    if save {
        let buffer = app
            .session
            .ui
            .plot_text_edit
            .take()
            .map(|edit| edit.buffer)
            .unwrap_or_default();
        crate::ui::plot_annotations::set_text(app, ci, object, annotation, &buffer);
    } else if delete {
        app.session.ui.plot_text_edit = None;
        crate::ui::plot_annotations::delete(app, ci, object, annotation);
    } else if cancel || !open {
        app.session.ui.plot_text_edit = None;
    }
}
