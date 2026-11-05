use super::*;
use crate::project::{load_project, save_project};

const FASTA: &str = include_str!("../../io/tests/fixtures/protein/two-chains.fasta");

fn import(app: &mut PlotiumApp, name: &str) -> Molecule {
    app.edit_protein(Edit::Import {
        source: format!("{name}.fasta"),
        fasta: FASTA.into(),
        name: None,
        chains: None,
    })
    .unwrap();
    app.doc.protein.molecules.last().unwrap().clone()
}

fn add_spectrum(app: &mut PlotiumApp) -> (DatasetId, Peak3DId, [f64; 3]) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/bruker-3d-qf");
    let (dataset, _) =
        crate::workflow::dataset_from_acquisition(plotium_io::load_path(path).unwrap().acquisition)
            .unwrap();
    let id = dataset.resource_id();
    app.doc.datasets.push(dataset);
    let v = volume(&app.doc, id).unwrap();
    let ppm = std::array::from_fn(|a| v.processed.axis_coordinate(a, 1).unwrap());
    (id, app.add_peak_3d(id, ppm).unwrap(), ppm)
}

fn atom(residue: ResidueId, names: &[&str]) -> Option<AtomTarget> {
    Some(AtomTarget {
        residue,
        group_meaning: if names.len() > 1 {
            GroupMeaning::OneOf
        } else {
            GroupMeaning::Unspecified
        },
        members: names.iter().map(|s| (*s).into()).collect(),
    })
}

