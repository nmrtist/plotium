use plotium_core::actions::Action;
use plotium_core::state::{
    AfmDataset, AfmForceBranch, AfmHertzDraft, DEFAULT_CANVAS_SIZE_MM, Dataset, DerivationKind,
    PlotiumApp,
};
use plotium_io::{AfmData, AfmForceSet, AfmScale};
use std::sync::Arc;

fn approach(spring: Option<f64>, with_z: bool) -> PlotiumApp {
    // F = indentation^1.5 / 64 nN, k = 1 N/m, linear detector baseline.
    // The displayed force includes a constant 2 nN offset from that baseline.
    let mut app = PlotiumApp::default();
    let z = [83., 90.578125, 95.875, 98.984375, 100., 101., 102., 103.];
    let raw = [192, 155, 136, 129, 128, 128, 128, 128];
    let force = AfmForceSet {
        grid_width: 2,
        grid_height: 1,
        samples_per_curve: raw.len(),
        raw: Arc::from([raw, raw].concat()),
        signal_scale: AfmScale {
            multiplier: 1. / 64.,
            offset: 0.,
            unit: "nm".into(),
        },
        sample_period_s: None,
        z_positions: with_z.then(|| Arc::from(z)),
        display_order: (0..raw.len()).collect(),
        approach_samples: raw.len(),
        deflection_sensitivity_m_per_v: None,
        spring_constant_n_per_m: spring,
    };
    let data = AfmDataset::load(AfmData {
        images: vec![],
        forces: Some(force),
        source: "synthetic.spm".into(),
        import_warnings: vec![],
    });
    let source = data.resource_id;
    app.try_execute_action(Action::insert_dataset_with_default_canvas(
        &app,
        Dataset::Afm(Box::new(data)),
        "Synthetic Hertz".into(),
        DEFAULT_CANVAS_SIZE_MM,
    ))
    .unwrap();
    let target = app
        .afm_fit_target(source, AfmForceBranch::Approach)
        .unwrap();
    let mut draft = AfmHertzDraft::new(target);
    draft.radius_nm = "400".into();
    draft.contact_z_nm = "100".into();
    draft.baseline_z_nm = ["101".into(), "103".into()];
    draft.fit_z_nm = ["83".into(), "99".into()];
    app.session.ui.afm_hertz_task = Some(draft);
    app
}

#[test]
fn manual_hertz_checks_calibration_and_confirmation() {
    let mut app = approach(Some(1.), false);
    assert!(app.run_afm_hertz().unwrap_err().contains("relative Z"));
    let mut app = approach(None, true);
    assert!(app.run_afm_hertz().unwrap_err().contains("spring constant"));
    let mut app = approach(Some(1.), true);
    assert!(app.run_afm_hertz().unwrap_err().contains("Confirm"));
    assert!(app.afm_hertz_result().is_none());
}

#[test]
fn manual_hertz_rejects_peakforce_capture_even_with_usable_calibration() {
    let mut app = approach(Some(1.), true);
    let draft = app.session.ui.afm_hertz_task.as_mut().unwrap();
    draft.spring_confirmed = true;
    let target = draft.target;
    app.run_afm_hertz().unwrap();

    let Dataset::Afm(afm) = &mut app.doc.datasets[0] else {
        panic!("expected AFM");
    };
    Arc::make_mut(&mut afm.data).source = "synthetic.PFC".into();
    assert!(
        app.afm_hertz_calibration(target)
            .unwrap_err()
            .contains("PeakForce Capture")
    );
    assert!(
        app.run_afm_hertz()
            .unwrap_err()
            .contains("PeakForce Capture")
    );
    assert!(app.afm_hertz_result().is_none());
}

