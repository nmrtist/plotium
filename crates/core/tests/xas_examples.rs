//! Runs beamline scans through the dataset path. Set `PLOTIUM_XAS_EXAMPLES` to
//! the example directory and run with `--ignored`; each scan must reproduce the
//! Larch results stored under `exports_larch/sample/`, computed with the Athena
//! project parameters, and Larch's default e0 and edge step.
use plotium_core::state::{XasDataset, XasSignal};
use plotium_io::xas;
use plotium_processing::xas::{AutobkParams, PreEdgeParams, XafsWindow, XasProcessing, XftfParams};
use serde_json::Value;
use std::path::Path;

const TOLERANCE: f64 = 1e-4;
/// Larch 2026.3.1 with default parameters on the sorted scans: e0 and edge step.
const LARCH_DEFAULTS: [(&str, f64, f64); 19] = [
    ("Fe_lepidocrocite.000", 7128.497700, 0.4506463),
    ("Fe_lepidocrocite.100", 7128.497700, 0.4459986),
    ("Fe_lepidocrocite.200", 7128.497700, 0.4496405),
    ("Fe_pyrite.000", 7117.998170, 0.3189635),
    ("hgcontrol.0001", 12292.463475, 0.1620865),
    ("hgcontrol.0002", 12292.430560, 0.1618470),
    ("hgcontrol.0003", 12292.364732, 0.1625330),
    ("hgcontrol.0004", 12291.311575, 0.1626076),
    ("hgcontrol.0005", 12291.936865, 0.1623289),
    ("hgcontrol.0006", 12292.331818, 0.1626785),
    ("hgcontrol.0007", 12292.331818, 0.1622814),
    ("hgcontrol.0008", 12292.364732, 0.1621989),
    ("hgcontrol.0009", 12292.364732, 0.1619989),
    ("hgcontrol.0010", 12291.936865, 0.1616792),
    ("hgcontrol.0011", 12290.916688, 0.1620143),
    ("hgcontrol.0012", 12291.936865, 0.1612439),
    ("hgcontrol.0013", 12291.476119, 0.1614379),
    ("hgcontrol.0014", 12290.949595, 0.1617917),
    ("hgcontrol.0015", 12290.390213, 0.1614416),
];

fn read_table(path: &Path) -> Vec<Vec<f64>> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| {
            line.split_whitespace()
                .map(|value| value.parse().unwrap())
                .collect()
        })
        .collect()
}

fn column(rows: &[Vec<f64>], index: usize) -> Vec<f64> {
    rows.iter().map(|row| row[index]).collect()
}

fn number(object: &Value, key: &str) -> f64 {
    object[key].as_f64().unwrap()
}

fn relative_rms(values: &[f64], reference: &[f64]) -> f64 {
    assert_eq!(values.len(), reference.len());
    let difference: f64 = values
        .iter()
        .zip(reference)
        .map(|(value, reference)| (value - reference).powi(2))
        .sum();
    let scale: f64 = reference.iter().map(|value| value * value).sum();
    (difference / scale).sqrt()
}

fn processing(parameters: &Value) -> XasProcessing {
    let (pre, bkg, fft) = (
        &parameters["pre_edge"],
        &parameters["autobk"],
        &parameters["fft"],
    );
    XasProcessing {
        energy_shift: 0.0,
        pre_edge: PreEdgeParams {
            e0: Some(number(pre, "e0")),
            pre1: Some(number(pre, "pre1")),
            pre2: Some(number(pre, "pre2")),
            norm1: Some(number(pre, "norm1")),
            norm2: Some(number(pre, "norm2")),
            nnorm: Some(number(pre, "nnorm") as usize),
            ..PreEdgeParams::default()
        },
        autobk: AutobkParams {
            rbkg: number(bkg, "rbkg"),
            kmin: number(bkg, "kmin"),
            kmax: Some(number(bkg, "kmax")),
            kweight: number(bkg, "kweight") as i32,
            dk: number(bkg, "dk"),
            nclamp: number(bkg, "nclamp") as usize,
            clamp_lo: number(bkg, "clamp_lo"),
            clamp_hi: number(bkg, "clamp_hi"),
            ..AutobkParams::default()
        },
        transform: XftfParams {
            kmin: number(fft, "kmin"),
            kmax: number(fft, "kmax"),
            dk: number(fft, "dk"),
            dk2: number(fft, "dk2"),
            window: XafsWindow::Hanning,
            ..XftfParams::default()
        },
    }
}

