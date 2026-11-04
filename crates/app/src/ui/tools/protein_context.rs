use super::*;

pub(super) fn spectrum_name(app: &PlotiumApp, dataset: DatasetId) -> String {
    app.doc
        .dataset_by_id(dataset)
        .map_or_else(|| dataset.to_string(), |d| d.display_name())
}

pub(super) fn assignment_context(app: &PlotiumApp, ui: &mut Ui, dataset: DatasetId) {
    ui.label(format!(
        "Assignment spectrum: {}",
        spectrum_name(app, dataset)
    ))
    .on_hover_text(dataset.to_string());
    if let Some(m) = app
        .doc
        .protein
        .bindings
        .get(&dataset)
        .and_then(|id| app.doc.protein.molecule(*id).ok())
    {
        ui.label(format!("Assignment molecule: {}", m.name));
    }
}

pub(super) fn clear_review(app: &mut PlotiumApp, ui: &mut Ui, panel: &mut Panel) {
    let Some((dataset, revision)) = panel.clear_spectrum else {
        return;
    };
    ui.separator();
    ui.label(format!(
        "Clear assignments from {}?",
        spectrum_name(app, dataset)
    ))
    .on_hover_text(dataset.to_string());
    let rows: Vec<_> = app
        .doc
        .protein
        .assignments
        .iter()
        .filter(|a| a.dataset == dataset)
        .collect();
    let candidates: usize = rows.iter().map(|a| a.candidates.len()).sum();
    let confirmed = rows.iter().filter(|a| a.confirmed.is_some()).count();
    ui.label(format!("{} peaks · {candidates} candidates · {confirmed} confirmed. Peak observations will remain. Undo restores the assignments.", rows.len()));
    let current = revision == app.doc.automation_revision;
    if !current {
        ui.label("The project changed. Review the updated counts before clearing.");
        if ui.button("Review updated counts").clicked() {
            panel.clear_spectrum = Some((dataset, app.doc.automation_revision));
        }
    }
    if ui
        .add_enabled(
            current && !rows.is_empty(),
            egui::Button::new("Confirm clear spectrum assignments"),
        )
        .clicked()
    {
        panel.feedback = app
            .edit_protein(Edit::ClearSpectrumAssignments { dataset })
            .err()
            .unwrap_or_default();
        panel.clear_spectrum = None;
    }
    if ui.button("Cancel clearing").clicked() {
        panel.clear_spectrum = None;
    }
}
