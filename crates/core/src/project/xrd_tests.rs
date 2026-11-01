use super::{load_project, save_project};
use crate::state::{Dataset, PlotiumApp, XrdDataset};
use plotium_processing::xrd::{SnipBackground, XrdNormalization, XrdProcessing};

#[test]
fn data_and_processing_round_trip() {
    let mut app = PlotiumApp::new();
    let data = plotium_io::XrdData {
        two_theta_deg: vec![3.0, 3.1, 3.2],
        intensity: vec![10.0, 25.0, 12.0],
        attenuation: Some(vec![1.0, 1.0, 2.0]),
        source: "sample.rasx".to_owned(),
        instrument: Some("MiniFlex".to_owned()),
        target: Some("Cu".to_owned()),
        wavelength_angstrom: Some(1.540593),
        voltage_kv: Some(40.0),
        current_ma: Some(15.0),
        scan_step_deg: Some(0.1),
        scan_speed_deg_min: Some(5.0),
    };
    let mut dataset = XrdDataset::load(data);
    dataset.params = XrdProcessing {
        background: Some(SnipBackground { iterations: 1 }),
        smoothing: None,
        normalization: XrdNormalization::Maximum,
        ..XrdProcessing::default()
    };
    dataset.rebuild().unwrap();
    app.doc.datasets.push(Dataset::Xrd(Box::new(dataset)));

    let path = std::env::temp_dir().join(format!("plotium-xrd-{}.plotium", uuid::Uuid::new_v4()));
    save_project(&app, &path, false).unwrap();
    let loaded = load_project(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    let Dataset::Xrd(restored) = &loaded.doc.datasets[0] else {
        panic!("expected XRD")
    };
    assert_eq!(restored.data.instrument.as_deref(), Some("MiniFlex"));
    assert_eq!(
        restored.data.attenuation.as_deref(),
        Some(&[1.0, 1.0, 2.0][..])
    );
    assert_eq!(
        restored.params,
        XrdProcessing {
            background: Some(SnipBackground { iterations: 1 }),
            smoothing: None,
            normalization: XrdNormalization::Maximum,
            ..XrdProcessing::default()
        }
    );
    assert_eq!(restored.processed.intensity.len(), 3);
}
