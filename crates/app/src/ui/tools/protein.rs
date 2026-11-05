use super::task_card;
use egui::{ComboBox, DragValue, Ui};
use plotium_core::protein::{
    self, AssignmentEdit, AtomTarget, Candidate, ChainLabel, Edit, GroupMeaning, Molecule,
    MoleculeId, ResidueId, Targets,
};
use plotium_core::state::{DatasetId, Peak3DId, PlotiumApp, TaskDockTab};

#[derive(Clone, Default)]
struct Panel {
    molecule: Option<MoleculeId>,
    residue: Option<ResidueId>,
    import: Option<Import>,
    draft: Option<Draft>,
    confirmed_only: bool,
    feedback: String,
    clear_spectrum: Option<(DatasetId, u64)>,
    reference_baseline: Option<plotium_core::protein_reference::ShiftReference>,
    reference_draft: Option<(DatasetId, plotium_core::protein_reference::ShiftReference)>,
    context_draft: Option<scientific_context_editor::ContextDraft>,
}

#[derive(Clone)]
struct Import {
    fasta: String,
    preview: Molecule,
}

#[derive(Clone)]
struct Draft {
    dataset: DatasetId,
    peak: Peak3DId,
    baseline: Option<Candidate>,
    residues: [Option<ResidueId>; 3],
    targets: Targets,
    note: String,
}

fn key(app: &PlotiumApp) -> egui::Id {
    egui::Id::new(("protein_panel", app.session.project_context.to_string()))
}

pub(in crate::ui) fn open(app: &mut PlotiumApp) {
    app.session.ui.protein_open = true;
    app.session.ui.open_task_tab(TaskDockTab::Protein);
}

pub(in crate::ui) fn import(app: &mut PlotiumApp, ctx: &egui::Context) {
    open(app);
    let key = key(app);
    let mut panel = ctx.data(|d| d.get_temp::<Panel>(key)).unwrap_or_default();
    if panel.import.is_some() {
        app.session.status = "Finish or discard the current sequence preview first.".into();
        return;
    }
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Protein FASTA", &["fasta", "fa", "faa", "txt"])
        .pick_file()
    else {
        return;
    };
    let source = path
        .file_name()
        .map_or_else(|| "Protein".into(), |s| s.to_string_lossy().into_owned());
    let result = std::fs::read_to_string(path)
        .map_err(|e| e.to_string())
        .and_then(|fasta| prepare(&source, fasta));
    match result {
        Ok(import) => panel.import = Some(import),
        Err(e) => panel.feedback = e,
    }
    ctx.data_mut(|d| d.insert_temp(key, panel));
}

fn prepare(source: &str, fasta: String) -> Result<Import, String> {
    Ok(Import {
        preview: protein::import_preview(source, &fasta)?,
        fasta,
    })
}

pub(in crate::ui) fn render(app: &mut PlotiumApp, host: &mut Ui) {
    let tab = TaskDockTab::Protein;
    if !task_card::is_active(app, tab) {
        return;
    }
    let geometry = task_card::geometry(app, host, tab, 340.0, false);
    let area = egui::Id::new("protein_task_card");
    let key = key(app);
    let mut panel = host
        .ctx()
        .data(|d| d.get_temp::<Panel>(key))
        .unwrap_or_default();
    task_card::area(host, area, geometry.pos).show(host.ctx(), |ui| {
        ui.set_width(geometry.width);
        crate::ui::card_frame(host.visuals().dark_mode, egui::Margin::ZERO).show(ui, |ui| {
            let mut close = false;
            task_card::header(ui, area, "Protein assignments", None::<String>, |ui| {
                close = crate::ui::panel_chrome::close(ui, "Close Protein assignments").clicked();
            });
            if close {
                app.session.ui.close_task_tab(tab);
            }
            if task_card::tab_bar(app, tab, ui) {
                ui.separator();
            }
            task_card::sized_body(ui, geometry.body_height, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("protein_body_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| body(app, ui, &mut panel));
            });
        });
        task_card::resize_handles(app, ui, area, tab, geometry.width, geometry.body_height);
    });
    host.ctx().data_mut(|d| d.insert_temp(key, panel));
}

