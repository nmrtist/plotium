//! The 3D peak table shares the active spectrum and its persistent peak records.
use super::commands::{self, CommandId};
use egui::{DragValue, Ui};
use plotium_core::state::{Peak3DId, Peak3DMark, PlotiumApp};

fn target(app: &PlotiumApp) -> Option<(usize, Peak3DId)> {
    let di = app.active_dataset()?;
    let (dataset, peak) = app.session.ui.selected_peak_3d?;
    (app.doc.datasets[di].resource_id() == dataset).then_some((di, peak))
}
pub(super) fn gate(app: &PlotiumApp, id: CommandId) -> Result<(), &'static str> {
    let di = app
        .active_dataset()
        .ok_or("Select a 3D spectrum before editing peaks.")?;
    let volume = app.doc.datasets[di]
        .as_nmr2d()
        .and_then(|n| n.volume.as_ref())
        .ok_or("Select a 3D spectrum before editing peaks.")?;
    if id == CommandId::Peak3DExport {
        return if volume.peaks.marks.is_empty() {
            Err("Add a peak before exporting the peak table.")
        } else {
            Ok(())
        };
    }
    if matches!(id, CommandId::Peak3DPrevious | CommandId::Peak3DNext) {
        return if volume.peaks.marks.is_empty() {
            Err("Add a peak before inspecting the peak list.")
        } else {
            Ok(())
        };
    }
    let (_, peak) = target(app).ok_or("Select a peak in the spectrum or peak table first.")?;
    if volume.peaks.marks.iter().any(|p| p.id == peak) {
        Ok(())
    } else {
        Err("Select an existing peak first.")
    }
}
pub(super) fn command(app: &mut PlotiumApp, _ctx: &egui::Context, id: CommandId) {
    if let Err(error) = gate(app, id) {
        app.session.status = error.into();
        return;
    }
    let di = app.active_dataset().expect("gated dataset");
    let dataset = app.doc.datasets[di].resource_id();
    let marks = app.doc.datasets[di]
        .as_nmr2d()
        .unwrap()
        .volume
        .as_ref()
        .unwrap()
        .peaks
        .marks
        .clone();
    if id == CommandId::Peak3DExport {
        let value = match app.peak_table_3d(dataset) {
            Ok(value) => value,
            Err(error) => {
                app.session.status = error;
                return;
            }
        };
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name("peaks3d.json")
            .save_file()
        {
            let result = serde_json::to_vec_pretty(&value)
                .map_err(|e| e.to_string())
                .and_then(|bytes| std::fs::write(path, bytes).map_err(|e| e.to_string()));
            if let Err(e) = result {
                app.session.status = e;
            }
        }
        return;
    }
    let selected = target(app).map(|(_, id)| id);
    if matches!(id, CommandId::Peak3DPrevious | CommandId::Peak3DNext) {
        let current = selected.and_then(|id| marks.iter().position(|m| m.id == id));
        let next = match current {
            None => 0,
            Some(i) if id == CommandId::Peak3DPrevious => i.saturating_sub(1),
            Some(i) => (i + 1).min(marks.len() - 1),
        };
        app.session.ui.selected_peak_3d = Some((dataset, marks[next].id));
        if let Err(e) = app.locate_peak_3d(dataset, marks[next].id) {
            app.session.status = e;
        }
        return;
    }
    let peak = selected.unwrap();
    let result = match id {
        CommandId::Peak3DDelete => app.delete_peak_3d(dataset, peak),
        CommandId::Peak3DLocate => app.locate_peak_3d(dataset, peak),
        _ => Ok(()),
    };
    if let Err(e) = result {
        app.session.status = e;
    }
}
fn button(app: &mut PlotiumApp, ui: &mut Ui, id: CommandId, label: &str) {
    let description = commands::describe(app, id);
    if ui
        .add_enabled(
            description.enabled,
            egui::Button::new(label).selected(description.checked.unwrap_or(false)),
        )
        .on_disabled_hover_text(description.disabled_reason.unwrap_or(""))
        .clicked()
    {
        commands::execute_without_clipboard(id, app, ui.ctx());
    }
}
pub(super) fn panel(app: &mut PlotiumApp, di: usize, ui: &mut Ui) {
    let dataset = app.doc.datasets[di].resource_id();
    let marks = app.doc.datasets[di]
        .as_nmr2d()
        .unwrap()
        .volume
        .as_ref()
        .unwrap()
        .peaks
        .marks
        .clone();
    let editing = target(app).map(|(_, id)| id);
    ui.small("Click the spectrum to place a peak. Drag a selected mark to move it in this plane.");
    ui.horizontal(|ui| {
        button(
            app,
            ui,
            CommandId::Tool(plotium_core::state::Tool::Peaks),
            "Pick peaks",
        );
        button(app, ui, CommandId::Peak3DPrevious, "Previous");
        button(app, ui, CommandId::Peak3DNext, "Next");
    });
    button(app, ui, CommandId::Peak3DExport, "Export peak table…");
    if marks.is_empty() {
        ui.label("No peaks yet — click a signal in the spectrum to add one.");
        return;
    }
    let axes = app.doc.datasets[di]
        .as_nmr2d()
        .unwrap()
        .volume
        .as_ref()
        .unwrap()
        .processed
        .axes()
        .iter()
        .enumerate()
        .map(|(axis, a)| format!("F{} {} (ppm)", axis + 1, a.nucleus().unwrap_or("?")))
        .collect::<Vec<_>>();
    ui.label(format!(
        "ID   {} / {} / {}   Signed height",
        axes[0], axes[1], axes[2]
    ));
    for mark in &marks {
        let selected = app.session.ui.selected_peak_3d == Some((dataset, mark.id));
        let summary = app
            .doc
            .protein
            .assignment(dataset, mark.id)
            .map_or_else(String::new, |a| {
                let status = a
                    .confirmed
                    .and_then(|id| a.candidates.iter().find(|c| c.id == id))
                    .map_or_else(
                        || format!("{} candidate(s)", a.candidates.len()),
                        |c| {
                            format!(
                                "Confirmed · {}{}",
                                if c.targets.iter().all(Option::is_some) {
                                    "complete"
                                } else {
                                    "partial"
                                },
                                if c.targets.iter().flatten().any(|t| t.members.len() > 1) {
                                    " · group"
                                } else {
                                    ""
                                }
                            )
                        },
                    );
                format!(" · {status}")
            });
        let response = ui.selectable_label(
            selected,
            format!(
                "{}  {:.3} / {:.3} / {:.3}  {:+.3e}{summary}",
                mark.id.0, mark.ppm[0], mark.ppm[1], mark.ppm[2], mark.intensity
            ),
        );
        if response.clicked() {
            app.session.ui.selected_peak_3d = Some((dataset, mark.id));
            commands::execute_without_clipboard(CommandId::Peak3DLocate, app, ui.ctx());
        }
    }
    if let Some(id) = editing
        && let Some(mark) = marks.iter().find(|m| m.id == id)
    {
        ui.separator();
        edit_fields(app, dataset, mark, &axes, ui);
        ui.horizontal(|ui| {
            button(app, ui, CommandId::Peak3DLocate, "Locate");
            let label = if app.doc.protein.assignment(dataset, id).is_some() {
                "Delete peak and assignments"
            } else {
                "Delete"
            };
            button(app, ui, CommandId::Peak3DDelete, label);
            button(app, ui, CommandId::ProteinAssignments, "Assignments");
        });
    }
}