#[test]
fn manual_assignments_keep_scientific_identity_through_edits_and_project_roundtrip() {
    let mut app = PlotiumApp::new();
    let m = import(&mut app, "first");
    let other = import(&mut app, "second");
    let (d, p, ppm) = add_spectrum(&mut app);
    let (d2, p2, _) = add_spectrum(&mut app);
    assert_ne!(d, d2);
    for dataset in [d, d2] {
        app.edit_protein(Edit::Bind {
            dataset,
            molecule: Some(m.id),
        })
        .unwrap();
    }
    let a = m.chains[0].residues[0].id;
    let b = m.chains[1].residues[0].id;
    assert_eq!(m.chains[0].start, m.chains[1].start);
    assert_ne!(a, b);
    let targets = [
        atom(a, &["HB1", "HB2", "HB3"]),
        atom(b, &["N"]),
        atom(b, &["HA2"]),
    ];
    let partial = [atom(a, &["H"]), None, None];
    let second_full = [atom(b, &["HA3"]), atom(a, &["N"]), atom(a, &["HA"])];
    for (dataset, peak, targets) in [
        (d, p, targets.clone()),
        (d, p, second_full),
        (d2, p2, partial.clone()),
    ] {
        app.edit_assignment(
            dataset,
            peak,
            AssignmentEdit::Add {
                targets,
                note: "review".into(),
            },
        )
        .unwrap();
    }
    let id = app.doc.protein.assignment(d, p).unwrap().candidates[0].id;
    let partial_id = app.doc.protein.assignment(d2, p2).unwrap().candidates[0].id;
    for (dataset, peak, candidate) in [(d, p, id), (d2, p2, partial_id)] {
        app.edit_assignment(dataset, peak, AssignmentEdit::Confirm { candidate })
            .unwrap();
    }
    let original = app.doc.protein.clone();
    let revision = app.doc.automation_revision;
    for bad in [
        [atom(a, &["CA"]), None, None],
        [atom(a, &["H", "CA"]), None, None],
        [atom(a, &["missing"]), None, None],
        [atom(other.chains[0].residues[0].id, &["H"]), None, None],
        [None, None, None],
    ] {
        assert!(
            app.edit_assignment(
                d,
                p,
                AssignmentEdit::Add {
                    targets: bad,
                    note: String::new()
                }
            )
            .is_err()
        );
    }
    assert!(
        app.edit_assignment(d, Peak3DId(u64::MAX), AssignmentEdit::Clear)
            .is_err()
    );
    assert!(
        app.edit_protein(Edit::Bind {
            dataset: d,
            molecule: Some(other.id)
        })
        .is_err()
    );
    assert!(app.edit_protein(Edit::Delete { molecule: m.id }).is_err());
    assert_eq!(app.doc.protein, original);
    assert_eq!(app.doc.automation_revision, revision);

    app.edit_assignment(
        d,
        p,
        AssignmentEdit::Update {
            candidate: id,
            targets: None,
            note: Some("new note".into()),
        },
    )
    .unwrap();
    assert_eq!(
        app.doc.protein.assignment(d, p).unwrap().confirmed,
        Some(id)
    );
    app.edit_assignment(
        d,
        p,
        AssignmentEdit::Update {
            candidate: id,
            targets: Some(partial),
            note: None,
        },
    )
    .unwrap();
    assert_eq!(app.doc.protein.assignment(d, p).unwrap().confirmed, None);
    app.undo();
    assert_eq!(
        app.doc.protein.assignment(d, p).unwrap().confirmed,
        Some(id)
    );
    app.redo();
    assert_eq!(app.doc.protein.assignment(d, p).unwrap().confirmed, None);
    app.undo();
    app.edit_assignment(d, p, AssignmentEdit::Withdraw).unwrap();
    assert_eq!(app.doc.protein.assignment(d, p).unwrap().confirmed, None);
    app.undo();
    app.edit_assignment(d, p, AssignmentEdit::Delete { candidate: id })
        .unwrap();
    assert_eq!(
        app.doc.protein.assignment(d, p).unwrap().candidates.len(),
        1
    );
    app.undo();

    app.edit_protein(Edit::LabelChain {
        molecule: m.id,
        chain: m.chains[0].id,
        label: ChainLabel {
            name: "Alpha".into(),
            start: 42,
        },
    })
    .unwrap();
    assert_eq!(
        app.doc.protein.molecule(m.id).unwrap().chains[0].residues[0].id,
        a
    );
    let assigned = app.doc.protein.clone();
    for axis in 0..3 {
        app.select_nmr_volume_plane(d, axis, 0).unwrap();
        app.locate_peak_3d(d, p).unwrap();
    }
    let baseline = volume(&app.doc, d).unwrap().phases;
    let mut phases = baseline;
    phases[2].p0_degrees = 180.;
    let mut worker = app
        .doc
        .dataset_by_id(d)
        .unwrap()
        .as_nmr2d()
        .unwrap()
        .clone();
    worker.set_volume_phases(phases).unwrap();
    let moved = std::array::from_fn(|a| {
        volume(&app.doc, d)
            .unwrap()
            .processed
            .axis_coordinate(a, 0)
            .unwrap()
    });
    assert_ne!(moved, ppm);
    app.edit_peak_3d(d, p, moved, "moved during worker".into())
        .unwrap();
    app.install_nmr_volume_result(d, baseline, worker).unwrap();
    assert_eq!(app.doc.protein, assigned);
    assert_eq!(volume(&app.doc, d).unwrap().peaks.marks[0].ppm, moved);
    let report = app.export_protein(m.id, false).unwrap();
    assert_eq!(report["spectra"].as_array().unwrap().len(), 2);
    let confirmed = app.export_protein(m.id, true).unwrap();
    assert!(
        confirmed["spectra"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["assignments"][0]["candidates"].as_array().unwrap().len() == 1)
    );
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("protein.plotium");
    save_project(&app, &path, false).unwrap();
    let reopened = load_project(&path).unwrap();
    assert_eq!(reopened.doc.protein, assigned);
    assert_eq!(reopened.export_protein(m.id, false).unwrap(), report);
    app.delete_peak_3d(d, p).unwrap();
    assert!(app.doc.protein.assignment(d, p).is_none());
    app.undo();
    assert_eq!(app.doc.protein, assigned);
    app.redo();
    assert!(app.doc.protein.assignment(d, p).is_none());
    app.undo();
    let index = app.doc.dataset_index(d).unwrap();
    app.try_execute_action(app.delete_dataset_action(index).unwrap())
        .unwrap();
    assert!(!app.doc.protein.bindings.contains_key(&d));
    assert!(app.doc.protein.assignment(d, p).is_none());
    app.undo();
    assert_eq!(app.doc.protein, assigned);
    app.redo();
    assert!(app.doc.protein.assignment(d, p).is_none());
}

#[test]
fn missing_nucleus_allows_recording_but_not_confirmation() {
    let mut app = PlotiumApp::new();
    let m = import(&mut app, "unknown");
    let t = atom(m.chains[0].residues[0].id, &["H"]).unwrap();
    assert!(validate_target(&m, &t, "", false).is_ok());
    assert!(
        validate_target(&m, &t, "", true)
            .unwrap_err()
            .contains("known nucleus")
    );
}

