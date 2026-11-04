use super::*;

pub(super) fn candidates(
    app: &mut PlotiumApp,
    ui: &mut Ui,
    panel: &mut Panel,
    d: DatasetId,
    p: Peak3DId,
) {
    ui.heading(format!("Peak {} assignments", p.0));
    assignment_context(app, ui, d);
    ui.small("Confirmed means manually accepted, not automatically validated.");
    if let Some(a) = app.doc.protein.assignment(d, p).cloned() {
        for (i, c) in a.candidates.iter().enumerate() {
            let complete = c.targets.iter().all(Option::is_some);
            let confirmed = a.confirmed == Some(c.id);
            ui.label(format!(
                "{} {} · {}",
                if confirmed { "Confirmed" } else { "Candidate" },
                i + 1,
                if complete { "Complete" } else { "Partial" }
            ));
            if let Some(m) = app
                .doc
                .protein
                .bindings
                .get(&d)
                .and_then(|id| app.doc.protein.molecule(*id).ok())
            {
                ui.small(
                    c.targets
                        .iter()
                        .map(|t| describe_target(m, t.as_ref()))
                        .collect::<Vec<_>>()
                        .join(" / "),
                );
            }
            if !c.note.is_empty() {
                ui.small(&c.note);
            }
            ui.horizontal_wrapped(|ui| {
                if ui.button("Edit").clicked() {
                    panel.draft = Some(Draft::new(d, p, Some(c.clone())));
                }
                let edit = if confirmed {
                    ui.button("Withdraw confirmation")
                        .clicked()
                        .then_some(AssignmentEdit::Withdraw)
                } else {
                    ui.button("Confirm")
                        .clicked()
                        .then_some(AssignmentEdit::Confirm { candidate: c.id })
                };
                let edit = if ui.button("Delete candidate").clicked() {
                    Some(AssignmentEdit::Delete { candidate: c.id })
                } else {
                    edit
                };
                if let Some(edit) = edit {
                    panel.feedback = app.edit_assignment(d, p, edit).err().unwrap_or_default();
                }
            });
        }
        if ui.button("Clear peak assignments").clicked() {
            panel.feedback = app
                .edit_assignment(d, p, AssignmentEdit::Clear)
                .err()
                .unwrap_or_default();
        }
    }
    if ui.button("New candidate").clicked() {
        panel.draft = Some(Draft::new(d, p, None));
    }
}

impl Draft {
    pub(super) fn new(dataset: DatasetId, peak: Peak3DId, baseline: Option<Candidate>) -> Self {
        let targets = baseline
            .as_ref()
            .map_or([None, None, None], |c| c.targets.clone());
        let residues = std::array::from_fn(|i| targets[i].as_ref().map(|t| t.residue));
        let note = baseline
            .as_ref()
            .map_or_else(String::new, |c| c.note.clone());
        Self {
            dataset,
            peak,
            baseline,
            residues,
            targets,
            note,
        }
    }
}

fn describe_target(m: &Molecule, target: Option<&AtomTarget>) -> String {
    let Some(t) = target else {
        return "Unassigned".into();
    };
    let atom = t.members.iter().cloned().collect::<Vec<_>>().join(", ");
    let atom = if t.members.len() > 1 {
        format!(
            "{} {{{atom}}}",
            match t.group_meaning {
                GroupMeaning::Unspecified => "Group meaning unspecified",
                GroupMeaning::OneOf => "One of",
                GroupMeaning::Shared => "Shared resonance",
            }
        )
    } else {
        atom
    };
    for c in &m.chains {
        if let Some((i, r)) = c
            .residues
            .iter()
            .enumerate()
            .find(|(_, r)| r.id == t.residue)
        {
            return format!(
                "{}:{}{} {atom}",
                c.name,
                r.code,
                i64::from(c.start) + i as i64
            );
        }
    }
    atom
}

