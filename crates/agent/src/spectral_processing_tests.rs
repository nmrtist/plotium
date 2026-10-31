use crate::session::{Failure, Session};
use plotium_core::state::*;
use plotium_io::XrdData;
use plotium_io::xps::{
    XpsEnergyKind, XpsExperiment, XpsMeasurement, XpsMeasurementId, XpsRegion, XpsRegionId,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const C1S: XpsRegionId = XpsRegionId(10);
const O1S: XpsRegionId = XpsRegionId(11);

fn xrd_dataset() -> Dataset {
    let intensity = (0..40)
        .map(|index| 5.0 + 100.0 * (-((f64::from(index) - 20.0) / 3.0).powi(2)).exp())
        .collect();
    Dataset::Xrd(Box::new(XrdDataset::load(XrdData {
        two_theta_deg: (0..40).map(|index| 20.0 + 0.1 * f64::from(index)).collect(),
        intensity,
        attenuation: None,
        source: "scan.rasx".to_owned(),
        instrument: None,
        target: None,
        wavelength_angstrom: None,
        voltage_kv: None,
        current_ma: None,
        scan_step_deg: Some(0.1),
        scan_speed_deg_min: None,
    })))
}

fn xps_dataset() -> Dataset {
    let measurement = XpsMeasurementId(1);
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
    Dataset::Xps(Box::new(XpsDataset::load(XpsExperiment {
        source: "regions.vms".into(),
        measurements: vec![XpsMeasurement {
            id: measurement,
            label: "Location 1".into(),
            position_mm: None,
            metadata: BTreeMap::new(),
        }],
        regions: vec![
            region(C1S, "C 1s", (283..=290).rev().map(f64::from).collect()),
            region(O1S, "O 1s", (531..=538).rev().map(f64::from).collect()),
        ],
        metadata: BTreeMap::new(),
        import_warnings: Vec::new(),
    })))
}

/// An arctangent edge with a slow oscillation, on the k grid autobk expects.
fn xas_dataset(step: f64) -> Dataset {
    let mut text = String::from("# Energy I0 It\n");
    for index in 0..900 {
        let energy = 6900.0 + 1100.0 * f64::from(index) / 899.0;
        let above = (energy - 7112.0).max(0.0);
        let k = (above / 3.81).sqrt();
        let oscillation = 0.1 * (4.0 * k).sin() * (-0.02 * k * k).exp();
        let edge = 0.5 + ((energy - 7112.0) / 2.0).atan() / std::f64::consts::PI;
        let mu = 0.2 + step * (edge + oscillation * f64::from(u8::from(above > 0.0)));
        text.push_str(&format!("{energy} 1000 {}\n", 1000.0 * (-mu).exp()));
    }
    Dataset::Xas(Box::new(XasDataset::load(
        plotium_io::xas::parse(&text).unwrap(),
    )))
}

fn app_with(datasets: Vec<Dataset>) -> (PlotiumApp, Session) {
    let mut app = PlotiumApp::new();
    app.doc.datasets.extend(datasets);
    let mut session = Session::default();
    session.set_allow_modify(true);
    (app, session)
}

fn call(
    session: &mut Session,
    app: &mut PlotiumApp,
    name: &str,
    mut arguments: Value,
) -> Result<Value, Failure> {
    arguments["expected_project_context"] = json!(app.session.project_context);
    arguments["expected_revision"] = json!(app.doc.automation_revision);
    session.call(app, name, arguments)
}

fn settle(app: &mut PlotiumApp) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.compute_busy() && std::time::Instant::now() < deadline {
        app.poll_compute();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    app.poll_compute();
    assert!(!app.compute_busy());
}

fn plan_edit(
    session: &mut Session,
    app: &mut PlotiumApp,
    index: usize,
    edits: Value,
) -> Result<Value, Failure> {
    let id = app.doc.datasets[index].resource_id().to_string();
    call(
        session,
        app,
        "plotium_plan",
        json!({"tool_id":"processing.edit","parameters":{"edits":edits},"ids":[id]}),
    )
}

fn edit(session: &mut Session, app: &mut PlotiumApp, index: usize, edits: Value) {
    let plan = plan_edit(session, app, index, edits).unwrap();
    session
        .call(app, "plotium_execute", json!({"plan_id":plan["plan_id"]}))
        .unwrap();
    settle(app);
}

fn inspect(session: &mut Session, app: &mut PlotiumApp, index: usize) -> Value {
    let id = app.doc.datasets[index].resource_id().to_string();
    call(
        session,
        app,
        "plotium_processing_inspect",
        json!({"ids":[id]}),
    )
    .unwrap()["value"]
        .clone()
}

fn xps_steps(session: &mut Session, app: &mut PlotiumApp) -> Value {
    inspect(session, app, 0)["axes"][0]["pipeline"]["steps"].clone()
}

#[test]
fn xrd_settings_are_inspected_edited_and_undone_through_the_connector() {
    let (mut app, mut session) = app_with(vec![xrd_dataset()]);
    let original = app.doc.datasets[0].as_xrd().unwrap().processed.clone();
    let settings = json!({
        "background":{"iterations":12},
        "smoothing":{"window":5,"polynomial_order":2},
        "normalization":"Maximum"
    });

    edit(
        &mut session,
        &mut app,
        0,
        json!([{"op":"set_xrd","processing":settings.clone()}]),
    );

    let dataset = app.doc.datasets[0].as_xrd().unwrap();
    assert_eq!(serde_json::to_value(dataset.params).unwrap(), settings);
    assert!(dataset.processed.background.is_some());
    assert_ne!(dataset.processed, original);
    let inspected = inspect(&mut session, &mut app, 0);
    assert_eq!(inspected["axes"][0]["axis"], "2theta");
    assert_eq!(
        inspected["axes"][0]["settings"]["background"]["iterations"],
        12
    );

    app.undo();
    assert_eq!(app.doc.datasets[0].as_xrd().unwrap().processed, original);
}

#[test]
fn invalid_xrd_settings_are_rejected_in_preflight() {
    let (mut app, mut session) = app_with(vec![xrd_dataset()]);
    let even_window = json!({
        "smoothing":{"window":4,"polynomial_order":2},
        "normalization":"None"
    });
    assert!(
        plan_edit(
            &mut session,
            &mut app,
            0,
            json!([{"op":"set_xrd","processing":even_window}]),
        )
        .is_err()
    );
    let partial = json!({"normalization":"Maximum"});
    let misspelled = json!({"background":null,"smoothin":null,"normalization":"None"});
    for settings in [partial, misspelled] {
        assert!(
            plan_edit(
                &mut session,
                &mut app,
                0,
                json!([{"op":"set_xrd","processing":settings}]),
            )
            .is_err()
        );
    }
    assert!(
        plan_edit(
            &mut session,
            &mut app,
            0,
            json!([{"op":"add","axis":"direct","kind":"Fft"}]),
        )
        .is_err()
    );
}

#[test]
fn a_paused_desktop_edit_blocks_connector_edits_until_resolved() {
    let (mut app, mut session) = app_with(vec![xrd_dataset()]);
    let id = app.doc.datasets[0].resource_id();
    let state = plotium_core::actions::DatasetProcessingState::from_dataset(&app.doc.datasets[0]);
    app.session.ui.proc_pending = Some((id, state));
    let settings = json!({"background":null,"smoothing":null,"normalization":"Area"});

    assert_eq!(
        inspect(&mut session, &mut app, 0)["axes"][0]["pending_apply"],
        true
    );
    assert!(
        plan_edit(
            &mut session,
            &mut app,
            0,
            json!([{"op":"set_xrd","processing":settings}]),
        )
        .is_err()
    );
}

#[test]
fn xps_steps_are_edited_per_region_with_stable_identities() {
    let (mut app, mut session) = app_with(vec![xps_dataset()]);
    let region = C1S.0.to_string();
    let inspected = inspect(&mut session, &mut app, 0);
    assert_eq!(inspected["axes"][0]["axis"], region);
    assert_eq!(inspected["axes"][0]["region"], "C 1s");
    assert_eq!(inspected["axes"][0]["pipeline"]["steps"], json!([]));

    edit(
        &mut session,
        &mut app,
        0,
        json!([
            {"op":"add","axis":region,"kind":{"Smooth":{"SavitzkyGolay":{"window":5,"poly_order":2}}}},
            {"op":"add","axis":region,"kind":{"Window":{"low_ev":284.0,"high_ev":289.0}}}
        ]),
    );
    let steps = xps_steps(&mut session, &mut app);
    let smooth = steps[0]["id"].as_u64().unwrap();
    let window = steps[1]["id"].as_u64().unwrap();

    edit(
        &mut session,
        &mut app,
        0,
        json!([
            {"op":"move","step":window,"before":smooth},
            {"op":"replace","step":smooth,"kind":{"Normalize":"MaxPeak"}},
            {"op":"enable","step":window,"enabled":false}
        ]),
    );

    let steps = xps_steps(&mut session, &mut app);
    assert!(steps[0]["kind"].get("Window").is_some());
    assert_eq!(steps[0]["enabled"], false);
    assert_eq!(steps[1]["id"], smooth);
    assert_eq!(steps[1]["kind"], json!({"Normalize":"MaxPeak"}));
    let other_region = &inspect(&mut session, &mut app, 0)["axes"][1];
    assert_eq!(other_region["pipeline"]["steps"], json!([]));
    let Dataset::Xps(xps) = &app.doc.datasets[0] else {
        panic!("fixture is XPS");
    };
    let processed = xps.displayed_region(C1S).unwrap().intensity;
    assert_eq!(processed.len(), 8);
    assert!((processed[3] - 1.0).abs() < 1e-12);

    edit(
        &mut session,
        &mut app,
        0,
        json!([{"op":"remove","step":window}]),
    );
    assert_eq!(
        xps_steps(&mut session, &mut app).as_array().unwrap().len(),
        1
    );
}

#[test]
fn invalid_xps_edits_are_rejected_in_preflight() {
    let (mut app, mut session) = app_with(vec![xps_dataset()]);
    let region = C1S.0.to_string();
    for edits in [
        json!([{"op":"add","axis":region,"kind":"Fft"}]),
        json!([{"op":"add","axis":"direct","kind":{"Normalize":"MaxPeak"}}]),
        json!([{"op":"add","axis":region,"kind":{"Window":{"low_ev":1.0,"high_ev":2.0}}}]),
        json!([{"op":"remove","step":999}]),
        json!([{"op":"set_xrd","processing":{"normalization":"None"}}]),
    ] {
        assert!(plan_edit(&mut session, &mut app, 0, edits).is_err());
    }
    assert_eq!(xps_steps(&mut session, &mut app), json!([]));
}

#[test]
fn saved_schemes_apply_to_datasets_of_the_same_kind() {
    let (mut app, mut session) = app_with(vec![xrd_dataset(), xrd_dataset(), xps_dataset()]);
    edit(
        &mut session,
        &mut app,
        0,
        json!([{"op":"set_xrd","processing":{"background":null,"smoothing":null,"normalization":"Area"}}]),
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("xrd.plotiumproc");
    plotium_core::project::save_scheme(&path, &app.doc.datasets[0]).unwrap();
    let ids: Vec<String> = app.doc.datasets[1..]
        .iter()
        .map(|dataset| dataset.resource_id().to_string())
        .collect();

    let plan = call(
        &mut session,
        &mut app,
        "plotium_plan",
        json!({"tool_id":"processing.apply_scheme","parameters":{"path":path,"compatible_only":true},"ids":ids}),
    )
    .unwrap();
    session
        .call(
            &mut app,
            "plotium_execute",
            json!({"plan_id":plan["plan_id"]}),
        )
        .unwrap();
    settle(&mut app);

    let applied = app.doc.datasets[1].as_xrd().unwrap().params;
    assert_eq!(
        serde_json::to_value(applied).unwrap()["normalization"],
        "Area"
    );
}

#[test]
fn xas_settings_edit_through_the_connector() {
    let (mut app, mut session) = app_with(vec![xas_dataset(1.0)]);
    let report = inspect(&mut session, &mut app, 0);
    let target = &report["axes"][0];
    assert_eq!(target["axis"], "energy");
    assert_eq!(target["calibration"]["available"], false);
    assert_eq!(target["columns"].as_array().unwrap().len(), 3);

    let mut settings = target["settings"].clone();
    settings["transform"]["kweight"] = json!(3);
    edit(
        &mut session,
        &mut app,
        0,
        json!([{"op":"set_xas","signal":target["signal"],"processing":settings}]),
    );
    let xas = app.doc.datasets[0].as_xas().unwrap();
    assert_eq!(xas.params.transform.kweight, 3);

    edit(
        &mut session,
        &mut app,
        0,
        json!([{"op":"apply_xas_preset","preset":"larch"}]),
    );
    let xas = app.doc.datasets[0].as_xas().unwrap();
    assert_eq!(xas.params.transform.kmin, 0.0);
    assert!(app.can_undo());
}

#[test]
fn xas_edits_reject_partial_settings_and_other_kinds() {
    let (mut app, mut session) = app_with(vec![xas_dataset(1.0), xrd_dataset()]);
    let partial = json!([{"op":"set_xas","signal":null,"processing":{"energy_shift":1.0}}]);
    assert!(plan_edit(&mut session, &mut app, 0, partial).is_err());
    let preset = json!([{"op":"apply_xas_preset","preset":"larch"}]);
    assert!(plan_edit(&mut session, &mut app, 1, preset).is_err());
    let current = serde_json::to_value(app.doc.datasets[0].as_xas().unwrap().params).unwrap();
    let mut misspelled = current.clone();
    misspelled["transform"]["k_weight"] = json!(3);
    let edit = json!([{"op":"set_xas","signal":app.doc.datasets[0].as_xas().unwrap().signal,"processing":misspelled}]);
    assert!(plan_edit(&mut session, &mut app, 0, edit).is_err());
    let cleared = json!([{"op":"set_xas","signal":null,"processing":current}]);
    assert!(plan_edit(&mut session, &mut app, 0, cleared).is_err());
    let invalid = {
        let xas = app.doc.datasets[0].as_xas().unwrap();
        let mut settings = serde_json::to_value(xas.params).unwrap();
        settings["transform"]["kweight"] = json!(1000);
        json!([{"op":"set_xas","signal":xas.signal,"processing":settings}])
    };
    assert!(plan_edit(&mut session, &mut app, 0, invalid).is_err());
}

#[test]
fn a_saved_xas_scheme_applies_without_moving_the_target_axis() {
    let (mut app, _) = app_with(vec![xas_dataset(1.0), xas_dataset(0.8)]);
    let mut source = app.doc.datasets[0].as_xas().unwrap().params;
    source.energy_shift = 2.0;
    source.transform.kweight = 3;
    let signal = app.doc.datasets[0].as_xas().unwrap().signal;
    app.doc.datasets[0]
        .as_xas_mut()
        .unwrap()
        .apply_processing(signal, source);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("xas.plotiumproc");
    plotium_core::project::save_scheme(&path, &app.doc.datasets[0]).unwrap();
    let scheme = plotium_core::project::load_scheme(&path).unwrap();
    assert_eq!(scheme.xas.as_ref().unwrap().energy_shift, 0.0);
    let state = plotium_core::project::apply_scheme(&scheme, &app.doc.datasets[1]).unwrap();
    let plotium_core::actions::DatasetProcessingState::Xas { processing, .. } = state else {
        panic!("expected an XAS state");
    };
    assert_eq!(processing.transform.kweight, 3);
    assert_eq!(processing.energy_shift, 0.0);
}
