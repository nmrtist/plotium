use super::*;
use crate::protein_shifts::*;

#[test]
fn shifts_preserve_sources_reviews_and_stale_decisions() {
    use crate::protein_context::*;
    let mut app = PlotiumApp::new();
    let m = import(&mut app, "shifts");
    let (dataset, peak, ppm) = add_spectrum(&mut app);
    app.edit_protein(Edit::Bind {
        dataset,
        molecule: Some(m.id),
    })
    .unwrap();
    let second = app
        .add_peak_3d(dataset, [ppm[0] + 0.125, ppm[1], ppm[2]])
        .unwrap();
    let sample = SampleCondition::default();
    let mut context = ScientificContext {
        samples: vec![sample.clone()],
        ..Default::default()
    };
    context.experiments.insert(
        dataset,
        Experiment {
            sample: sample.id,
            kind: ExperimentKind::Unknown,
            roles: Default::default(),
            source: "test".into(),
        },
    );
    app.edit_scientific_context(context).unwrap();
    for peak in [peak, second] {
        app.edit_assignment(
            dataset,
            peak,
            AssignmentEdit::Add {
                targets: [atom(m.chains[0].residues[0].id, &["H"]), None, None],
                note: "partial".into(),
            },
        )
        .unwrap();
    }
    let rows = app.chemical_shifts().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].observations.len(), 2);
    assert!((rows[0].observations[1].ppm - rows[0].observations[0].ppm - 0.125).abs() < 1e-10);
    let o = rows[0].observations[0].clone();
    let review = ShiftReview {
        observation: o.clone(),
        uncertainty_ppm: Some(0.02),
        decision: ShiftDecision::Excluded,
        note: "overlap".into(),
    };
    app.review_shift(review.clone()).unwrap();
    app.undo();
    assert!(app.shift_review(&o).is_none());
    app.redo();
    assert_eq!(app.shift_review(&o), Some(&review));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("shifts.plotium");
    save_project(&app, &path, false).unwrap();
    let loaded = load_project(&path).unwrap();
    assert_eq!(loaded.doc.protein, app.doc.protein);
    let mut invalid = review.clone();
    invalid.uncertainty_ppm = Some(-1.);
    assert!(app.review_shift(invalid).is_err());
    let mut selected = review.clone();
    selected.decision = ShiftDecision::Selected;
    assert!(app.review_shift(selected).is_err());
    app.set_shift_reference(
        dataset,
        crate::protein_reference::ShiftReference {
            offsets_ppm: [0.1, 0., 0.],
            source: "standard".into(),
            confirmed: true,
        },
    )
    .unwrap();
    let fresh = app.chemical_shifts().unwrap()[0].observations[0].clone();
    assert!(app.shift_review(&fresh).is_none());
    assert!(app.review_shift(review).is_err());
    app.edit_assignment(
        dataset,
        peak,
        AssignmentEdit::Confirm {
            candidate: fresh.id.candidate,
        },
    )
    .unwrap();
    let current = app.chemical_shifts().unwrap()[0].observations[0].clone();
    app.review_shift(ShiftReview {
        observation: current.clone(),
        uncertainty_ppm: None,
        decision: ShiftDecision::Selected,
        note: "chosen, not averaged".into(),
    })
    .unwrap();
    assert_eq!(
        app.shift_review(&current).unwrap().decision,
        ShiftDecision::Selected
    );

    sample_edits_invalidate_reviews(&mut app);

    // Alternative group meanings stay separate, even with identical members.
    for meaning in [GroupMeaning::OneOf, GroupMeaning::Shared] {
        let mut group = atom(m.chains[0].residues[0].id, &["HB1", "HB2", "HB3"]).unwrap();
        group.group_meaning = meaning;
        app.edit_assignment(
            dataset,
            peak,
            AssignmentEdit::Add {
                targets: [Some(group), None, None],
                note: "alternative".into(),
            },
        )
        .unwrap();
    }
    let rows = app.chemical_shifts().unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[1].target.group_meaning, GroupMeaning::OneOf);
    assert_eq!(rows[2].target.group_meaning, GroupMeaning::Shared);
    assert_eq!(rows[1].observations.len(), 1);
    assert_eq!(rows[1].target.members.len(), 3);
    assert!(!rows[1].observations[0].assignment_confirmed);
    let (other, other_peak, _) = add_spectrum(&mut app);
    app.edit_protein(Edit::Bind {
        dataset: other,
        molecule: Some(m.id),
    })
    .unwrap();
    app.edit_assignment(
        other,
        other_peak,
        AssignmentEdit::Add {
            targets: [atom(m.chains[0].residues[0].id, &["H"]), None, None],
            note: String::new(),
        },
    )
    .unwrap();
    assert!(app.chemical_shifts().is_err());
    let mut context = app.doc.protein.context.clone();
    let other_sample = SampleCondition::default();
    context.samples.push(other_sample.clone());
    context.experiments.insert(
        other,
        Experiment {
            sample: other_sample.id,
            kind: ExperimentKind::Unknown,
            roles: Default::default(),
            source: "test".into(),
        },
    );
    app.edit_scientific_context(context).unwrap();
    let rows = app.chemical_shifts().unwrap();
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[3].sample, other_sample.id);
    assert_eq!(rows[3].observations.len(), 1);
    save_project(&app, &path, false).unwrap();
    assert_eq!(load_project(&path).unwrap().doc.protein, app.doc.protein);
}

fn sample_edits_invalidate_reviews(app: &mut PlotiumApp) {
    let original = app.doc.protein.context.clone();
    let observation = app.chemical_shifts().unwrap()[0].observations[0].clone();
    let mut changes = Vec::new();
    for field in 0..7 {
        let mut changed = original.clone();
        let sample = &mut changed.samples[0];
        match field {
            0 => sample.ph = Some(6.5),
            1 => sample.temperature_kelvin = Some(298.),
            2 => sample.solvent = "D2O".into(),
            3 => sample.isotope_labeling = "15N".into(),
            4 => sample.construct = "construct B".into(),
            5 => sample.nonstandard_chemistry = "recorded chemistry".into(),
            _ => sample.single_major_conformer = !sample.single_major_conformer,
        }
        changes.push(changed);
    }
    for decision in [ShiftDecision::Selected, ShiftDecision::Excluded] {
        let review = ShiftReview {
            observation: observation.clone(),
            uncertainty_ppm: Some(0.01),
            decision,
            note: "review baseline".into(),
        };
        app.review_shift(review.clone()).unwrap();
        for changed in &changes {
            app.edit_scientific_context(changed.clone()).unwrap();
            let fresh = app.chemical_shifts().unwrap()[0].observations[0].clone();
            assert_eq!(fresh.sample.id, observation.sample.id);
            assert_ne!(fresh, observation);
            assert!(app.shift_review(&fresh).is_none());
            // Unsaved drafts carry the same old snapshot and cannot be applied.
            assert!(app.review_shift(review.clone()).is_err());
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("changed-conditions.plotium");
            save_project(app, &path, false).unwrap();
            let reopened = load_project(&path).unwrap();
            let restored = reopened.chemical_shifts().unwrap()[0].observations[0].clone();
            assert_eq!(restored, fresh);
            assert!(reopened.shift_review(&restored).is_none());
            assert_eq!(
                reopened.doc.protein.shift_reviews,
                app.doc.protein.shift_reviews
            );
            app.undo();
            assert_eq!(app.shift_review(&observation), Some(&review));
            app.redo();
            let fresh = app.chemical_shifts().unwrap()[0].observations[0].clone();
            assert!(app.shift_review(&fresh).is_none());
            app.undo();
        }
    }
}
