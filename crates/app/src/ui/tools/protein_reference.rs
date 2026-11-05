use super::Panel;
use plotium_core::state::{DatasetId, PlotiumApp};
pub(super) fn render(
    app: &mut PlotiumApp,
    ui: &mut egui::Ui,
    panel: &mut Panel,
    active: Option<DatasetId>,
) {
    ui.collapsing("Chemical-shift reference", |ui| {
        ui.label("Absolute offsets from acquisition reference: corrected ppm = original ppm + offset.");
        if let Some(id) = active {
            if ui.button("Edit reference").clicked() {
                match plotium_core::protein::volume(&app.doc, id) {
                    Ok(v) => {
                        panel.reference_baseline = Some(v.peaks.reference.clone());
                        panel.reference_draft = Some((id, v.peaks.reference.clone()));
                    },
                    Err(e) => panel.feedback = e,
                }
            }
            for &other in app.doc.protein.context.experiments.keys() {
                if other != id {
                    let result = app.reference_differences(id, other);
                    if let Ok(rows) = &result {
                        if rows.is_empty() { ui.label("No matching confirmed single-atom observations; not evaluated."); }
                        for row in rows {
                            ui.label(format!("Peak {} F{} vs {} F{}: delta {:+.5} ppm", row.left_peak.0, row.left_axis + 1, row.right_peak.0, row.right_axis + 1, row.delta_ppm));
                        }
                    }
                    ui.label(format!("{}: {}", other, result.err().unwrap_or_else(|| "References manually confirmed; inspect matching nuclei and axis roles before comparing peaks".into())));
                }
            }
        }
        let mut close = false;
        if let Some((id, draft)) = &mut panel.reference_draft {
            let name = app.doc.dataset_index(*id).and_then(|i| app.doc.datasets[i].as_nmr2d()).map(|n| n.name.as_deref().unwrap_or("Unnamed spectrum")).unwrap_or("Deleted spectrum");
            ui.label(format!("Reference draft target: {name} ({id})"));
            if active != Some(*id) {
                ui.colored_label(ui.visuals().warn_fg_color, "The active spectrum differs from this draft. Saving edits the draft target shown above.");
            }

            for (axis, value) in draft.offsets_ppm.iter_mut().enumerate() {
                ui.horizontal(|ui| { ui.label(format!("F{} offset (ppm)", axis + 1)); ui.add(egui::DragValue::new(value).speed(0.001)); });
            }
            ui.label("Reference source / standard / comparison evidence");
            ui.text_edit_singleline(&mut draft.source);
            ui.checkbox(&mut draft.confirmed, "Manually reviewed reference (not scientific certification)");
            if ui.button("Save reference").clicked() {
                let current = plotium_core::protein::volume(&app.doc, *id).map(|v| &v.peaks.reference);
                let result = match current {
                    Ok(value) if Some(value) == panel.reference_baseline.as_ref() => app.set_shift_reference(*id, draft.clone()),
                    _ => Err("Reference changed or spectrum was removed. Discard and reopen the reference draft.".into()),
                };
                match result {
                    Ok(()) => { close = true; panel.feedback = "Reference saved".into(); }
                    Err(e) => panel.feedback = e,
                }
            }
            close |= ui.button("Discard reference draft").clicked();
        }
        if close { panel.reference_draft = None; }
    });
}

#[cfg(test)]
#[path = "protein_reference_tests.rs"]
mod tests;