pub(super) fn draft_editor(app: &mut PlotiumApp, ui: &mut Ui, panel: &mut Panel) {
    let Some(draft) = &mut panel.draft else {
        return;
    };
    if ui.button("Discard candidate draft").clicked() {
        panel.draft = None;
        return;
    }
    let (d, p) = (draft.dataset, draft.peak);
    let Some(m) = app
        .doc
        .protein
        .bindings
        .get(&d)
        .and_then(|id| app.doc.protein.molecule(*id).ok())
    else {
        ui.label("The draft spectrum is no longer bound.");
        return;
    };
    let Some((v, mark)) = protein::volume(&app.doc, d)
        .ok()
        .and_then(|v| v.peaks.marks.iter().find(|m| m.id == p).map(|m| (v, m)))
    else {
        ui.label("The draft peak no longer exists.");
        return;
    };
    ui.heading(format!("Candidate draft · Peak {}", p.0));
    assignment_context(app, ui, d);
    if app
        .active_dataset()
        .and_then(|i| app.doc.datasets.get(i))
        .map(|d| d.resource_id())
        != Some(d)
    {
        ui.label(
            "Editing a draft for a different spectrum. Save applies to the spectrum named above.",
        );
    }
    ui.small("Atoms follow standard peptide-chain positions and the axis nucleus; protonation is not inferred.");
    for i in 0..3 {
        ui.push_id(i, |ui| {
            let nucleus = v.processed.axes()[i].nucleus().unwrap_or("unknown");
            ui.label(format!("F{} · {nucleus} · {:.3} ppm", i + 1, mark.ppm[i]));
            let old = draft.residues[i];
            ComboBox::from_id_salt("residue")
                .selected_text(draft.targets[i].as_ref().map_or_else(
                    || {
                        if old.is_some() {
                            "Select atoms".into()
                        } else {
                            "Unassigned".into()
                        }
                    },
                    |t| describe_target(m, Some(t)),
                ))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut draft.residues[i], None, "Unassigned");
                    for c in &m.chains {
                        for (n, r) in c.residues.iter().enumerate() {
                            ui.selectable_value(
                                &mut draft.residues[i],
                                Some(r.id),
                                format!("{}:{}{}", c.name, r.code, i64::from(c.start) + n as i64),
                            );
                        }
                    }
                });
            if old != draft.residues[i] {
                draft.targets[i] = None;
            }
            if let Some(r) = m
                .chains
                .iter()
                .flat_map(|c| &c.residues)
                .find(|r| Some(r.id) == draft.residues[i])
            {
                let mut members = draft.targets[i]
                    .as_ref()
                    .map(|t| t.members.clone())
                    .unwrap_or_default();
                let mut meaning = draft.targets[i].as_ref().map_or(GroupMeaning::Unspecified, |t| t.group_meaning);
                let allowed = match protein::residue_atoms(m, r.id) {
                    Ok(atoms) => atoms.collect::<Vec<_>>(),
                    Err(e) => { ui.label(e); return; }
                };
                ui.horizontal_wrapped(|ui| {
                    for (name, element) in allowed {
                        if !protein::atom_matches_nucleus(element, nucleus) {
                            continue;
                        }
                        let mut selected = members.contains(name);
                        if ui.checkbox(&mut selected, name).changed() {
                            if selected {
                                members.insert(name.into());
                            } else {
                                members.remove(name);
                            }
                        }
                    }
                });
                if members.len() > 1 {
                    ui.label("Atom group meaning");
                    ui.radio_value(&mut meaning, GroupMeaning::Unspecified, "Unspecified (candidate only)");
                    ui.radio_value(&mut meaning, GroupMeaning::OneOf, "One of these atoms (identity unresolved)");
                    ui.radio_value(&mut meaning, GroupMeaning::Shared, "All members share this resonance");
                    ui.small("A shared resonance does not imply chemical equivalence or stereospecific assignment.");
                } else {
                    meaning = GroupMeaning::Unspecified;
                }
                draft.targets[i] = (!members.is_empty()).then_some(AtomTarget {
                    residue: r.id,
                    members,
                    group_meaning: meaning,
                });
            }
        });
    }
    ui.label("Candidate note");
    ui.add(
        egui::TextEdit::multiline(&mut draft.note)
            .hint_text("Optional note")
            .desired_rows(2),
    );
    let missing_atoms = draft
        .residues
        .iter()
        .zip(&draft.targets)
        .any(|(residue, target)| {
            residue.is_some_and(|id| {
                target
                    .as_ref()
                    .is_none_or(|t| t.residue != id || t.members.is_empty())
            })
        });
    if missing_atoms {
        ui.label("Choose atoms for each selected residue, or set the axis to Unassigned.");
    }
    if ui
        .add_enabled(!missing_atoms, egui::Button::new("Save candidate"))
        .clicked()
    {
        let mut withdrew = false;
        let edit = if let Some(base) = &draft.baseline {
            let a = app.doc.protein.assignment(d, p);
            if a.and_then(|a| a.candidates.iter().find(|c| c.id == base.id)) != Some(base) {
                panel.feedback =
                    "The candidate changed while editing. Discard this draft and reopen it.".into();
                return;
            }
            withdrew =
                a.is_some_and(|a| a.confirmed == Some(base.id)) && base.targets != draft.targets;
            AssignmentEdit::Update {
                candidate: base.id,
                targets: Some(draft.targets.clone()),
                note: Some(draft.note.clone()),
            }
        } else {
            AssignmentEdit::Add {
                targets: draft.targets.clone(),
                note: draft.note.clone(),
            }
        };
        match app.edit_assignment(d, p, edit) {
            Ok(()) => {
                panel.draft = None;
                panel.feedback = if withdrew {
                    "Targets changed. Confirm the candidate again.".into()
                } else {
                    String::new()
                };
            }
            Err(e) => panel.feedback = e,
        }
    }
}