#[test]
fn undoing_peak_creation_does_not_recycle_its_id() {
    let mut app = PlotiumApp::new();
    let (dataset, original, ppm) = add_spectrum(&mut app);
    app.undo();
    assert!(volume(&app.doc, dataset).unwrap().peaks.marks.is_empty());
    app.redo();
    assert_eq!(
        volume(&app.doc, dataset).unwrap().peaks.marks[0].id,
        original
    );
    app.undo();
    let replacement_ppm = std::array::from_fn(|axis| {
        volume(&app.doc, dataset)
            .unwrap()
            .processed
            .axis_coordinate(axis, 0)
            .unwrap()
    });
    assert_ne!(replacement_ppm, ppm);
    let replacement = app.add_peak_3d(dataset, replacement_ppm).unwrap();
    assert_ne!(replacement, original);
    assert!(
        app.edit_assignment(dataset, original, AssignmentEdit::Clear)
            .is_err()
    );
    app.undo();
    assert!(app.add_peak_3d(dataset, ppm).unwrap().0 > replacement.0);
}

#[test]
fn peptide_positions_reject_leaving_atoms_without_using_display_numbers() {
    let mut app = PlotiumApp::new();
    app.edit_protein(Edit::Import {
        source: "positions.fasta".into(),
        fasta: ">proline\nPAP\n>single\nA".into(),
        name: None,
        chains: None,
    })
    .unwrap();
    let m = app.doc.protein.molecules[0].clone();
    let (d, p, _) = add_spectrum(&mut app);
    app.edit_protein(Edit::Bind {
        dataset: d,
        molecule: Some(m.id),
    })
    .unwrap();
    let revision = app.doc.automation_revision;
    // Internal and C-terminal proline cannot carry the backbone amide H.
    for (index, name) in [(1, "H2"), (1, "HXT"), (2, "H")] {
        let error = app
            .edit_assignment(
                d,
                p,
                AssignmentEdit::Add {
                    targets: [atom(m.chains[0].residues[index].id, &[name]), None, None],
                    note: String::new(),
                },
            )
            .unwrap_err();
        assert!(error.contains("standard peptide chain"), "{error}");
    }
    assert_eq!(app.doc.automation_revision, revision);
    assert!(app.doc.protein.assignment(d, p).is_none());
    for (chain, index, name, nucleus) in [
        (0, 0, "H", "1H"),
        (0, 1, "H", "1H"),
        (0, 2, "HXT", "1H"),
        (0, 2, "OXT", "17O"),
        (1, 0, "H2", "1H"),
        (1, 0, "HXT", "1H"),
    ] {
        validate_target(
            &m,
            &atom(m.chains[chain].residues[index].id, &[name]).unwrap(),
            nucleus,
            true,
        )
        .unwrap();
    }
    for c in &m.chains {
        let before: Vec<_> = c
            .residues
            .iter()
            .map(|r| residue_atoms(&m, r.id).unwrap().collect::<Vec<_>>())
            .collect();
        app.edit_protein(Edit::LabelChain {
            molecule: m.id,
            chain: c.id,
            label: ChainLabel {
                name: c.name.clone(),
                start: -42,
            },
        })
        .unwrap();
        let renamed = app.doc.protein.molecule(m.id).unwrap();
        let after: Vec<_> = c
            .residues
            .iter()
            .map(|r| residue_atoms(renamed, r.id).unwrap().collect::<Vec<_>>())
            .collect();
        assert_eq!(before, after);
    }
    // The original review case must fail as well, even for an unconfirmed candidate.
    let internal_pro = import_preview("review.fasta", ">chain\nAPA").unwrap();
    for name in ["H", "HXT"] {
        assert!(
            validate_target(
                &internal_pro,
                &atom(internal_pro.chains[0].residues[1].id, &[name]).unwrap(),
                "1H",
                false
            )
            .is_err()
        );
    }
}