fn check(directory: &Path, path: &Path, fluorescence: bool) {
    let name = path.file_name().unwrap().to_str().unwrap();
    let exports = directory.join("exports_larch/sample");
    let parameters: Value = serde_json::from_str(
        &std::fs::read_to_string(exports.join(format!("{name}_parameters.json"))).unwrap(),
    )
    .unwrap();
    let mut dataset = XasDataset::load(xas::load(path).unwrap());
    assert_eq!(
        matches!(dataset.signal, Some(XasSignal::Fluorescence { .. })),
        fluorescence,
        "{name}: {:?}",
        dataset.signal
    );
    let reference = read_table(&exports.join(format!("{name}_norm.txt")));
    let shift = reference[0][0] - reference[0][7];
    let mut params = processing(&parameters);
    params.energy_shift = shift;
    dataset.apply_processing(dataset.signal, params);
    let chik = read_table(&exports.join(format!("{name}_chik.txt")));
    let chir = read_table(&exports.join(format!("{name}_chiR.txt")));
    let exafs = dataset.processed.exafs.as_ref().unwrap();
    assert_eq!(dataset.processed.energy.len(), reference.len(), "{name}");
    let norm = &dataset.processed.edge.as_ref().unwrap().norm;
    let error = relative_rms(norm, &column(&reference, 1));
    assert!(error < TOLERANCE, "{name}: norm {error:.2e}");
    let error = relative_rms(&exafs.chi, &column(&chik, 1));
    assert!(error < TOLERANCE, "{name}: chi {error:.2e}");
    let error = relative_rms(&exafs.chi_r_magnitude, &column(&chir, 1));
    assert!(error < TOLERANCE, "{name}: chi(R) {error:.2e}");
    println!(
        "{name}: chi {:.1e}, {} points, e0 {:.2}, step {:.4}",
        relative_rms(&exafs.chi, &column(&chik, 1)),
        dataset.processed.energy.len(),
        dataset.processed.edge.as_ref().unwrap().e0,
        dataset.processed.edge.as_ref().unwrap().edge_step
    );
}

fn check_defaults(path: &Path) {
    let name = path.file_name().unwrap().to_str().unwrap();
    let (_, e0, step) = LARCH_DEFAULTS.iter().find(|row| row.0 == name).unwrap();
    let dataset = XasDataset::load(xas::load(path).unwrap());
    let edge = dataset.processed.edge.as_ref().unwrap();
    assert!(
        (edge.e0 - e0).abs() < 1e-3,
        "{name}: e0 {} vs {e0}",
        edge.e0
    );
    assert!(
        (edge.edge_step / step - 1.0).abs() < 1e-3,
        "{name}: step {}",
        edge.edge_step
    );
}

#[test]
#[ignore = "needs PLOTIUM_XAS_EXAMPLES"]
fn beamline_scans_match_larch_through_the_dataset() {
    let root = std::path::PathBuf::from(std::env::var_os("PLOTIUM_XAS_EXAMPLES").unwrap());
    let mut scans = 0;
    for (beamline, fluorescence) in [("01_NSLS_X23A2_Fe_K", false), ("02_APS_20BM_Hg_L3", true)] {
        let directory = root.join(beamline);
        let mut paths: Vec<_> = std::fs::read_dir(directory.join("raw"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        paths.sort();
        for path in paths {
            assert!(plotium_io::detect_format(&path).is_ok(), "{path:?}");
            check(&directory, &path, fluorescence);
            check_defaults(&path);
            scans += 1;
        }
    }
    assert_eq!(scans, 19);
}
