//! Compares every XAS step with Larch 2026.3.1. The committed reference is a
//! synthetic spectrum; `beamline_scans_match_larch` repeats the comparison on
//! measured scans when `PLOTIUM_XAS_EXAMPLES` names the example directory.
use plotium_processing::xas::{
    self, AutobkParams, ChiR, PreEdge, PreEdgeParams, XafsWindow, XftfParams,
};
use std::path::Path;

fn table(text: &str) -> Vec<Vec<f64>> {
    text.lines()
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

fn header_value(text: &str, key: &str) -> f64 {
    let pattern = format!("{key}=");
    let start = text.find(&pattern).unwrap() + pattern.len();
    let value = text[start..].split_whitespace().next().unwrap();
    value.parse().unwrap()
}

/// Root-mean-square difference relative to the reference's own RMS.
fn relative_rms(values: &[f64], reference: &[f64]) -> f64 {
    assert_eq!(values.len(), reference.len());
    let difference: f64 = values
        .iter()
        .zip(reference)
        .map(|(value, reference)| (value - reference).powi(2))
        .sum();
    let scale: f64 = reference.iter().map(|value| value * value).sum();
    (difference / scale.max(f64::MIN_POSITIVE)).sqrt()
}

fn assert_rms(label: &str, values: &[f64], reference: &[f64], tolerance: f64) {
    let error = relative_rms(values, reference);
    assert!(error < tolerance, "{label}: relative RMS {error:.3e}");
}

fn pick(values: &[f64], rows: &[f64]) -> Vec<f64> {
    rows.iter().map(|row| values[*row as usize]).collect()
}

struct Case {
    pre_edge: PreEdgeParams,
    autobk: AutobkParams,
    xftf: XftfParams,
    norm: &'static str,
    chik: &'static str,
    chir: &'static str,
}

fn check_synthetic(case: &Case) {
    let spectrum = table(include_str!("fixtures/xas/synthetic_mu.tsv"));
    let (energy, mu) = xas::prepare_spectrum(&column(&spectrum, 0), &column(&spectrum, 1)).unwrap();
    let normalized = xas::pre_edge(&energy, &mu, &case.pre_edge).unwrap();
    assert!((normalized.e0 - header_value(case.norm, "e0")).abs() < 1e-9);
    assert!(
        (normalized.edge_step / header_value(case.norm, "edge_step") - 1.0).abs() < 1e-9,
        "edge step {}",
        normalized.edge_step
    );

    let reference = table(case.norm);
    let rows = column(&reference, 0);
    for (label, values, index) in [
        ("norm", &normalized.norm, 1),
        ("flat", &normalized.flat, 2),
        ("pre_edge", &normalized.pre_edge, 3),
        ("post_edge", &normalized.post_edge, 4),
    ] {
        assert_rms(
            label,
            &pick(values, &rows),
            &column(&reference, index),
            1e-9,
        );
    }

    let background = xas::autobk(
        &energy,
        &mu,
        normalized.e0,
        normalized.edge_step,
        &case.autobk,
    )
    .unwrap();
    assert_rms(
        "background",
        &pick(&background.background, &rows),
        &column(&reference, 5),
        1e-6,
    );
    let chik = table(case.chik);
    let k_rows: Vec<f64> = (0..chik.len()).map(|row| (row * 4) as f64).collect();
    assert_eq!(pick(&background.k, &k_rows).len(), chik.len());
    assert_rms(
        "chi",
        &pick(&background.chi, &k_rows),
        &column(&chik, 1),
        1e-4,
    );

    let chi_r = xas::xftf(&background.k, &background.chi, &case.xftf).unwrap();
    let chir = table(case.chir);
    let r_rows: Vec<f64> = (0..chir.len()).map(|row| (row * 3) as f64).collect();
    assert_rms("r", &pick(&chi_r.r, &r_rows), &column(&chir, 0), 1e-9);
    assert_rms(
        "magnitude",
        &pick(&chi_r.magnitude, &r_rows),
        &column(&chir, 1),
        1e-4,
    );
    assert_rms("re", &pick(&chi_r.re, &r_rows), &column(&chir, 2), 1e-4);
    assert_rms("im", &pick(&chi_r.im, &r_rows), &column(&chir, 3), 1e-4);
}

#[test]
fn athena_style_parameters_match_larch() {
    check_synthetic(&Case {
        pre_edge: PreEdgeParams {
            e0: Some(7121.3),
            pre1: Some(-150.0),
            pre2: Some(-30.0),
            norm1: Some(150.0),
            norm2: Some(740.0),
            nnorm: Some(2),
            ..PreEdgeParams::default()
        },
        autobk: AutobkParams {
            kmax: Some(14.8),
            kweight: 2,
            dk: 1.0,
            nclamp: 5,
            clamp_hi: 24.0,
            ..AutobkParams::default()
        },
        xftf: XftfParams {
            kmin: 3.0,
            kmax: 12.8,
            window: XafsWindow::Hanning,
            ..XftfParams::default()
        },
        norm: include_str!("fixtures/xas/athena_norm.tsv"),
        chik: include_str!("fixtures/xas/athena_chik.tsv"),
        chir: include_str!("fixtures/xas/athena_chir.tsv"),
    });
}

#[test]
fn default_parameters_match_larch() {
    check_synthetic(&Case {
        pre_edge: PreEdgeParams::default(),
        autobk: AutobkParams::default(),
        xftf: XftfParams {
            kmin: 2.0,
            kmax: 13.0,
            kweight: 3,
            dk: 2.0,
            dk2: 2.0,
            ..XftfParams::default()
        },
        norm: include_str!("fixtures/xas/defaults_norm.tsv"),
        chik: include_str!("fixtures/xas/defaults_chik.tsv"),
        chir: include_str!("fixtures/xas/defaults_chir.tsv"),
    });
}

#[test]
fn windows_match_larch() {
    let reference = table(include_str!("fixtures/xas/windows.tsv"));
    let x: Vec<f64> = (0..321).map(|index| 0.05 * index as f64).collect();
    let rows: Vec<f64> = (0..reference.len()).map(|row| (row * 5) as f64).collect();
    for (index, window, dx2) in [
        (1, XafsWindow::KaiserBessel, 1.5),
        (2, XafsWindow::Hanning, 1.0),
        (3, XafsWindow::Parzen, 1.5),
        (4, XafsWindow::Welch, 1.5),
        (5, XafsWindow::Gaussian, 1.5),
        (6, XafsWindow::Sine, 1.5),
    ] {
        let values = xas::ft_window(&x, 3.0, 12.0, 1.5, dx2, window);
        for (row, expected) in rows.iter().zip(column(&reference, index)) {
            let value = values[*row as usize];
            assert!(
                (value - expected).abs() < 1e-9,
                "{window:?} at {row}: {value} vs {expected}"
            );
        }
    }
}

fn number(value: &serde_json::Value, key: &str) -> f64 {
    value[key].as_f64().unwrap()
}

fn read_table(path: &Path) -> Vec<Vec<f64>> {
    table(&std::fs::read_to_string(path).unwrap())
}

fn compare_chi_r(label: &str, chi_r: &ChiR, reference: &[Vec<f64>], tolerance: f64) {
    assert_rms(label, &chi_r.magnitude, &column(reference, 1), tolerance);
    assert_rms(label, &chi_r.re, &column(reference, 2), tolerance);
    assert_rms(label, &chi_r.im, &column(reference, 3), tolerance);
}

/// Larch's own e0 search on the same spectra, for `find_e0`.
const LARCH_E0: [(&str, f64); 4] = [
    ("Fe_lepidocrocite.000", 7127.9967),
    ("Fe_pyrite.000", 7117.60017),
    ("hgcontrol.0008", 12292.36473199),
    ("hgcontrol.0013", 12291.47611881),
];

struct Agreement {
    edge_step: f64,
    chi: f64,
    chi_r: f64,
}

fn check_scan(directory: &Path, name: &str) -> Agreement {
    let normalized = read_table(&directory.join(format!("{name}_norm.txt")));
    let (energy, mu) = (column(&normalized, 0), column(&normalized, 3));
    let parameters: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(directory.join(format!("{name}_parameters.json"))).unwrap(),
    )
    .unwrap();
    let pre = &parameters["pre_edge"];
    let result: PreEdge = xas::pre_edge(
        &energy,
        &mu,
        &PreEdgeParams {
            e0: Some(number(pre, "e0")),
            pre1: Some(number(pre, "pre1")),
            pre2: Some(number(pre, "pre2")),
            norm1: Some(number(pre, "norm1")),
            norm2: Some(number(pre, "norm2")),
            nnorm: Some(number(pre, "nnorm") as usize),
            ..PreEdgeParams::default()
        },
    )
    .unwrap();
    assert_rms("norm", &result.norm, &column(&normalized, 1), 1e-9);
    assert_rms("flat", &result.flat, &column(&normalized, 2), 1e-9);
    assert_rms("pre_edge", &result.pre_edge, &column(&normalized, 4), 1e-9);

    let bkg = &parameters["autobk"];
    let background = xas::autobk(
        &energy,
        &mu,
        result.e0,
        result.edge_step,
        &AutobkParams {
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
    )
    .unwrap();
    let chik = read_table(&directory.join(format!("{name}_chik.txt")));
    assert_rms("k", &background.k, &column(&chik, 0), 1e-9);
    let chi_error = relative_rms(&background.chi, &column(&chik, 1));

    let fft = &parameters["fft"];
    let params = XftfParams {
        kmin: number(fft, "kmin"),
        kmax: number(fft, "kmax"),
        dk: number(fft, "dk"),
        dk2: number(fft, "dk2"),
        window: XafsWindow::Hanning,
        ..XftfParams::default()
    };
    let chi_r = xas::xftf(&background.k, &background.chi, &params).unwrap();
    let reference = read_table(&directory.join(format!("{name}_chiR.txt")));
    // Against the same chi(k), the transform alone has to agree exactly.
    let exact = xas::xftf(&column(&chik, 0), &column(&chik, 1), &params).unwrap();
    compare_chi_r(name, &exact, &reference, 1e-9);
    Agreement {
        edge_step: result.edge_step,
        chi: chi_error,
        chi_r: relative_rms(&chi_r.magnitude, &column(&reference, 1)),
    }
}

#[test]
#[ignore = "needs PLOTIUM_XAS_EXAMPLES"]
fn beamline_scans_match_larch() {
    let root = std::path::PathBuf::from(std::env::var_os("PLOTIUM_XAS_EXAMPLES").unwrap());
    let mut worst_chi: f64 = 0.0;
    let mut worst_chi_r: f64 = 0.0;
    let mut scans = 0;
    for beamline in ["01_NSLS_X23A2_Fe_K", "02_APS_20BM_Hg_L3"] {
        for kind in ["sample", "merged"] {
            let directory = root.join(beamline).join("exports_larch").join(kind);
            let mut names: Vec<String> = std::fs::read_dir(&directory)
                .unwrap()
                .filter_map(|entry| {
                    let file = entry.unwrap().file_name().into_string().unwrap();
                    file.strip_suffix("_parameters.json").map(str::to_owned)
                })
                .collect();
            names.sort();
            for name in names {
                let agreement = check_scan(&directory, &name);
                println!(
                    "{name}: step {:.6} chi {:.2e} chi(R) {:.2e}",
                    agreement.edge_step, agreement.chi, agreement.chi_r
                );
                worst_chi = worst_chi.max(agreement.chi);
                worst_chi_r = worst_chi_r.max(agreement.chi_r);
                scans += 1;
            }
        }
    }
    println!("{scans} scans, worst chi {worst_chi:.2e}, chi(R) {worst_chi_r:.2e}");
    assert_eq!(scans, 21);
    assert!(worst_chi < 1e-4, "chi {worst_chi:.3e}");
    assert!(worst_chi_r < 1e-4, "chi(R) {worst_chi_r:.3e}");

    for (name, expected) in LARCH_E0 {
        let beamline = if name.starts_with("Fe") {
            "01_NSLS_X23A2_Fe_K"
        } else {
            "02_APS_20BM_Hg_L3"
        };
        let path = root
            .join(beamline)
            .join(format!("exports_larch/sample/{name}_norm.txt"));
        let rows = read_table(&path);
        let e0 = xas::find_e0(&column(&rows, 0), &column(&rows, 3)).unwrap();
        println!("{name}: e0 {e0} vs {expected}");
        assert!((e0 - expected).abs() < 0.5, "{name}: {e0} vs {expected}");
    }
}
