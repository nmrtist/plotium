use super::*;
use crate::state::{CanvasDocument, Dataset, ObjectFrame, XpsDataset};
use plotium_analysis::xps::{
    XpsCenterConstraint, XpsComponentId, XpsFwhmConstraint, XpsPeakSpec, fit_xps_peaks,
};
use plotium_io::Acquisition;
use plotium_io::xps::{
    ImportedXpsFit, ImportedXpsPeak, XpsEnergyKind, XpsExperiment, XpsMeasurement,
    XpsMeasurementId, XpsRegion, XpsRegionId,
};
use std::collections::BTreeMap;

fn two_region_experiment() -> (XpsExperiment, XpsMeasurementId, XpsRegionId, XpsRegionId) {
    let measurement = XpsMeasurementId(1);
    let c1s = XpsRegionId(10);
    let o1s = XpsRegionId(11);
    let region = |id, name: &str, energy: Vec<f64>| XpsRegion {
        id,
        measurement,
        name: name.into(),
        native_energy_kind: XpsEnergyKind::Binding,
        native_energy_ev: energy.clone(),
        binding_energy_ev: Some(energy),
        intensity_cps: vec![1.0, 2.0, 4.0, 8.0, 7.0, 4.0, 2.0, 1.0],
        counts: None,
        photon_energy_ev: Some(1486.69),
        dwell_time_s: None,
        sweeps: None,
        imported_fit: None,
        metadata: BTreeMap::new(),
    };
    (
        XpsExperiment {
            source: "multi-region.vms".into(),
            measurements: vec![XpsMeasurement {
                id: measurement,
                label: "Location 1".into(),
                position_mm: None,
                metadata: BTreeMap::new(),
            }],
            regions: vec![
                region(c1s, "C 1s", (283..=290).rev().map(f64::from).collect()),
                region(o1s, "O 1s", (531..=538).rev().map(f64::from).collect()),
            ],
            metadata: BTreeMap::new(),
            import_warnings: Vec::new(),
        },
        measurement,
        c1s,
        o1s,
    )
}

#[test]
fn processing_steps_are_region_specific_while_charge_shift_is_measurement_wide() {
    let (experiment, measurement, c1s, o1s) = two_region_experiment();
    let mut app = PlotiumApp::new();
    let mut xps = XpsDataset::load(experiment);
    let dataset = xps.resource_id;
    let background_before = xps.fit_workspaces[&c1s].invocation.background.clone();
    let workspace = xps.fit_workspaces.get_mut(&c1s).unwrap();
    workspace.invocation.peaks =
        xps_template("C 1s", &[1.0, 2.0], &mut workspace.next_component_id).unwrap();
    app.doc.datasets.push(Dataset::Xps(Box::new(xps)));

    app.add_xps_processing_step(
        dataset,
        c1s,
        XpsStepKind::Window {
            low_ev: 284.5,
            high_ev: 288.5,
        },
    )
    .unwrap();
    app.set_xps_energy_shift(dataset, measurement, 0.5).unwrap();

    let xps = app.doc.datasets[0].as_xps().unwrap();
    assert_eq!(xps.recipe(c1s).unwrap().steps.len(), 1);
    assert!(xps.recipe(o1s).unwrap().steps.is_empty());
    assert_eq!(
        xps.processed_region(c1s).unwrap().binding_energy_ev.len(),
        4
    );
    assert_eq!(
        xps.processed_region(o1s).unwrap().binding_energy_ev.len(),
        8
    );
    assert_eq!(
        xps.processed_region(c1s).unwrap().binding_energy_ev[0],
        288.5
    );
    assert_eq!(
        xps.processed_region(o1s).unwrap().binding_energy_ev[0],
        538.5
    );
    let XpsStepKind::Window { low_ev, high_ev } = xps.recipe(c1s).unwrap().steps[0].kind else {
        panic!("expected an XPS processing window")
    };
    assert_eq!([low_ev, high_ev], [285.0, 289.0]);
    let workspace = &xps.fit_workspaces[&c1s];
    assert_eq!(
        workspace.invocation.background.window_ev,
        background_before.window_ev.map(|value| value + 0.5)
    );
    assert!(matches!(
        workspace.invocation.peaks[0].center,
        XpsCenterConstraint::Free {
            initial_ev: 284.8,
            ..
        }
    ));

    app.undo();
    let xps = app.doc.datasets[0].as_xps().unwrap();
    assert_eq!(xps.energy_shift(measurement), Some(0.0));
    assert_eq!(xps.recipe(c1s).unwrap().steps.len(), 1);
    let XpsStepKind::Window { low_ev, high_ev } = xps.recipe(c1s).unwrap().steps[0].kind else {
        panic!("expected an XPS processing window")
    };
    assert_eq!([low_ev, high_ev], [284.5, 288.5]);
    assert_eq!(
        xps.fit_workspaces[&c1s].invocation.background,
        background_before
    );
    app.undo();
    assert!(
        app.doc.datasets[0]
            .as_xps()
            .unwrap()
            .recipe(c1s)
            .unwrap()
            .steps
            .is_empty()
    );
    app.redo();
    app.redo();
    assert_eq!(
        app.doc.datasets[0]
            .as_xps()
            .unwrap()
            .energy_shift(measurement),
        Some(0.5)
    );
}

