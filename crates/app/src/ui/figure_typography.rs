//! Document-level Figure Typography window: the tick / axis-title / figure-title
//! point sizes, axis strokes and minor ticks stamped onto every plot. Edits
//! apply live (each rebuild restamps the document value) and one slider gesture
//! coalesces into one undo step, the same contract as the canvas-size fields.

use super::*;
use plotium_core::properties::{PropertyAddress, typography};
use plotium_figure::FigureTypography;

pub(super) fn figure_typography_window(app: &mut PlotiumApp, ctx: &egui::Context) {
    if !app.session.ui.figure_typography_open {
        return;
    }
    let mut open = true;
    egui::Window::new("Figure typography")
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .show(ctx, |ui| {
            crate::ui::result_follow::register_editor(ui);
            ui.label(
                egui::RichText::new(
                    "Point sizes of every plot's axis text, axis lines and ticks in this \
                     document. Sizes are absolute (journal convention): resizing a panel \
                     never changes them.",
                )
                .small()
                .color(ui.visuals().weak_text_color()),
            );
            ui.add_space(6.0);
            crate::ui::properties::panel::typography_section(app, ui);
            ui.add_space(8.0);
            if ui.button("Reset to defaults").clicked() {
                let before = app.doc.style_library.figure_typography;
                let action = Action::set_figure_typography(before, FigureTypography::default());
                let document = app.document_target();
                let reset = typography::IDS.map(|id| PropertyAddress::new(document.clone(), id));
                app.execute_hand_edit(action, &reset, false);
            }
        });
    if !open {
        app.session.ui.figure_typography_open = false;
        app.session.ui.figure_typography_before = None;
    }
}