#[test]
fn manual_hertz_invalidates_result_and_saves_disclosed_fit_data() {
    let mut app = approach(Some(1.), true);
    app.session
        .ui
        .afm_hertz_task
        .as_mut()
        .unwrap()
        .spring_confirmed = true;
    app.run_afm_hertz().unwrap();
    let result = app.afm_hertz_result().unwrap();
    assert_eq!(result.rows.len(), 4);
    assert!((result.reduced_modulus_mpa - 0.5859375).abs() < 1e-10);
    assert!(result.residual_rms_nn < 1e-10);

    app.session.ui.afm_hertz_task.as_mut().unwrap().contact_z_nm = "99".into();
    assert!(app.afm_hertz_result().is_none());
    assert!(app.create_afm_hertz_output().is_err());
    app.session.ui.afm_hertz_task.as_mut().unwrap().contact_z_nm = "100".into();

    let Dataset::Afm(afm) = &mut app.doc.datasets[0] else {
        panic!("expected AFM");
    };
    afm.selected_pixel = [0, 0];
    assert!(app.afm_hertz_result().is_none());
    app.run_afm_hertz().unwrap();
    app.create_afm_hertz_output().unwrap();
    let table = app.doc.datasets[1].as_table().unwrap();
    assert_eq!(
        table.lineage.as_ref().unwrap().kind,
        DerivationKind::AfmHertzFitTable
    );
    let schema = &table.typed_state.envelope.revision.snapshot.schema;
    for (name, unit) in [
        ("Indentation", "nm"),
        ("Residual", "nN"),
        ("Contact Z", "nm"),
        ("Reduced modulus E*", "MPa"),
    ] {
        let column = schema
            .columns
            .iter()
            .find(|column| column.name == name)
            .unwrap();
        assert_eq!(column.unit.as_ref().unwrap().display_unit, unit);
    }

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("hertz.plotium");
    plotium_core::project::save_project(&app, &path, false).unwrap();
    let reopened = plotium_core::project::load_project(&path).unwrap();
    let saved = reopened.doc.datasets[1].as_table().unwrap();
    let lineage = saved.lineage.as_ref().unwrap();
    assert_eq!(lineage.kind, DerivationKind::AfmHertzFitTable);
    assert_eq!(
        lineage.sources,
        vec![reopened.doc.datasets[0].resource_id()]
    );
    assert!(reopened.afm_hertz_result().is_none());

    app.undo();
    assert_eq!(app.doc.datasets.len(), 1);
}

#[test]
fn hertz_method_switch_preserves_manual_inputs_and_exports_provenance() {
    use plotium_core::state::AfmHertzWindow;

    let mut app = approach(Some(1.), true);
    let draft = app.session.ui.afm_hertz_task.as_mut().unwrap();
    draft.spring_confirmed = true;
    let manual = draft.input(1.).unwrap();
    draft.linearized = true;
    draft.window = AfmHertzWindow::ForceNn;
    draft.fit_force_nn = ["0.01".into(), "1.01".into()];
    let linearized = draft.input(1.).unwrap();

    draft.linearized = false;
    draft.window = AfmHertzWindow::RelativeZ;
    assert_eq!(draft.input(1.).unwrap(), manual);

    draft.linearized = true;
    draft.window = AfmHertzWindow::ForceNn;
    assert_eq!(draft.input(1.).unwrap(), linearized);
    draft.contact_z_nm.clear();
    app.run_afm_hertz().unwrap();
    let result = app.afm_hertz_result().unwrap();
    assert_eq!(result.sample_indices, [0, 1, 2, 3]);
    assert!((result.reduced_modulus_mpa - 0.5859375).abs() < 1e-10);
    app.create_afm_hertz_output().unwrap();
    let table = app.doc.datasets[1].as_table().unwrap();
    let schema = &table.typed_state.envelope.revision.snapshot.schema;
    for name in [
        "Fit method",
        "Fit window",
        "Reference force",
        "Original sample index (zero-based)",
        "Fitted extrapolated zero",
        "Reduced modulus E*",
        "Residual",
        "Linearized slope",
    ] {
        assert!(
            schema.columns.iter().any(|column| column.name == name),
            "{name}"
        );
    }
}
