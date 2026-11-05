use super::*;
#[test]
fn peak_identity_coordinates_and_notes_survive_navigation_undo_and_project() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/bruker-3d-qf");
    let acquisition = plotium_io::load_path(&fixture).unwrap().acquisition;
    let (dataset, _) = crate::workflow::dataset_from_acquisition(acquisition).unwrap();
    let mut app = PlotiumApp::new();
    let dataset_id = dataset.resource_id();
    app.doc.datasets.push(dataset);
    let v = app.doc.datasets[0]
        .as_nmr2d()
        .unwrap()
        .volume
        .as_ref()
        .unwrap();
    let ppm = [
        v.processed.axis_coordinate(0, 1).unwrap(),
        v.processed.axis_coordinate(1, 1).unwrap(),
        v.processed.axis_coordinate(2, 2).unwrap(),
    ];
    let intensity = v.processed.signed_value([1, 1, 2]).unwrap();
    let id = app.add_peak_3d(dataset_id, ppm).unwrap();
    app.edit_peak_3d(dataset_id, id, ppm, "overlap".into())
        .unwrap();
    for axis in 0..3 {
        app.select_nmr_volume_plane(dataset_id, axis, 0).unwrap();
        app.locate_peak_3d(dataset_id, id).unwrap();
    }
    let current = app.doc.datasets[0]
        .as_nmr2d()
        .unwrap()
        .volume
        .as_ref()
        .unwrap();
    assert_eq!(current.peaks.marks[0].intensity, intensity);
    assert_eq!(current.index, 2);
    let before = current.peaks.clone();
    assert!(
        app.edit_peak_3d(dataset_id, id, [f64::NAN; 3], String::new())
            .is_err()
    );
    app.delete_peak_3d(dataset_id, id).unwrap();
    app.undo();
    assert_eq!(
        app.doc.datasets[0]
            .as_nmr2d()
            .unwrap()
            .volume
            .as_ref()
            .unwrap()
            .peaks,
        before
    );
    let path =
        std::env::temp_dir().join(format!("plotium-peaks3d-{}.plotium", uuid::Uuid::new_v4()));
    crate::project::save_project(&app, &path, false).unwrap();
    let restored = crate::project::load_project(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(
        restored.doc.datasets[0]
            .as_nmr2d()
            .unwrap()
            .volume
            .as_ref()
            .unwrap()
            .peaks,
        before
    );
}

fn fixture() -> (PlotiumApp, DatasetId) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/bruker-3d-qf");
    let (dataset, _) =
        crate::workflow::dataset_from_acquisition(plotium_io::load_path(path).unwrap().acquisition)
            .unwrap();
    let id = dataset.resource_id();
    let mut app = PlotiumApp::new();
    app.doc.datasets.push(dataset);
    (app, id)
}
fn volume(app: &PlotiumApp) -> &NmrVolumeState {
    app.doc.datasets[0]
        .as_nmr2d()
        .unwrap()
        .volume
        .as_ref()
        .unwrap()
}

// An independent direct DFT of the small QF acquisition gives numerical answers
// without using the volume sampler or FFT that peak operations exercise.
fn expected_height(indices: [usize; 3]) -> f64 {
    use std::f64::consts::TAU;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/bruker-3d-qf/ser");
    let bytes = std::fs::read(path).unwrap();
    let samples: Vec<_> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f64::from(i32::from_le_bytes(*b)))
        .collect();
    let frequencies = [
        indices[0] as f64 - 2.,
        indices[1] as f64 - 1.,
        indices[2] as f64 - 2.,
    ];
    let mut expected = 0.;
    for i in 0..5 {
        for j in 0..3 {
            for k in 0..4 {
                let offset = ((i * 3 + j) * 4 + k) * 2;
                let angle = -TAU * frequencies[2] * k as f64 / 4.;
                let weight = (TAU * frequencies[0] * i as f64 / 5.).cos()
                    * (TAU * frequencies[1] * j as f64 / 3.).cos();
                expected +=
                    weight * (samples[offset] * angle.cos() - samples[offset + 1] * angle.sin());
            }
        }
    }
    expected
}

