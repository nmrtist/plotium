//! Reads beamline scans outside the repository. Set `PLOTIUM_XAS_EXAMPLES` to
//! the example directory and run with `--ignored`; every `raw/` scan must give
//! the mu(E) stored next to it under `exports_larch/sample/`.
use plotium_io::xas;
use std::path::{Path, PathBuf};

const ENERGY_COLUMN: usize = 7;
const MU_COLUMN: usize = 3;

struct Beamline {
    directory: &'static str,
    mu: fn(&xas::XasScan) -> Vec<f64>,
}

fn transmission(scan: &xas::XasScan) -> Vec<f64> {
    let i0 = scan.column(scan.channels.i0.unwrap()).unwrap();
    let it = scan.column(scan.channels.transmission.unwrap()).unwrap();
    i0.iter().zip(it).map(|(i0, it)| (i0 / it).ln()).collect()
}

fn fluorescence(scan: &xas::XasScan) -> Vec<f64> {
    let i0 = scan.column(scan.channels.i0.unwrap()).unwrap();
    let signal = scan.column(scan.channels.fluorescence[0]).unwrap();
    signal
        .iter()
        .zip(i0)
        .map(|(signal, i0)| signal / i0)
        .collect()
}

fn exported_mu(path: &Path) -> Vec<(f64, f64)> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| {
            let row: Vec<f64> = line.split('\t').map(|v| v.parse().unwrap()).collect();
            (row[ENERGY_COLUMN], row[MU_COLUMN])
        })
        .collect()
}

fn check(root: &Path, beamline: &Beamline) -> usize {
    let directory = root.join(beamline.directory);
    let mut scans: Vec<PathBuf> = std::fs::read_dir(directory.join("raw"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    scans.sort();
    for path in &scans {
        let name = path.file_name().unwrap().to_str().unwrap();
        let scan = xas::load(path).unwrap();
        assert!(scan.warnings.is_empty(), "{name}: {:?}", scan.warnings);
        let mu = (beamline.mu)(&scan);
        let energy = scan.energy();
        // The authors deleted a few glitch points from the Hg project, so
        // match rows by energy rather than position.
        for (expected_energy, expected_mu) in
            exported_mu(&directory.join(format!("exports_larch/sample/{name}_norm.txt")))
        {
            let row = energy
                .iter()
                .position(|value| (value - expected_energy).abs() < 1e-6)
                .unwrap_or_else(|| panic!("{name}: no row at {expected_energy}"));
            assert!(
                (mu[row] - expected_mu).abs() < 1e-9,
                "{name} at {expected_energy}: {} vs {expected_mu}",
                mu[row]
            );
        }
    }
    scans.len()
}

#[test]
#[ignore = "needs PLOTIUM_XAS_EXAMPLES"]
fn beamline_scans_match_project_mu() {
    let root = PathBuf::from(std::env::var_os("PLOTIUM_XAS_EXAMPLES").unwrap());
    let fe = Beamline {
        directory: "01_NSLS_X23A2_Fe_K",
        mu: transmission,
    };
    let hg = Beamline {
        directory: "02_APS_20BM_Hg_L3",
        mu: fluorescence,
    };
    assert_eq!(check(&root, &fe), 4);
    assert_eq!(check(&root, &hg), 15);
}