#[test]
fn imported_fit_curves_are_hidden_after_processing() {
    let (mut experiment, _, c1s, _) = two_region_experiment();
    experiment.regions[0].imported_fit = Some(ImportedXpsFit {
        background_cps: vec![1.0; 8],
        envelope_cps: vec![1.0, 2.0, 4.0, 8.0, 7.0, 4.0, 2.0, 1.0],
        components_cps: vec![vec![0.0, 1.0, 3.0, 7.0, 6.0, 3.0, 1.0, 0.0]],
        peaks: vec![ImportedXpsPeak {
            label: "Imported C 1s".into(),
            position_ev: 284.8,
            fwhm_ev: 1.2,
            area: 10.0,
            lineshape: Some("GL(30)".into()),
        }],
    });
    let mut app = PlotiumApp::new();
    let xps = XpsDataset::load(experiment);
    let dataset = xps.resource_id;
    let field = xps.field_for_region(c1s).unwrap();
    app.doc.datasets.push(Dataset::Xps(Box::new(xps)));

    let figure = app.doc.datasets[0]
        .as_xps()
        .unwrap()
        .field_figure(field)
        .unwrap();
    assert!(
        figure
            .series
            .iter()
            .any(|series| series.name == "Imported envelope")
    );

    app.add_xps_processing_step(
        dataset,
        c1s,
        XpsStepKind::Normalize(plotium_processing::NormalizeMethod::MaxPeak),
    )
    .unwrap();
    let figure = app.doc.datasets[0]
        .as_xps()
        .unwrap()
        .field_figure(field)
        .unwrap();
    assert!(
        figure
            .series
            .iter()
            .all(|series| !series.name.starts_with("Imported"))
    );
}

#[test]
fn selecting_region_updates_selected_chart_field_and_undoes_atomically() {
    let (experiment, _, c1s, o1s) = two_region_experiment();
    let mut app = PlotiumApp::new();
    let xps = XpsDataset::load(experiment);
    let dataset = xps.resource_id;
    let c1s_field = xps.field_for_region(c1s).unwrap();
    let o1s_field = xps.field_for_region(o1s).unwrap();
    app.doc.datasets.push(Dataset::Xps(Box::new(xps)));
    let mut canvas = CanvasDocument::new("XPS".into(), [120.0, 80.0]);
    let [width, height] = canvas.size_pt();
    let object = canvas.allocate_object_id();
    canvas.objects.push(app.build_plot_object(
        0,
        ObjectFrame::new(0.0, 0.0, width, height),
        object,
        "XPS spectrum".into(),
    ));
    canvas.selected_object = Some(object);
    app.doc.canvases.push(canvas);
    app.session.active_canvas = Some(0);

    app.select_xps_region(dataset, o1s).unwrap();
    assert_eq!(app.doc.datasets[0].as_xps().unwrap().active_region, o1s);
    let plot = app.doc.canvases[0].object(object).unwrap().plot().unwrap();
    assert_eq!(plot.binding.series[0].source.field, o1s_field);
    assert!(
        plot.figure().x.min > 500.0,
        "the bound O 1s field must be rendered"
    );

    app.undo();
    assert_eq!(app.doc.datasets[0].as_xps().unwrap().active_region, c1s);
    assert_eq!(
        app.doc.canvases[0]
            .object(object)
            .unwrap()
            .plot()
            .unwrap()
            .binding
            .series[0]
            .source
            .field,
        c1s_field
    );
    app.redo();
    assert_eq!(app.doc.datasets[0].as_xps().unwrap().active_region, o1s);
    assert_eq!(
        app.doc.canvases[0]
            .object(object)
            .unwrap()
            .plot()
            .unwrap()
            .binding
            .series[0]
            .source
            .field,
        o1s_field
    );
}