fn body(app: &mut PlotiumApp, ui: &mut Ui, panel: &mut Panel) {
    if !panel.feedback.is_empty() {
        ui.label(&panel.feedback);
    }
    if let Some(import) = &mut panel.import {
        ui.heading("Sequence import preview");
        ui.label(&import.preview.source);
        ui.text_edit_singleline(&mut import.preview.name);
        for chain in &mut import.preview.chains {
            ui.label(format!(
                "{} · {} residues",
                chain.title,
                chain.residues.len()
            ));
            label_fields(ui, &mut chain.name, &mut chain.start);
        }
        if ui.button("Import sequence").clicked() {
            let result = app.edit_protein(Edit::Import {
                source: import.preview.source.clone(),
                fasta: import.fasta.clone(),
                name: Some(import.preview.name.clone()),
                chains: Some(
                    import
                        .preview
                        .chains
                        .iter()
                        .map(|c| ChainLabel {
                            name: c.name.clone(),
                            start: c.start,
                        })
                        .collect(),
                ),
            });
            match result {
                Ok(()) => {
                    panel.molecule = app.doc.protein.molecules.last().map(|m| m.id);
                    panel.import = None;
                    panel.feedback.clear();
                }
                Err(e) => panel.feedback = e,
            }
        }
        if ui.button("Discard import preview").clicked() {
            panel.import = None;
        }
        return;
    }
    if app.doc.protein.molecules.is_empty() {
        ui.label("No protein sequences — choose Import protein sequence on the Data tab.");
    }
    let active = app
        .active_dataset()
        .and_then(|i| app.doc.datasets.get(i))
        .map(|d| d.resource_id())
        .filter(|d| protein::volume(&app.doc, *d).is_ok());
    let selected_peak = app.session.ui.selected_peak_3d.filter(|(d, p)| {
        Some(*d) == active
            && protein::volume(&app.doc, *d).is_ok_and(|v| v.peaks.marks.iter().any(|m| m.id == *p))
    });
    let bound = active.and_then(|d| app.doc.protein.bindings.get(&d).copied());
    let selected = panel
        .molecule
        .filter(|id| app.doc.protein.molecule(*id).is_ok())
        .or(bound)
        .or_else(|| app.doc.protein.molecules.first().map(|m| m.id));
    panel.molecule = selected;
    ComboBox::from_id_salt("protein_molecule")
        .selected_text(
            selected
                .and_then(|id| app.doc.protein.molecule(id).ok())
                .map_or("Select molecule", |m| &m.name),
        )
        .show_ui(ui, |ui| {
            for m in &app.doc.protein.molecules {
                if ui
                    .selectable_value(&mut panel.molecule, Some(m.id), &m.name)
                    .clicked()
                {
                    panel.residue = None;
                }
            }
        });
    if let Some(m) = panel
        .molecule
        .and_then(|id| app.doc.protein.molecule(id).ok())
        .cloned()
    {
        if let Some(d) = active {
            let label = bound
                .and_then(|id| app.doc.protein.molecule(id).ok())
                .map_or("Unbound", |m| &m.name);
            ui.label(format!("Active spectrum: {}", spectrum_name(app, d)));
            ui.label(format!("Bound molecule: {label}"));
            if bound.is_some_and(|id| id != m.id) {
                ui.label("The sequence browser shows a different molecule from the active spectrum. Assignments use the bound molecule.");
            }
            if bound != Some(m.id) && ui.button("Bind active spectrum to this molecule").clicked() {
                panel.feedback = app
                    .edit_protein(Edit::Bind {
                        dataset: d,
                        molecule: Some(m.id),
                    })
                    .err()
                    .unwrap_or_default();
            }
            if app.doc.protein.assignments.iter().any(|a| a.dataset == d)
                && ui
                    .button("Clear all assignments on active spectrum")
                    .clicked()
            {
                panel.clear_spectrum = Some((d, app.doc.automation_revision));
            }
            if bound.is_some() && ui.button("Unbind active spectrum").clicked() {
                panel.feedback = app
                    .edit_protein(Edit::Bind {
                        dataset: d,
                        molecule: None,
                    })
                    .err()
                    .unwrap_or_default();
            }
        } else {
            ui.label("Select a 3D spectrum to bind it.");
        }
        ui.collapsing("Molecule details", |ui| details(app, ui, panel, &m));
        ui.collapsing("Sequence and associated peaks", |ui| {
            sequence(app, ui, panel, &m)
        });
    }
    scientific_context(app, ui, panel, active);
    reference_editor::render(app, ui, panel, active);
    clear_review(app, ui, panel);
    ui.separator();
    if panel.draft.is_some() {
        draft_editor(app, ui, panel);
    } else if let Some((d, p)) = selected_peak {
        if bound.is_some() {
            candidates(app, ui, panel, d, p);
        } else {
            ui.label("Bind the spectrum to enter assignments.");
        }
    } else {
        ui.label("Select a peak in the 3D peak table to enter assignments.");
    }
}

fn label_fields(ui: &mut Ui, name: &mut String, start: &mut i32) {
    ui.horizontal(|ui| {
        ui.label("Chain");
        ui.text_edit_singleline(name);
        ui.label("Start");
        ui.add(DragValue::new(start));
    });
}

