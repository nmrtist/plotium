use super::*;
use plotium_io::xas::parse;

/// An arctangent edge at 7112 eV with a decaying oscillation above it.
fn spectrum(points: usize, end: f64) -> Vec<(f64, f64)> {
    let start = 6900.0;
    (0..points)
        .map(|index| {
            let energy = start + (end - start) * index as f64 / (points - 1) as f64;
            let above = (energy - 7112.0).max(0.0);
            let k = (above / 3.81).sqrt();
            let chi = 0.1 * (2.0 * 2.0 * k).sin() * (-0.02 * k * k).exp();
            let mu = 0.2
                + 1.0 * (0.5 + ((energy - 7112.0) / 2.0).atan() / std::f64::consts::PI)
                + chi * f64::from(above > 0.0);
            (energy, mu)
        })
        .collect()
}

fn scan_text(rows: &[(f64, f64)], header: &str, fluorescence: bool) -> String {
    let mut text = format!("# {header}\n");
    for (energy, mu) in rows {
        let i0 = 1000.0;
        if fluorescence {
            // Weak transmission: the file records it, the sample shows no edge.
            let it = i0 * (-0.5 - 0.001 * mu).exp();
            let signal = i0 * mu;
            text.push_str(&format!("{energy} {i0} {it} {signal}\n"));
        } else {
            let it = i0 * (-mu).exp();
            text.push_str(&format!("{energy} {i0} {it}\n"));
        }
    }
    text
}

fn dataset(rows: &[(f64, f64)]) -> XasDataset {
    XasDataset::load(parse(&scan_text(rows, "Energy I0 It", false)).unwrap())
}

#[test]
fn full_scan_exposes_every_stage_in_order() {
    let dataset = dataset(&spectrum(900, 8000.0));
    assert!(matches!(
        dataset.signal,
        Some(XasSignal::Transmission { .. })
    ));
    assert!(dataset.issues().is_empty(), "{:?}", dataset.issues());
    let keys: Vec<_> = dataset
        .field_descriptors()
        .into_iter()
        .map(|field| field.local_id)
        .collect();
    assert_eq!(
        keys,
        ["xas.norm", "xas.mu", "xas.flat", "xas.chik", "xas.chir"]
    );
    let edge = dataset.processed.edge.as_ref().unwrap();
    assert!((edge.e0 - 7112.0).abs() < 2.0, "e0 = {}", edge.e0);
}

#[test]
fn xanes_scan_stops_before_exafs_and_says_why() {
    let dataset = dataset(&spectrum(300, 7160.0));
    assert!(dataset.processed.edge.is_some());
    assert!(dataset.processed.exafs.is_none());
    assert!(dataset.issues()[0].contains("too short for EXAFS"));
    assert!(
        dataset
            .field_id(CHI_K)
            .is_some_and(|id| dataset.curve(id).is_none())
    );
}

#[test]
fn a_flat_transmission_channel_gives_way_to_fluorescence() {
    let text = scan_text(&spectrum(900, 8000.0), "Energy I0 It If", true);
    let dataset = XasDataset::load(parse(&text).unwrap());
    assert!(matches!(
        dataset.signal,
        Some(XasSignal::Fluorescence { .. })
    ));
}

#[test]
fn energy_shift_moves_the_edge() {
    let mut dataset = dataset(&spectrum(900, 8000.0));
    let before = dataset.processed.edge.as_ref().unwrap().e0;
    let mut params = dataset.params;
    params.energy_shift = 3.0;
    dataset.apply_processing(dataset.signal, params);
    let after = dataset.processed.edge.as_ref().unwrap().e0;
    assert!((after - before - 3.0).abs() < 1e-6);
}

#[test]
fn an_absorption_column_beats_a_flat_transmission_channel() {
    let mut text = String::from("# Energy I0 It mu\n");
    for (energy, mu) in spectrum(900, 8000.0) {
        let it = 1000.0 * (-0.5 - 0.001 * mu).exp();
        text.push_str(&format!("{energy} 1000 {it} {mu}\n"));
    }
    let dataset = XasDataset::load(parse(&text).unwrap());
    assert!(matches!(dataset.signal, Some(XasSignal::Absorption { .. })));
}

#[test]
fn a_file_without_a_signal_says_so_instead_of_guessing() {
    let text = "# XDI/1.0\n# Column.1: energy eV\n# Column.2: time\n#---\n1 2\n2 2\n3 2\n4 2\n";
    let dataset = XasDataset::load(parse(text).unwrap());
    assert_eq!(dataset.signal, None);
    assert!(!dataset.has_spectrum());
    assert!(dataset.field_descriptors().is_empty());
    assert!(dataset.issues()[0].contains("no signal"));
}

#[test]
fn a_dead_monitor_opens_empty_with_a_reason() {
    let mut text = String::from("# Energy I0 It\n");
    for (energy, _) in spectrum(50, 7200.0) {
        text.push_str(&format!("{energy} 0 0\n"));
    }
    let dataset = XasDataset::load(parse(&text).unwrap());
    assert!(!dataset.has_spectrum());
    assert!(!dataset.issues().is_empty());
}

#[test]
fn repeated_energies_are_reported() {
    let mut rows = spectrum(900, 8000.0);
    rows.insert(100, rows[100]);
    let dataset = dataset(&rows);
    assert!(dataset.issues()[0].contains("Dropped 1 row with"));
    assert_eq!(dataset.processed.energy.len(), 900);
}