#[test]
fn kinetic_only_region_rejects_an_empty_processing_window() {
    let measurement = XpsMeasurementId(1);
    let region = XpsRegionId(1);
    let experiment = XpsExperiment {
        source: "kinetic.vms".into(),
        measurements: vec![XpsMeasurement {
            id: measurement,
            label: "Location 1".into(),
            position_mm: None,
            metadata: BTreeMap::new(),
        }],
        regions: vec![XpsRegion {
            id: region,
            measurement,
            name: "Auger".into(),
            native_energy_kind: XpsEnergyKind::Kinetic,
            native_energy_ev: (93..=100).rev().map(f64::from).collect(),
            binding_energy_ev: None,
            intensity_cps: vec![1.0; 8],
            counts: None,
            photon_energy_ev: None,
            dwell_time_s: None,
            sweeps: None,
            imported_fit: None,
            metadata: BTreeMap::new(),
        }],
        metadata: BTreeMap::new(),
        import_warnings: Vec::new(),
    };
    let mut app = PlotiumApp::new();
    let xps = XpsDataset::load(experiment);
    let dataset = xps.resource_id;
    app.doc.datasets.push(Dataset::Xps(Box::new(xps)));

    let error = app
        .add_xps_processing_step(
            dataset,
            region,
            XpsStepKind::Window {
                low_ev: 200.0,
                high_ev: 201.0,
            },
        )
        .unwrap_err();
    assert!(error.contains("fewer than two points"));
    assert!(
        app.doc.datasets[0]
            .as_xps()
            .unwrap()
            .recipe(region)
            .unwrap()
            .steps
            .is_empty()
    );
}

#[test]
#[ignore = "requires PLOTIUM_XPS_REFERENCE_DIR"]
fn location_two_charge_shift_is_shared_by_all_regions() {
    let root = std::env::var_os("PLOTIUM_XPS_REFERENCE_DIR").expect("reference directory");
    let path = std::path::Path::new(&root).join("WBG250331.vms");
    let loaded = plotium_io::xps::load_vamas(&path).unwrap();
    let Acquisition::Xps(experiment) = loaded.acquisition else {
        panic!("expected XPS")
    };
    let measurement = experiment
        .measurements
        .iter()
        .find(|measurement| measurement.label.ends_with(": 2"))
        .unwrap()
        .id;
    let c1s = experiment
        .regions
        .iter()
        .find(|region| region.measurement == measurement && region.name == "C 1s")
        .unwrap();
    let shift = estimate_xps_charge_shift(
        c1s.binding_energy_ev.as_deref().unwrap(),
        &c1s.intensity_cps,
        284.8,
    )
    .unwrap();
    // VAMAS ordinate extrema are metadata, not the first two intensity points.
    // With the payload aligned to its regular ruler this reference is near +4.8 eV.
    assert!((shift - 4.80).abs() < 0.15, "shift={shift}");

    let raw = experiment
        .regions
        .iter()
        .filter(|region| region.measurement == measurement)
        .map(|region| (region.id, region.binding_energy_ev.as_ref().unwrap()[0]))
        .collect::<Vec<_>>();
    let mut app = PlotiumApp::new();
    let xps = XpsDataset::load(*experiment);
    let dataset = xps.resource_id;
    app.doc.datasets.push(Dataset::Xps(Box::new(xps)));
    app.set_xps_energy_shift(dataset, measurement, shift)
        .unwrap();
    let xps = app.doc.datasets[0].as_xps().unwrap();
    for (region, original) in raw {
        let processed = xps.processed_region(region).unwrap();
        assert!((processed.binding_energy_ev[0] - original - shift).abs() < 1e-10);
        assert_eq!(
            xps.region(region)
                .unwrap()
                .binding_energy_ev
                .as_ref()
                .unwrap()[0],
            original
        );
    }
}

