use egui::{RichText, Ui};
use egui_phosphor::regular as icon;
use plotium_core::state::{PlotiumApp, TaskDockTab, Tool};

/// Labelled switcher shared by every page in the one top-right task dock.
/// Hidden task pages keep their state; selecting a tab also restores the
/// dataset that owns it so controls never edit a dataset other than the canvas.
pub(in crate::ui::tools) fn tab_bar(
    app: &mut PlotiumApp,
    current: TaskDockTab,
    ui: &mut Ui,
) -> bool {
    let tabs = [
        (TaskDockTab::Protein, icon::SELECTION, "Protein", None),
        (
            TaskDockTab::Dosy,
            icon::CHART_LINE,
            "DOSY",
            app.session
                .ui
                .dosy_task_dataset
                .and_then(|id| app.doc.dataset_index(id)),
        ),
        (
            TaskDockTab::Processing,
            icon::FLOW_ARROW,
            "Process",
            app.session
                .ui
                .processing_task_dataset
                .and_then(|id| app.doc.dataset_index(id)),
        ),
        (
            TaskDockTab::Craft,
            icon::WAVEFORM,
            "CRAFT",
            app.session
                .ui
                .craft_task_dataset
                .and_then(|id| app.doc.dataset_index(id)),
        ),
        (
            TaskDockTab::Regions,
            icon::SELECTION,
            "Regions",
            app.session
                .ui
                .region_task_dataset
                .and_then(|id| app.doc.dataset_index(id)),
        ),
        (
            TaskDockTab::CurveFit,
            icon::CHART_LINE_UP,
            "Fit",
            app.session.ui.curve_fit_task_dataset,
        ),
        (
            TaskDockTab::Statistics,
            icon::FUNCTION,
            "Stats",
            app.session.ui.stat_task_dataset,
        ),
    ];
    let open = tabs
        .into_iter()
        .filter(|(tab, _, _, dataset)| {
            dataset.is_some() || (*tab == TaskDockTab::Protein && app.session.ui.protein_open)
        })
        .collect::<Vec<_>>();
    if open.len() < 2 {
        return false;
    }
    let mut action = None;
    ui.scope(|ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for (tab, glyph, label, dataset) in &open {
                let selected = *tab == current;
                let (fill, stroke) = if selected {
                    (
                        ui.visuals().selection.bg_fill,
                        ui.visuals().selection.stroke,
                    )
                } else {
                    (
                        ui.visuals().widgets.inactive.weak_bg_fill,
                        ui.visuals().widgets.inactive.bg_stroke,
                    )
                };
                egui::Frame::NONE
                    .fill(fill)
                    .stroke(stroke)
                    .corner_radius(ui.visuals().widgets.inactive.corner_radius)
                    .inner_margin(egui::Margin::symmetric(6, 2))
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        ui.horizontal(|ui| {
                            if ui
                                .add(
                                    egui::Button::selectable(
                                        selected,
                                        RichText::new(format!("{glyph} {label}")).small(),
                                    )
                                    .frame(false),
                                )
                                .clicked()
                            {
                                action = Some((false, *tab, *dataset));
                            }
                            if ui
                                .add(egui::Button::new(RichText::new(icon::X).small()).frame(false))
                                .on_hover_text(format!("Close {label}"))
                                .clicked()
                            {
                                action = Some((true, *tab, *dataset));
                            }
                        });
                    });
            }
        });
    });
    if let Some((close, tab, dataset)) = action {
        if close {
            app.session.ui.close_task_tab(tab);
            return true;
        }
        if app.session.ui.task_dock_active == Some(TaskDockTab::Regions)
            && tab != TaskDockTab::Regions
            && app.session.tool == Tool::Regions
        {
            app.set_tool(Tool::BrowseZoom);
        }
        app.session.ui.open_task_tab(tab);
        if let Some(dataset) = dataset {
            app.set_active_dataset(Some(dataset));
        }
    }
    true
}