fn details(app: &mut PlotiumApp, ui: &mut Ui, panel: &mut Panel, m: &Molecule) {
    ui.push_id(m.id, |ui| {
        let key = ui.id().with("name");
        let mut name = ui
            .ctx()
            .data(|d| d.get_temp::<String>(key))
            .unwrap_or_else(|| m.name.clone());
        ui.text_edit_singleline(&mut name);
        if ui.button("Save molecule name").clicked() {
            panel.feedback = app
                .edit_protein(Edit::Rename {
                    molecule: m.id,
                    name: name.clone(),
                })
                .err()
                .unwrap_or_default();
        }
        ui.ctx().data_mut(|d| {
            if name == m.name {
                d.remove::<String>(key);
            } else {
                d.insert_temp(key, name);
            }
        });
        for c in &m.chains {
            ui.push_id(c.id, |ui| {
                let key = ui.id().with("label");
                let mut label = ui
                    .ctx()
                    .data(|d| d.get_temp::<ChainLabel>(key))
                    .unwrap_or_else(|| ChainLabel {
                        name: c.name.clone(),
                        start: c.start,
                    });
                label_fields(ui, &mut label.name, &mut label.start);
                if ui.button("Save chain label").clicked() {
                    panel.feedback = app
                        .edit_protein(Edit::LabelChain {
                            molecule: m.id,
                            chain: c.id,
                            label: label.clone(),
                        })
                        .err()
                        .unwrap_or_default();
                }
                ui.ctx().data_mut(|d| {
                    if label.name == c.name && label.start == c.start {
                        d.remove::<ChainLabel>(key);
                    } else {
                        d.insert_temp(key, label);
                    }
                });
            });
        }
        for (&d, _) in app
            .doc
            .protein
            .bindings
            .iter()
            .filter(|(_, id)| **id == m.id)
        {
            if let Some(dataset) = app.doc.dataset_by_id(d) {
                ui.label(format!("Bound: {}", dataset.display_name()));
            }
        }
        ui.checkbox(
            &mut panel.confirmed_only,
            "Export only confirmed assignments",
        );
        if ui.button("Export JSON…").clicked()
            && let Some(path) = rfd::FileDialog::new()
                .set_file_name("protein-assignments.json")
                .save_file()
        {
            panel.feedback = app
                .export_protein(m.id, panel.confirmed_only)
                .and_then(|value| serde_json::to_vec_pretty(&value).map_err(|e| e.to_string()))
                .and_then(|bytes| std::fs::write(path, bytes).map_err(|e| e.to_string()))
                .err()
                .unwrap_or_default();
        }
        if ui.button("Delete molecule").clicked() {
            panel.feedback = app
                .edit_protein(Edit::Delete { molecule: m.id })
                .err()
                .unwrap_or_default();
        }
    });
}

fn sequence(app: &mut PlotiumApp, ui: &mut Ui, panel: &mut Panel, m: &Molecule) {
    for chain in &m.chains {
        ui.collapsing(format!("Chain {} · {}", chain.name, chain.title), |ui| {
            ui.horizontal_wrapped(|ui| {
                for (i, r) in chain.residues.iter().enumerate() {
                    ui.selectable_value(
                        &mut panel.residue,
                        Some(r.id),
                        format!("{}{}", r.code, i64::from(chain.start) + i as i64),
                    );
                }
            });
        });
    }
    let Some(residue) = panel.residue else {
        ui.label("Select a residue to find associated peaks.");
        return;
    };
    let rows: Vec<_> = app
        .doc
        .protein
        .assignments
        .iter()
        .filter(|a| app.doc.protein.bindings.get(&a.dataset) == Some(&m.id))
        .filter_map(|a| {
            let matches: Vec<_> = a
                .candidates
                .iter()
                .filter(|c| c.targets.iter().flatten().any(|t| t.residue == residue))
                .collect();
            (!matches.is_empty()).then(|| {
                (
                    a.dataset,
                    a.peak,
                    matches.iter().any(|c| Some(c.id) == a.confirmed),
                )
            })
        })
        .collect();
    if rows.is_empty() {
        ui.label("No associated peaks for this residue.");
    }
    for (d, p, confirmed) in rows {
        let name = app
            .doc
            .dataset_by_id(d)
            .map_or_else(|| d.to_string(), |d| d.display_name());
        if ui
            .button(format!(
                "{name} · Peak {} · {}",
                p.0,
                if confirmed { "Confirmed" } else { "Candidate" }
            ))
            .clicked()
        {
            match app.locate_peak_3d(d, p) {
                Ok(()) => {
                    let owns_plot = |canvas: &plotium_core::state::CanvasDocument| {
                        canvas
                            .objects
                            .iter()
                            .any(|o| o.plot().is_some_and(|p| p.display_owner == Some(d)))
                    };
                    if !app
                        .session
                        .active_canvas
                        .and_then(|i| app.doc.canvases.get(i))
                        .is_some_and(owns_plot)
                        && let Some(i) = app.doc.canvases.iter().position(owns_plot)
                    {
                        app.activate_canvas(i);
                    }
                    app.set_active_dataset(app.doc.dataset_index(d));
                    app.session.ui.selected_peak_3d = Some((d, p));
                }
                Err(e) => panel.feedback = e,
            }
        }
    }
}

#[path = "protein_editor.rs"]
mod editor;
use editor::{candidates, draft_editor};

#[path = "protein_context.rs"]
mod context;
use context::{assignment_context, clear_review, spectrum_name};

#[cfg(test)]
#[path = "protein_tests.rs"]
mod tests;

#[path = "protein_scientific_context.rs"]
mod scientific_context_editor;
use scientific_context_editor::scientific_context;

#[path = "protein_reference.rs"]
mod reference_editor;