#[test]
fn completed_fit_is_discarded_after_workspace_changes() {
    let measurement = XpsMeasurementId(1);
    let region = XpsRegionId(1);
    let energy = vec![290.0, 289.0, 288.0, 287.0, 286.0, 285.0, 284.0, 283.0];
    let intensity = vec![3.0, 4.0, 6.0, 12.0, 20.0, 60.0, 18.0, 5.0];
    let experiment = XpsExperiment {
        source: "memory.vms".into(),
        measurements: vec![XpsMeasurement {
            id: measurement,
            label: "Location 1".into(),
            position_mm: None,
            metadata: BTreeMap::new(),
        }],
        regions: vec![XpsRegion {
            id: region,
            measurement,
            name: "C 1s".into(),
            native_energy_kind: XpsEnergyKind::Binding,
            native_energy_ev: energy.clone(),
            binding_energy_ev: Some(energy),
            intensity_cps: intensity,
            counts: None,
            photon_energy_ev: Some(1486.69),
            dwell_time_s: None,
            sweeps: None,
            imported_fit: None,
            metadata: BTreeMap::new(),
        }],
        metadata: BTreeMap::new(),
        import_warnings: Vec::new(),
    };
    let mut xps = XpsDataset::load(experiment);
    let dataset = xps.resource_id;
    let processed = xps.processed_region(region).unwrap();
    let workspace = xps.fit_workspaces.get_mut(&region).unwrap();
    workspace.invocation.peaks.push(XpsPeakSpec::independent(
        XpsComponentId::new(1),
        "C 1s",
        285.0,
        50.0,
    ));
    workspace.next_component_id = 2;
    let invocation = workspace.invocation.clone();
    let input_sha256 = xps_input_sha256(
        region,
        &processed.binding_energy_ev,
        &processed.intensity,
        &invocation,
    );
    let result = fit_xps_peaks(
        &processed.binding_energy_ev,
        &processed.intensity,
        &invocation,
        &|| false,
    )
    .unwrap();
    xps.fit_workspaces
        .get_mut(&region)
        .unwrap()
        .invocation
        .peaks[0]
        .label = "Changed while fitting".into();
    let mut app = PlotiumApp::new();
    app.doc.datasets.push(Dataset::Xps(Box::new(xps)));
    let (_tx, rx) = std::sync::mpsc::channel();
    let job = XpsFitWorker {
        dataset,
        epoch: app.session.dataset_epoch,
        region,
        input_sha256,
        energy_shift_ev: app.doc.datasets[0].as_xps().unwrap().measurement_shifts[&measurement],
        processing_recipe: app.doc.datasets[0].as_xps().unwrap().region_recipes[&region].clone(),
        invocation,
        started_at: Instant::now(),
        cancel: Arc::new(AtomicBool::new(false)),
        rx,
    };
    app.finish_xps_fit(job, Ok(result));
    assert!(app.doc.datasets[0].as_xps().unwrap().fits.is_empty());
    assert!(app.session.status.contains("discarded"));

    let mut invalid = app.doc.datasets[0].as_xps().unwrap().fit_workspaces[&region].clone();
    invalid.invocation.peaks[0].fwhm = XpsFwhmConstraint::Fixed { value_ev: -1.0 };
    app.session.ui.proc_paused = true;
    assert!(app.set_xps_fit_workspace(dataset, region, invalid).is_err());
    assert!(matches!(
        app.doc.datasets[0].as_xps().unwrap().fit_workspaces[&region]
            .invocation
            .peaks[0]
            .fwhm,
        XpsFwhmConstraint::Free { .. }
    ));
}

#[test]
fn undone_processing_steps_keep_their_identities_reserved() {
    let (experiment, _, c1s, _) = two_region_experiment();
    let mut app = PlotiumApp::new();
    let xps = XpsDataset::load(experiment);
    let dataset = xps.resource_id;
    app.doc.datasets.push(Dataset::Xps(Box::new(xps)));
    let kind = XpsStepKind::Normalize(plotium_processing::NormalizeMethod::MaxPeak);

    let first = app
        .add_xps_processing_step(dataset, c1s, kind.clone())
        .unwrap();
    app.undo();
    let second = app.add_xps_processing_step(dataset, c1s, kind).unwrap();

    assert_ne!(first, second);
}