#[test]
fn group_interpretation_is_explicit_and_survives_confirmation_undo_and_export() {
    let mut app = PlotiumApp::new();
    let m = import(&mut app, "group");
    let (d, p, _) = add_spectrum(&mut app);
    app.edit_protein(Edit::Bind {
        dataset: d,
        molecule: Some(m.id),
    })
    .unwrap();
    let residue = m.chains[0].residues[0].id;
    let unspecified: AtomTarget = serde_json::from_value(serde_json::json!({
        "residue":residue,"members":["HB1","HB2","HB3"]
    }))
    .unwrap();
    let targets = [Some(unspecified), None, None];
    app.edit_assignment(
        d,
        p,
        AssignmentEdit::Add {
            targets: targets.clone(),
            note: String::new(),
        },
    )
    .unwrap();
    let id = app.doc.protein.assignment(d, p).unwrap().candidates[0].id;
    let revision = app.doc.automation_revision;
    assert!(
        app.edit_assignment(d, p, AssignmentEdit::Confirm { candidate: id })
            .unwrap_err()
            .contains("shared resonance")
    );
    assert_eq!(revision, app.doc.automation_revision);
    for meaning in [GroupMeaning::Shared, GroupMeaning::OneOf] {
        let mut targets = targets.clone();
        targets[0].as_mut().unwrap().group_meaning = meaning;
        app.edit_assignment(
            d,
            p,
            AssignmentEdit::Update {
                candidate: id,
                targets: Some(targets),
                note: None,
            },
        )
        .unwrap();
        assert!(
            app.doc
                .protein
                .assignment(d, p)
                .unwrap()
                .confirmed
                .is_none()
        );
        app.edit_assignment(d, p, AssignmentEdit::Confirm { candidate: id })
            .unwrap();
        let report = app.export_protein(m.id, true).unwrap();
        assert_eq!(
            report["spectra"][0]["assignments"][0]["candidates"][0]["targets"][0]["group_meaning"],
            serde_json::to_value(meaning).unwrap()
        );
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("groups.plotium");
        save_project(&app, &path, false).unwrap();
        assert_eq!(
            load_project(&path)
                .unwrap()
                .export_protein(m.id, true)
                .unwrap(),
            report
        );
        if meaning == GroupMeaning::OneOf {
            app.undo(); // confirmation
            app.undo(); // meaning change
            let restored = app.doc.protein.assignment(d, p).unwrap();
            assert_eq!(restored.confirmed, Some(id));
            assert_eq!(
                restored.candidates[0].targets[0]
                    .as_ref()
                    .unwrap()
                    .group_meaning,
                GroupMeaning::Shared
            );
            app.redo();
            assert!(
                app.doc
                    .protein
                    .assignment(d, p)
                    .unwrap()
                    .confirmed
                    .is_none()
            );
        }
    }
}
#[test]
fn scientific_context_roundtrip_undo_and_scope_boundaries() {
    use crate::protein_context::*;
    let mut app = PlotiumApp::new();
    let m = import(&mut app, "context");
    let (dataset, _, _) = add_spectrum(&mut app);
    app.edit_protein(Edit::Bind {
        dataset,
        molecule: Some(m.id),
    })
    .unwrap();
    let mut context = ScientificContext::default();
    let sample = SampleCondition::default();
    context.experiments.insert(
        dataset,
        Experiment {
            sample: sample.id,
            kind: ExperimentKind::Unknown,
            roles: Default::default(),
            source: "manual experimental record".into(),
        },
    );
    context.samples.push(sample);
    app.edit_scientific_context(context.clone()).unwrap();
    assert!(
        context
            .check_structure_scope(&app.doc, dataset)
            .unwrap_err()
            .contains("single")
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("context.plotium");
    save_project(&app, &path, false).unwrap();
    let loaded = load_project(&path).unwrap();
    assert_eq!(loaded.doc.protein.context, context);
    let mut invalid = context.clone();
    invalid.samples[0].temperature_kelvin = Some(-1.0);
    assert!(app.edit_scientific_context(invalid).is_err());
    assert_eq!(app.doc.protein.context, context);
    app.undo();
    assert!(app.doc.protein.context.is_empty());
    app.redo();
    assert_eq!(app.doc.protein.context, context);
    app.doc
        .protein
        .context
        .experiments
        .get_mut(&dataset)
        .unwrap()
        .sample = SampleConditionId(uuid::Uuid::new_v4());
    assert!(app.doc.protein.validate(&app.doc).is_err());
}
#[test]
fn structure_scope_requires_explicit_roles_and_supported_chemistry() {
    use crate::protein_context::*;
    let mut app = PlotiumApp::new();
    app.edit_protein(Edit::Import {
        source: "single.fasta".into(),
        fasta: include_str!("../../io/tests/fixtures/protein/ubiquitin-bmrb-6457.fasta").into(),
        name: None,
        chains: None,
    })
    .unwrap();
    let molecule = app.doc.protein.molecules[0].id;
    let (dataset, _, _) = add_spectrum(&mut app);
    app.edit_protein(Edit::Bind {
        dataset,
        molecule: Some(molecule),
    })
    .unwrap();
    let sample = SampleCondition {
        single_major_conformer: true,
        ..Default::default()
    };
    let mut context = ScientificContext::default();
    context.experiments.insert(
        dataset,
        Experiment {
            sample: sample.id,
            kind: ExperimentKind::Unknown,
            roles: Default::default(),
            source: String::new(),
        },
    );
    context.samples.push(sample);
    assert!(
        context
            .check_structure_scope(&app.doc, dataset)
            .unwrap_err()
            .contains("role")
    );
    let e = context.experiments.get_mut(&dataset).unwrap();
    e.kind = ExperimentKind::Noesy;
    e.roles = [
        ExperimentAxisRole::IndirectProton,
        ExperimentAxisRole::Nitrogen,
        ExperimentAxisRole::ObservedProton,
    ];
    context.check_noe_roles(&app.doc, dataset).unwrap();
    context.experiments.get_mut(&dataset).unwrap().roles[0] = ExperimentAxisRole::ObservedProton;
    assert!(context.check_noe_roles(&app.doc, dataset).is_err());
    context.experiments.get_mut(&dataset).unwrap().roles[0] = ExperimentAxisRole::Carbon;
    assert!(context.validate(&app.doc).unwrap_err().contains("F1"));
    context.experiments.get_mut(&dataset).unwrap().roles[0] = ExperimentAxisRole::IndirectProton;
    context.samples[0].nonstandard_chemistry = "ligand".into();
    context.validate(&app.doc).unwrap();
    assert!(
        context
            .check_structure_scope(&app.doc, dataset)
            .unwrap_err()
            .contains("ligands")
    );
}
#[test]
fn sample_validation_is_atomic_at_ph_boundaries_and_duplicate_ids() {
    use crate::protein_context::*;
    let mut app = PlotiumApp::new();
    let mut context = ScientificContext::default();
    context.samples.push(SampleCondition::default());
    for ph in [0.0, 14.0] {
        context.samples[0].ph = Some(ph);
        app.edit_scientific_context(context.clone()).unwrap();
        assert_eq!(app.doc.protein.context, context);
    }
    let before = app.doc.protein.clone();
    for ph in [-0.001, 14.001, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut invalid = context.clone();
        invalid.samples[0].ph = Some(ph);
        assert!(app.edit_scientific_context(invalid).is_err());
        assert_eq!(app.doc.protein, before);
    }
    context.samples.push(context.samples[0].clone());
    assert!(
        app.edit_scientific_context(context)
            .unwrap_err()
            .contains("unique")
    );
    assert_eq!(app.doc.protein, before);
    app.undo();
    assert_eq!(app.doc.protein.context.samples[0].ph, Some(0.0));
}

#[test]
fn deleting_spectrum_cleans_only_its_metadata_and_undo_restores_it() {
    use crate::protein_context::*;
    let mut app = PlotiumApp::new();
    let (dataset, _, _) = add_spectrum(&mut app);
    let (other, _, _) = add_spectrum(&mut app);
    let mut context = ScientificContext::default();
    let sample = SampleCondition::default();
    for d in [dataset, other] {
        context.experiments.insert(
            d,
            Experiment {
                sample: sample.id,
                kind: ExperimentKind::Unknown,
                roles: Default::default(),
                source: d.to_string(),
            },
        );
    }
    context.samples.push(sample);
    app.edit_scientific_context(context.clone()).unwrap();
    let index = app
        .doc
        .datasets
        .iter()
        .position(|d| d.resource_id() == dataset)
        .unwrap();
    app.try_execute_action(app.delete_dataset_action(index).unwrap())
        .unwrap();
    let after = app.doc.protein.context.clone();
    assert!(!after.experiments.contains_key(&dataset));
    assert_eq!(
        after.experiments.get(&other),
        context.experiments.get(&other)
    );
    assert_eq!(after.samples, context.samples);
    app.doc.protein.validate(&app.doc).unwrap();
    assert!(
        app.save_scientific_context_draft(&context, context.clone())
            .is_err()
    );
    assert_eq!(app.doc.protein.context, after);
    app.undo();
    assert_eq!(app.doc.protein.context, context);
    app.doc.protein.validate(&app.doc).unwrap();
    app.redo();
    assert_eq!(app.doc.protein.context, after);
}