#[test]
fn manual_peaks_map_unequal_axes_duplicate_nuclei_signed_samples_and_boundaries() {
    let (mut app, owner) = fixture();
    assert_eq!(volume(&app).processed.shape(), [5, 3, 4]);
    assert_eq!(
        volume(&app).processed.axes()[0].nucleus(),
        volume(&app).processed.axes()[2].nucleus()
    );
    let mut negative = false;
    for a in 0..5 {
        for b in 0..3 {
            for c in 0..4 {
                let indices = [a, b, c];
                let ppm = std::array::from_fn(|axis| {
                    volume(&app)
                        .processed
                        .axis_coordinate(axis, indices[axis])
                        .unwrap()
                });
                let id = app.add_peak_3d(owner, ppm).unwrap();
                let mark = volume(&app).peaks.marks.last().unwrap();
                assert_eq!((mark.id, mark.ppm), (id, ppm));
                let expected = expected_height(indices);
                assert!((mark.intensity - expected).abs() < 1e-8);
                negative |= mark.intensity < -1e-8;
                assert_eq!(volume(&app).peak_indices(ppm).unwrap(), indices);
                for (axis, index) in indices.into_iter().enumerate() {
                    app.select_nmr_volume_plane(owner, axis, 0).unwrap();
                    app.locate_peak_3d(owner, id).unwrap();
                    assert_eq!(volume(&app).index, index);
                    assert_eq!(app.session.ui.selected_peak_3d, Some((owner, id)));
                }
            }
        }
    }
    assert!(negative);
    let v = volume(&app);
    let mut ppm = std::array::from_fn(|axis| v.processed.axis_coordinate(axis, 0).unwrap());
    ppm[0] += (v.processed.axis_coordinate(0, 1).unwrap() - ppm[0]) * 0.2;
    let id = app.add_peak_3d(owner, ppm).unwrap();
    assert_eq!(volume(&app).peaks.marks.last().unwrap().ppm, ppm);
    assert!(
        (volume(&app).peaks.marks.last().unwrap().intensity - expected_height([0; 3])).abs() < 1e-8
    );
    let before = volume(&app).peaks.clone();
    for invalid in [[f64::NAN; 3], [f64::INFINITY; 3], [1e10; 3]] {
        assert!(app.add_peak_3d(owner, invalid).is_err());
        assert!(
            app.edit_peak_3d(owner, id, invalid, "invalid".into())
                .is_err()
        );
        assert_eq!(volume(&app).peaks, before);
    }
}

