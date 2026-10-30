use super::*;
use crate::state::{Dataset, XpsDataset, XrdDataset};
use plotium_io::xps::{
    XpsEnergyKind, XpsExperiment, XpsMeasurement, XpsMeasurementId, XpsRegion, XpsRegionId,
};
use plotium_processing::{NormalizeMethod, xrd};
use std::collections::BTreeMap;

fn xrd_dataset() -> Dataset {
    let mut dataset = XrdDataset::load(plotium_io::XrdData {
        two_theta_deg: vec![10.0, 10.1, 10.2, 10.3],
        intensity: vec![1.0, 4.0, 2.0, 1.0],
        attenuation: None,
        source: "scan.rasx".to_owned(),
        instrument: None,
        target: None,
        wavelength_angstrom: None,
        voltage_kv: None,
        current_ma: None,
        scan_step_deg: Some(0.1),
        scan_speed_deg_min: None,
    });
    dataset.params.normalization = xrd::XrdNormalization::Maximum;
    Dataset::Xrd(Box::new(dataset))
}

fn xps_dataset() -> Dataset {
    let energy: Vec<f64> = (283..=290).rev().map(f64::from).collect();
    let mut dataset = XpsDataset::load(XpsExperiment {
        source: "region.vms".into(),
        measurements: vec![XpsMeasurement {
            id: XpsMeasurementId(1),
            label: "Location 1".into(),
            position_mm: None,
            metadata: BTreeMap::new(),
        }],
        regions: vec![XpsRegion {
            id: XpsRegionId(10),
            measurement: XpsMeasurementId(1),
            name: "C 1s".into(),
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
        }],
        metadata: BTreeMap::new(),
        import_warnings: Vec::new(),
    });
    let id = StepId::new(dataset.next_step_id);
    dataset.next_step_id += 1;
    dataset
        .region_recipes
        .get_mut(&XpsRegionId(10))
        .unwrap()
        .steps
        .push(XpsProcessingStep {
            id,
            kind: XpsStepKind::Normalize(NormalizeMethod::MaxPeak),
            enabled: false,
            source: StepSource::Default,
        });
    Dataset::Xps(Box::new(dataset))
}

fn round_trip(dataset: &Dataset) -> ProcessingScheme {
    let scheme = scheme_from_dataset(dataset).unwrap();
    serde_json::from_slice(&serde_json::to_vec(&scheme).unwrap()).unwrap()
}

#[test]
fn xrd_scheme_round_trips_and_validates_its_limits() {
    let dataset = xrd_dataset();
    let mut scheme = round_trip(&dataset);
    assert!(matches!(
        apply_scheme(&scheme, &dataset).unwrap(),
        DatasetProcessingState::Xrd(settings)
            if settings.normalization == xrd::XrdNormalization::Maximum
    ));
    scheme.xrd = Some(XrdProcessing {
        background: Some(xrd::SnipBackground { iterations: 0 }),
        ..XrdProcessing::default()
    });
    assert!(apply_scheme(&scheme, &dataset).is_err());
}

#[test]
fn xps_scheme_remints_step_identities_on_the_active_region() {
    let dataset = xps_dataset();
    let next = dataset_next_step_id(&dataset);
    let scheme = round_trip(&dataset);
    let DatasetProcessingState::Xps {
        region_recipes,
        next_step_id,
        ..
    } = apply_scheme(&scheme, &dataset).unwrap()
    else {
        panic!("XPS scheme yields XPS state");
    };
    let steps = &region_recipes[&XpsRegionId(10)].steps;
    assert_eq!(steps[0].id.get(), next);
    assert_eq!(next_step_id, next + 1);
    assert!(!steps[0].enabled);
    assert_eq!(steps[0].source, StepSource::User);
}

#[test]
fn schemes_do_not_cross_dataset_kinds() {
    let xrd = xrd_dataset();
    let xps = xps_dataset();
    assert!(apply_scheme(&round_trip(&xrd), &xps).is_err());
    assert!(apply_scheme(&round_trip(&xps), &xrd).is_err());
}
