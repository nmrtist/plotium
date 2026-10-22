use egui::{Ui, Vec2};
use plotium_core::state::PlotiumApp;

use crate::ui::commands::{self, CommandId};

fn workspace_contents(app: &mut PlotiumApp, ui: &mut Ui) {
    use egui_phosphor::regular as icon;
    use plotium_core::state::PrimaryView;

    ui.add_space(20.0);
    welcome_heading(ui, "Workspace overview");
    ui.label(format!(
        "{} dataset{} · {} canvas{}",
        app.doc.datasets.len(),
        if app.doc.datasets.len() == 1 { "" } else { "s" },
        app.doc.canvases.len(),
        if app.doc.canvases.len() == 1 {
            ""
        } else {
            "es"
        },
    ));
    ui.weak("Choose a canvas or dataset to continue working.");
    if !app.doc.canvases.is_empty() {
        ui.add_space(12.0);
        welcome_heading(ui, "Canvases");
        let mut selected = None;
        for canvas in &app.doc.canvases {
            ui.push_id(canvas.resource_id, |ui| {
                if welcome_action(ui, icon::FRAME_CORNERS, &canvas.name) {
                    selected = Some(canvas.resource_id);
                }
            });
        }
        if let Some(index) = selected.and_then(|id| app.doc.canvas_index(id)) {
            crate::ui::primary_sidebar::selection::select_canvas(app, ui, index);
        }
    }
    if !app.doc.datasets.is_empty() {
        ui.add_space(12.0);
        welcome_heading(ui, "Datasets");
        let mut selected = None;
        for dataset in &app.doc.datasets {
            ui.push_id(dataset.resource_id(), |ui| {
                let label = if dataset.as_table().is_some() {
                    format!("{} — View table", dataset.display_name())
                } else {
                    format!("{} — Select dataset", dataset.display_name())
                };
                if welcome_action(ui, icon::DATABASE, &label) {
                    selected = Some(dataset.resource_id());
                }
            });
        }
        if let Some(index) = selected.and_then(|id| app.doc.dataset_index(id)) {
            let visible = (0..app.doc.datasets.len()).collect::<Vec<_>>();
            crate::ui::primary_sidebar::selection::select_dataset(
                app,
                ui,
                &visible,
                index,
                Default::default(),
            );
            app.session.view = PrimaryView::Data;
            app.session.primary_sidebar_visible = true;
            if app.doc.datasets[index].as_table().is_some() {
                app.session.ui.sheet_open = Some(index);
            }
        }
    }
}

pub(super) fn render(app: &mut PlotiumApp, ui: &mut Ui) -> egui::scroll_area::ScrollAreaOutput<()> {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(28.0);
            let width = ui.available_width().min(500.0);
            ui.vertical_centered(|ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(width, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.label(
                            crate::typography::large_title("Plotium")
                                .color(ui.visuals().strong_text_color()),
                        );
                        ui.label(
                            egui::RichText::new("Scientific data analysis and figure preparation")
                                .color(ui.visuals().text_color()),
                        );
                        ui.add_space(22.0);
                        if app.session.data_imports.is_pending() {
                            ui.add_space(16.0);
                            ui.horizontal(|ui| {
                                ui.spinner();
                                let batch =
                                    app.session.data_imports.batches().iter().find(|batch| {
                                        batch.phase != plotium_core::state::ImportPhase::Complete
                                    });
                                ui.label(batch.map_or_else(
                                    || "Preparing data…".into(),
                                    crate::ui::import_feedback::summary,
                                ));
                            });
                        }
                        if !app.doc.datasets.is_empty() || !app.doc.canvases.is_empty() {
                            workspace_contents(app, ui);
                        } else if !app.session.data_imports.is_pending() {
                            ui.label("Open data or a project to get started.");
                        }
                        welcome_start(app, ui);
                    },
                );
            });
        })
}

fn welcome_start(app: &mut PlotiumApp, ui: &mut Ui) {
    use egui_phosphor::regular as icon;

    ui.add_space(20.0);
    welcome_heading(ui, "Start");
    for (glyph, label, command) in [
        (icon::FILE, "Open data…", CommandId::OpenFile),
        (icon::FOLDER, "Open data folder…", CommandId::OpenFolder),
        (icon::FOLDER_OPEN, "Open project…", CommandId::OpenProject),
        (icon::TABLE, "Import table…", CommandId::ImportTable),
        (icon::FILE_PLUS, "New empty data table", CommandId::NewTable),
    ] {
        let enabled = commands::describe(app, command).enabled;
        if ui
            .add_enabled_ui(enabled, |ui| welcome_action(ui, glyph, label))
            .inner
        {
            commands::execute_without_clipboard(command, app, ui.ctx());
        }
    }

    welcome_recent(app, ui);

    ui.add_space(20.0);
    welcome_heading(ui, "Tip");
    ui.label(
        egui::RichText::new("Drop data anywhere in the workspace to open it.")
            .color(ui.visuals().weak_text_color()),
    );
}

/// The most recent openable paths, straight back into work. Icons come from
/// the path shape alone (no filesystem probing in the paint loop); a missing
/// file surfaces through the loader's normal failure report on click.
fn welcome_recent(app: &mut PlotiumApp, ui: &mut Ui) {
    use egui_phosphor::regular as icon;
    const WELCOME_RECENT_LIMIT: usize = 5;

    if app.session.recent_files.is_empty() {
        return;
    }
    ui.add_space(20.0);
    welcome_heading(ui, "Recent");
    let mut open = None;
    for (index, path) in app
        .session
        .recent_files
        .iter()
        .take(WELCOME_RECENT_LIMIT)
        .enumerate()
    {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let glyph = match path.extension() {
            Some(extension) if extension.eq_ignore_ascii_case("plotium") => icon::FOLDER_OPEN,
            Some(_) => icon::FILE,
            // Bruker acquisitions (`fid`/`ser`) are extensionless *files*;
            // other extensionless paths are folder gestures.
            None if path.file_name().is_some_and(|name| {
                name.eq_ignore_ascii_case("fid") || name.eq_ignore_ascii_case("ser")
            }) =>
            {
                icon::FILE
            }
            None => icon::FOLDER,
        };
        let clicked = ui
            .add(
                egui::Button::new(
                    egui::RichText::new(format!("{glyph}  {name}"))
                        .color(ui.visuals().hyperlink_color),
                )
                .frame(false),
            )
            .on_hover_text(path.display().to_string())
            .clicked();
        if clicked {
            open = Some(index);
        }
    }
    if let Some(index) = open {
        commands::execute_without_clipboard(CommandId::OpenRecent(index), app, ui.ctx());
    }
}

fn welcome_heading(ui: &mut Ui, text: &str) {
    ui.label(crate::typography::title_3(text).color(ui.visuals().strong_text_color()));
    ui.add_space(4.0);
}

fn welcome_action(ui: &mut Ui, glyph: &str, label: &str) -> bool {
    ui.add(
        egui::Button::new(
            egui::RichText::new(format!("{glyph}  {label}")).color(ui.visuals().hyperlink_color),
        )
        .frame(false),
    )
    .clicked()
}

#[cfg(test)]
#[path = "workspace_tests.rs"]
mod tests;
