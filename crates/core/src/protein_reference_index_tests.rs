use super::*;

#[test]
fn reference_index_keeps_all_matching_peak_pairs() {
    use crate::protein_context::*;
    use crate::protein_reference::ShiftReference;
    let mut app = PlotiumApp::new();
    let molecule = import(&mut app, "index");
    let (left, lp, _) = add_spectrum(&mut app);
    let (right, rp, _) = add_spectrum(&mut app);
    let sample = SampleCondition::default();
    let mut context = ScientificContext {
        samples: vec![sample.clone()],
        ..Default::default()
    };
    for (dataset, peak) in [(left, lp), (right, rp)] {
        app.edit_protein(Edit::Bind {
            dataset,
            molecule: Some(molecule.id),
        })
        .unwrap();
        let ppm = volume(&app.doc, dataset).unwrap().peaks.marks[0].ppm;
        let duplicate = app.add_peak_3d(dataset, ppm).unwrap();
        for peak in [peak, duplicate] {
            app.edit_assignment(
                dataset,
                peak,
                AssignmentEdit::Add {
                    targets: [atom(molecule.chains[0].residues[0].id, &["H"]), None, None],
                    note: String::new(),
                },
            )
            .unwrap();
            let candidate = app
                .doc
                .protein
                .assignment(dataset, peak)
                .unwrap()
                .candidates[0]
                .id;
            app.edit_assignment(dataset, peak, AssignmentEdit::Confirm { candidate })
                .unwrap();
        }
        context.experiments.insert(
            dataset,
            Experiment {
                sample: sample.id,
                kind: ExperimentKind::Unknown,
                roles: Default::default(),
                source: "test".into(),
            },
        );
        app.set_shift_reference(
            dataset,
            ShiftReference {
                source: "Standard".into(),
                confirmed: true,
                ..Default::default()
            },
        )
        .unwrap();
    }
    app.edit_scientific_context(context).unwrap();
    let rows = app.reference_differences(left, right).unwrap();
    assert_eq!(rows.len(), 4);
    let pairs: HashSet<_> = rows.iter().map(|r| (r.left_peak, r.right_peak)).collect();
    assert_eq!(pairs.len(), 4);
    assert!(rows.iter().all(|r| r.delta_ppm == 0.));
}
