use super::*;
use plotium_core::protein_shifts::{ShiftDecision, ShiftReview};

pub(super) fn render(app: &mut PlotiumApp, ui: &mut Ui, panel: &mut Panel) {
    ui.collapsing("Chemical-shift list", |ui| {
        ui.label("By sample conditions and atom target. Different observations require review; no averaging or automatic chemical validation.");
        let rows = match app.chemical_shifts() {
            Ok(rows) => rows,
            Err(e) => { ui.label(e); return; }
        };
        if rows.is_empty() { ui.label("No observations — assign peaks and record sample conditions first."); }
        for row in rows {
            let sample = app.doc.protein.context.samples.iter().find(|s| s.id == row.sample);
            ui.label(format!("{} · {} · {:?} · {:?}", sample.map_or("Unknown sample", |s| s.name.as_str()), residue_label(app, row.target.residue), row.target.members, row.target.group_meaning));
            if row.observations.len() > 1 { ui.label("Multiple observations: compare each value and explicitly select or exclude; no default resolution."); }
            for o in row.observations {
                ui.push_id(format!("{:?}", o.id), |ui| {
                    ui.label(format!("{} · peak {} · F{} · {:.6} ppm · candidate {} · assignment {} / reference {}", spectrum_name(app, o.id.dataset), o.id.peak.0, o.id.axis + 1, o.ppm, o.id.candidate.0, o.assignment_confirmed, o.reference.confirmed));
                    ui.label(format!("Nucleus: {:?} · reference source: {}", o.nucleus, o.reference.source));
                    let mut review = app.shift_review(&o).cloned().unwrap_or(ShiftReview { observation: o, uncertainty_ppm: None, decision: ShiftDecision::Pending, note: String::new() });
                    ui.label(format!("Decision: {:?} (missing/stale review is Pending)", review.decision));
                    let key = ui.id().with("shift_review_draft");
                    let baseline = review.clone();
                    let mut draft = ui.ctx().data(|d| d.get_temp::<(ShiftReview, ShiftReview)>(key)).filter(|(base, _)| *base == baseline).map(|(_, draft)| draft).unwrap_or(review.clone());
                    let mut known = draft.uncertainty_ppm.is_some();
                    ui.checkbox(&mut known, "Known uncertainty (ppm)");
                    if known { ui.add(DragValue::new(draft.uncertainty_ppm.get_or_insert(0.)).speed(0.001).range(0.0..=f64::MAX)); }
                    else { draft.uncertainty_ppm = None; }
                    ui.text_edit_singleline(&mut draft.note);
                    ui.horizontal(|ui| {
                        for (label, decision) in [("Select", ShiftDecision::Selected), ("Exclude", ShiftDecision::Excluded), ("Reset / save error", ShiftDecision::Pending)] {
                            if ui.button(label).clicked() {
                                review.decision = decision;
                                review.note = draft.note.clone();
                                review.uncertainty_ppm = draft.uncertainty_ppm;
                                if let Err(e) = app.review_shift(review.clone()) { app.session.status = e.clone(); panel.feedback = e; }
                            }
                        }
                    });
                    ui.ctx().data_mut(|d| d.insert_temp(key, (baseline, draft)));
                });
            }
        }
    });
}

fn residue_label(app: &PlotiumApp, id: ResidueId) -> String {
    for molecule in &app.doc.protein.molecules {
        for chain in &molecule.chains {
            if let Some((index, residue)) =
                chain.residues.iter().enumerate().find(|(_, r)| r.id == id)
            {
                return format!(
                    "{} / {} / {}{}",
                    molecule.name,
                    chain.name,
                    residue.code,
                    i64::from(chain.start) + index as i64
                );
            }
        }
    }
    format!("Unknown residue ({})", id.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_residue_labels_resolve_molecule_chain_and_sequence_number() {
        let mut app = PlotiumApp::new();
        for name in ["First protein", "Second protein"] {
            app.edit_protein(Edit::Import {
                source: "labels.fasta".into(),
                fasta: ">first\nAG\n>second\nAG\n".into(),
                name: Some(name.into()),
                chains: Some(vec![
                    ChainLabel {
                        name: "A".into(),
                        start: -2,
                    },
                    ChainLabel {
                        name: "B".into(),
                        start: 10,
                    },
                ]),
            })
            .unwrap();
        }
        let first = &app.doc.protein.molecules[0];
        assert_eq!(
            residue_label(&app, first.chains[0].residues[1].id),
            "First protein / A / G-1"
        );
        assert_eq!(
            residue_label(&app, first.chains[1].residues[1].id),
            "First protein / B / G11"
        );
        let second = &app.doc.protein.molecules[1];
        assert_eq!(
            residue_label(&app, second.chains[1].residues[1].id),
            "Second protein / B / G11"
        );
        let missing = ResidueId(app.doc.protein.molecules[0].id.0);
        assert_eq!(
            residue_label(&app, missing),
            format!("Unknown residue ({})", missing.0)
        );
    }
}
