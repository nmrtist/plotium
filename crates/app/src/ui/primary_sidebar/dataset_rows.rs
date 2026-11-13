use egui::Ui;
use plotium_core::state::PlotiumApp;

use crate::ui::{
    commands::{self, CommandId},
    data_navigation::PlotIndex,
};

pub(super) fn filters(app: &mut PlotiumApp, ui: &mut Ui) {
    if app.session.ui.data_browser_batch.is_some_and(|id| {
        !app.session
            .data_imports
            .batches()
            .iter()
            .any(|b| b.id == id)
    }) {
        app.session.ui.data_browser_batch = None;
    }
    ui.horizontal_wrapped(|ui| {
        if let Some(batch) = app.session.ui.data_browser_batch
            && ui
                .small_button(format!("Import {batch} ×"))
                .on_hover_text("Remove import filter")
                .clicked()
        {
            app.session.ui.data_browser_batch = None;
        }
        if let Some(tag) = &app.session.ui.data_browser_tag
            && ui
                .small_button(format!("{tag} ×"))
                .on_hover_text("Remove tag filter")
                .clicked()
        {
            app.session.ui.data_browser_tag = None;
        }
        button(app, ui, CommandId::EditDatasetMetadata(None));
    });
}

pub(super) fn details(app: &PlotiumApp, di: usize) -> String {
    let dataset = &app.doc.datasets[di];
    let mut text = format!(
        "{} · {}
Source: {}",
        dataset.dimension_label().unwrap_or(dataset.kind_label()),
        dataset.summary(),
        dataset.source_identifier()
    );
    if let Some(experiment) = dataset.acquisition_identity().acquisition.as_deref() {
        text.push_str(&format!(
            "
Imported experiment: {experiment}"
        ));
    }
    if let Some(lineage) = super::data_browser::sources_tooltip(app, di) {
        text.push_str(&format!(
            "
{lineage}"
        ));
    }
    text.push_str(
        "
Double-click to open; right-click to rename",
    );
    text
}

pub(super) fn summary(
    app: &mut PlotiumApp,
    ui: &mut Ui,
    di: usize,
    depth: usize,
    index: &PlotIndex,
) {
    let dataset = &app.doc.datasets[di];
    let id = dataset.resource_id();
    let occurrences = index.occurrences(id);
    let summary = dataset.experiment_label().map_or_else(
        || dataset.compact_category().to_owned(),
        |name| format!("{} · {name}", dataset.compact_category()),
    );
    let tags = dataset.organization().tags().to_vec();
    ui.horizontal_wrapped(|ui| {
        ui.add_space(depth as f32 * 12.0 + 18.0);
        ui.weak(summary);
        for tag in tags {
            if ui
                .small_button(&tag)
                .on_hover_text("Filter by this tag")
                .clicked()
            {
                app.session.ui.data_browser_tag = Some(tag);
            }
        }
        if occurrences.is_empty() {
            ui.weak("Not plotted");
        } else {
            button(app, ui, CommandId::LocateDatasetPlot(Some(id)));
        }
    });
}

pub(super) fn button(app: &mut PlotiumApp, ui: &mut Ui, command: CommandId) -> bool {
    let descriptor = commands::describe(app, command);
    if ui
        .add_enabled(
            descriptor.enabled,
            egui::Button::new(descriptor.label).small().wrap(),
        )
        .on_disabled_hover_text(descriptor.disabled_reason.unwrap_or_default())
        .clicked()
    {
        commands::execute_without_clipboard(command, app, ui.ctx());
        true
    } else {
        false
    }
}