#[derive(Clone)]
struct EditDraft {
    baseline: Peak3DMark,
    ppm: [f64; 3],
    note: String,
}

fn edit_fields(
    app: &mut PlotiumApp,
    dataset: plotium_core::state::DatasetId,
    mark: &Peak3DMark,
    axes: &[String],
    ui: &mut Ui,
) {
    let key = egui::Id::new(("peak3d_edit", dataset, mark.id));
    let mut draft = ui
        .ctx()
        .data(|d| d.get_temp::<EditDraft>(key))
        .filter(|d| d.baseline.ppm == mark.ppm && d.baseline.note == mark.note)
        .unwrap_or_else(|| EditDraft {
            baseline: mark.clone(),
            ppm: mark.ppm,
            note: mark.note.clone(),
        });
    let mut finished = false;
    ui.push_id((dataset, mark.id), |ui| {
        for (axis, value) in draft.ppm.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(&axes[axis]);
                let response = ui.add(DragValue::new(value).speed(0.001));
                finished |= response.drag_stopped() || response.lost_focus();
            });
        }
        ui.label("Note");
        finished |= ui.text_edit_singleline(&mut draft.note).lost_focus();
    });
    // Commit at gesture end, so typing and dragging each have one undo step.
    if finished {
        ui.ctx().data_mut(|d| d.remove::<EditDraft>(key));
        if (draft.ppm != mark.ppm || draft.note != mark.note)
            && let Err(error) = app.edit_peak_3d(dataset, mark.id, draft.ppm, draft.note)
        {
            app.session.status = error;
        }
    } else {
        ui.ctx().data_mut(|d| d.insert_temp(key, draft));
    }
}

#[cfg(test)]
#[path = "peaks3d_tests.rs"]
mod tests;