#[test]
fn phase_refresh_preserves_edits_made_during_worker_and_roundtrips_export() {
    let (mut app, owner) = fixture();
    let indices = [1, 1, 2];
    let ppm = std::array::from_fn(|axis| {
        volume(&app)
            .processed
            .axis_coordinate(axis, indices[axis])
            .unwrap()
    });
    let id = app.add_peak_3d(owner, ppm).unwrap();
    let before_export = app.peak_table_3d(owner).unwrap();
    let baseline = volume(&app).phases;
    let mut phases = baseline;
    phases[2].p0_degrees = 180.;
    let mut worker = app.doc.datasets[0].as_nmr2d().unwrap().clone();
    worker.set_volume_phases(phases).unwrap();
    app.edit_peak_3d(owner, id, ppm, "edited during phase processing".into())
        .unwrap();
    let other = app.add_peak_3d(owner, ppm).unwrap();
    app.install_nmr_volume_result(owner, baseline, worker)
        .unwrap();
    for mark in &volume(&app).peaks.marks {
        assert_eq!(mark.ppm, ppm);
        assert!((mark.intensity + expected_height(indices)).abs() < 1e-8);
    }
    assert_eq!(volume(&app).peaks.marks[0].id, id);
    assert_eq!(volume(&app).peaks.marks[1].id, other);
    let after = volume(&app).peaks.clone();
    app.undo();
    assert!((volume(&app).peaks.marks[0].intensity - expected_height(indices)).abs() < 1e-8);
    app.redo();
    assert_eq!(volume(&app).peaks, after);
    let table = app.peak_table_3d(owner).unwrap();
    assert_eq!(table["source"], before_export["source"]);
    assert_ne!(
        table["processing"]["canonical_dataset_sha256"],
        before_export["processing"]["canonical_dataset_sha256"]
    );
    assert_eq!(table["processing"]["phases"][2]["p0_degrees"], 180.);
    let path = std::env::temp_dir().join(format!(
        "plotium-peaks-phase-{}.plotium",
        uuid::Uuid::new_v4()
    ));
    crate::project::save_project(&app, &path, false).unwrap();
    let restored = crate::project::load_project(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(restored.peak_table_3d(owner).unwrap(), table);
}

#[test]
fn reference_is_absolute_and_survives_phase_undo_and_project() {
    use crate::protein_reference::ShiftReference;
    let (mut app, dataset) = fixture();
    let original = crate::protein::volume(&app.doc, dataset)
        .unwrap()
        .processed
        .axis_coordinate(0, 1)
        .unwrap();
    let ppm = std::array::from_fn(|a| {
        crate::protein::volume(&app.doc, dataset)
            .unwrap()
            .processed
            .axis_coordinate(a, 1)
            .unwrap()
    });
    let id = app.add_peak_3d(dataset, ppm).unwrap();
    let reference = ShiftReference {
        offsets_ppm: [0.1, -0.2, 0.3],
        source: "Manual standard comparison".into(),
        confirmed: true,
    };
    app.set_shift_reference(dataset, reference.clone()).unwrap();
    app.set_shift_reference(dataset, reference.clone()).unwrap();
    let check = |app: &PlotiumApp| {
        let v = crate::protein::volume(&app.doc, dataset).unwrap();
        assert!((v.processed.axis_coordinate(0, 1).unwrap() - original - 0.1).abs() < 1e-10);
        assert_eq!(v.peaks.marks[0].id, id);
        assert!((v.peaks.marks[0].ppm[0] - ppm[0] - 0.1).abs() < 1e-10);
        assert_eq!(
            app.peak_table_3d(dataset).unwrap()["reference"]["confirmed"],
            true
        );
    };
    check(&app);
    app.set_nmr_volume_phases(
        dataset,
        [plotium_processing::nmr_volume::VolumePhase::default(); 3],
    )
    .unwrap();
    check(&app);
    app.undo();
    check(&app);
    app.redo();
    check(&app);
    let path = std::env::temp_dir().join(format!(
        "plotium-reference-{}.plotium",
        uuid::Uuid::new_v4()
    ));
    crate::project::save_project(&app, &path, false).unwrap();
    let restored = crate::project::load_project(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    check(&restored);
    let before = crate::protein::volume(&app.doc, dataset)
        .unwrap()
        .peaks
        .clone();
    assert!(
        app.set_shift_reference(
            dataset,
            ShiftReference {
                offsets_ppm: [f64::NAN; 3],
                ..reference
            }
        )
        .is_err()
    );
    assert_eq!(
        crate::protein::volume(&app.doc, dataset).unwrap().peaks,
        before
    );
    assert!(app.check_reference_comparison(dataset, dataset).is_err());
}

#[test]
fn reference_comparison_requires_same_sample_and_explicit_review() {
    use crate::protein_context::*;
    use crate::protein_reference::ShiftReference;
    let (mut app, first) = fixture();
    let (other, _) = fixture();
    app.doc.datasets.push(other.doc.datasets[0].clone());
    let second_actual = app.doc.datasets[1].resource_id();
    let sample = SampleCondition::default();
    let other_sample = SampleCondition::default();
    let mut context = ScientificContext {
        samples: vec![sample.clone(), other_sample.clone()],
        ..Default::default()
    };
    let experiment = Experiment {
        sample: sample.id,
        kind: ExperimentKind::Unknown,
        roles: Default::default(),
        source: "test".into(),
    };
    context.experiments.insert(first, experiment.clone());
    context.experiments.insert(second_actual, experiment);
    app.edit_scientific_context(context.clone()).unwrap();
    assert!(
        app.check_reference_comparison(first, second_actual)
            .unwrap_err()
            .contains("confirm")
    );
    for id in [first, second_actual] {
        app.set_shift_reference(
            id,
            ShiftReference {
                source: "External standard".into(),
                confirmed: true,
                ..Default::default()
            },
        )
        .unwrap();
    }
    app.check_reference_comparison(first, second_actual)
        .unwrap();
    context.experiments.get_mut(&second_actual).unwrap().sample = other_sample.id;
    app.edit_scientific_context(context).unwrap();
    assert!(
        app.check_reference_comparison(first, second_actual)
            .unwrap_err()
            .contains("different samples")
    );
}

#[path = "reference_undo_tests.rs"]
mod reference_undo;
