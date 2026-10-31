use super::*;
use crate::state::{Dataset, PlotiumApp};
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
fn a_new_scan_opens_with_the_athena_transform() {
    let dataset = dataset(&spectrum(900, 8000.0));
    let k_end = dataset.k_end().unwrap();
    let transform = &dataset.params.transform;
    assert_eq!(transform.kmin, 3.0);
    assert!(
        (transform.kmax - (k_end - 2.0)).abs() < 0.1,
        "{transform:?}"
    );
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

#[test]
fn summary_canvas_holds_three_pinned_panels_and_undoes_in_one_step() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    app.doc
        .datasets
        .push(Dataset::Xas(Box::new(dataset(&spectrum(900, 8000.0)))));
    app.create_xas_summary_canvas(0).unwrap();
    let canvas = app.doc.canvases.last().unwrap();
    let plots: Vec<_> = canvas.objects.iter().filter_map(|o| o.plot()).collect();
    assert_eq!(plots.len(), 3);
    assert!(plots.iter().all(|plot| plot.display_owner.is_none()));
    let fields: std::collections::BTreeSet<_> = plots
        .iter()
        .map(|plot| plot.binding.series[0].source.field)
        .collect();
    assert_eq!(fields.len(), 3);
    let canvases = app.doc.canvases.len();
    app.undo();
    assert_eq!(app.doc.canvases.len(), canvases - 1);
}

#[test]
fn a_xanes_scan_cannot_build_the_summary() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    app.doc
        .datasets
        .push(Dataset::Xas(Box::new(dataset(&spectrum(300, 7160.0)))));
    assert!(app.create_xas_summary_canvas(0).is_err());
}

#[test]
fn edge_lookup_ignores_l1_and_l2() {
    let edge = processing::nearest_edge(7128.5).unwrap();
    assert_eq!((edge.0, edge.1), ("Fe", "K"));
}

#[test]
fn a_processing_edit_redraws_the_summary_panels() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    app.doc
        .datasets
        .push(Dataset::Xas(Box::new(dataset(&spectrum(900, 8000.0)))));
    app.create_xas_summary_canvas(0).unwrap();
    let before = crate::actions::DatasetProcessingState::from_dataset(&app.doc.datasets[0]);
    let xas = app.doc.datasets[0].as_xas().unwrap();
    let (signal, mut params) = (xas.signal, xas.params);
    params.transform.kweight = 3;
    app.commit_processing_edit(
        0,
        before,
        crate::actions::DatasetProcessingState::Xas {
            signal,
            processing: Box::new(params),
        },
    );
    let canvas = app.doc.canvases.last().unwrap();
    let plot = canvas
        .objects
        .iter()
        .filter_map(|o| o.plot())
        .nth(1)
        .unwrap();
    let label = &plot.figure().y.label;
    assert!(label.contains('³'), "{label}");
}

#[test]
fn export_lists_each_stage_with_its_own_length() {
    let dataset = dataset(&spectrum(900, 8000.0));
    let columns = dataset.export_columns();
    let names: Vec<_> = columns.iter().map(|(name, _)| *name).collect();
    assert_eq!(names[..2], ["energy_ev", "mu"]);
    assert!(names.contains(&"chi_r_magnitude"));
    assert_eq!(columns[0].1.len(), 900);
    let k = columns.iter().find(|(name, _)| *name == "k_per_angstrom");
    assert!(k.unwrap().1.len() != 900);
}

fn with_reference(rows: &[(f64, f64)], shift: f64) -> XasDataset {
    let mut text = String::from("# Energy I0 It Ir\n");
    for (index, (energy, mu)) in rows.iter().enumerate() {
        let it = 1000.0 * (-mu).exp();
        let foil = rows[index].1;
        let ir = it * (-foil).exp();
        text.push_str(&format!("{} 1000 {it} {ir}\n", energy + shift));
    }
    XasDataset::load(parse(&text).unwrap())
}

#[test]
fn calibration_needs_a_reference() {
    let dataset = dataset(&spectrum(900, 8000.0));
    assert_eq!(dataset.calibration(), Err(XasCalibrationError::NoReference));
}

#[test]
fn calibration_settles_after_one_click_even_with_a_fixed_e0() {
    let rows: Vec<_> = (0..1500)
        .map(|index| {
            let energy = 6900.0 + index as f64 * 0.5;
            (
                energy,
                0.2 + 0.5 + ((energy - 7112.0) / 2.0).atan() / std::f64::consts::PI,
            )
        })
        .collect();
    let mut dataset = with_reference(&rows, 5.0);
    let mut params = dataset.params;
    params.pre_edge.e0 = Some(7117.0);
    dataset.apply_processing(dataset.signal, params);
    let first = dataset.calibration().unwrap();
    assert_eq!((first.element, first.edge), ("Fe", "K"));
    assert!((first.shift + 5.0).abs() < 1.0, "{first:?}");
    let params = dataset.params.with_energy_shift(first.shift);
    assert_eq!(params.pre_edge.e0, Some(7117.0 + first.shift));
    dataset.apply_processing(dataset.signal, params);
    let second = dataset.calibration().unwrap();
    assert!((second.shift - first.shift).abs() < 1e-6, "{second:?}");
}

#[test]
fn a_reference_without_an_edge_refuses_to_calibrate() {
    let mut text = String::from("# Energy I0 It Ir\n");
    for (energy, mu) in spectrum(1500, 8000.0) {
        let it = 1000.0 * (-mu).exp();
        text.push_str(&format!("{energy} 1000 {it} {}\n", it * 0.6));
    }
    let dataset = XasDataset::load(parse(&text).unwrap());
    assert_eq!(
        dataset.calibration(),
        Err(XasCalibrationError::NoReferenceEdge)
    );
}

#[test]
fn a_coarse_reference_refuses_to_calibrate() {
    let dataset = with_reference(&spectrum(120, 8000.0), 0.0);
    assert!(matches!(
        dataset.calibration(),
        Err(XasCalibrationError::CoarseReference(_))
    ));
}

#[test]
fn a_transform_past_the_data_says_so() {
    let mut dataset = dataset(&spectrum(900, 8000.0));
    let mut params = dataset.params;
    params.transform.kmin = 18.0;
    dataset.apply_processing(dataset.signal, params);
    assert!(
        dataset
            .issues()
            .iter()
            .any(|issue| issue.contains("past the end"))
    );
}

#[test]
fn an_e0_outside_the_scan_says_so() {
    let mut dataset = dataset(&spectrum(900, 8000.0));
    let mut params = dataset.params;
    params.pre_edge.e0 = Some(9000.0);
    dataset.apply_processing(dataset.signal, params);
    assert!(
        dataset
            .issues()
            .iter()
            .any(|issue| issue.contains("outside the scan"))
    );
}

#[test]
fn reset_returns_to_the_detected_signal_and_defaults() {
    let mut dataset = dataset(&spectrum(900, 8000.0));
    let (signal, defaults) = (dataset.signal, dataset.params);
    let mut params = dataset.params;
    params.energy_shift = 2.0;
    dataset.apply_processing(signal, params);
    let reset = crate::project::reset_processing(&Dataset::Xas(Box::new(dataset))).unwrap();
    assert_eq!(
        reset,
        crate::actions::DatasetProcessingState::Xas {
            signal,
            processing: Box::new(defaults),
        }
    );
}

#[test]
fn a_drag_session_is_one_undo_step() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    app.doc
        .datasets
        .push(Dataset::Xas(Box::new(dataset(&spectrum(900, 8000.0)))));
    let start = app.doc.datasets[0].as_xas().unwrap().params;
    app.begin_processing_session(0);
    for step in 1..=4 {
        let before = crate::actions::DatasetProcessingState::from_dataset(&app.doc.datasets[0]);
        let xas = app.doc.datasets[0].as_xas().unwrap();
        let signal = xas.signal;
        let mut params = xas.params;
        params.energy_shift = f64::from(step);
        app.commit_processing_edit(
            0,
            before,
            crate::actions::DatasetProcessingState::Xas {
                signal,
                processing: Box::new(params),
            },
        );
    }
    app.finish_processing_session();
    assert_eq!(
        app.doc.datasets[0].as_xas().unwrap().params.energy_shift,
        4.0
    );
    app.undo();
    assert_eq!(app.doc.datasets[0].as_xas().unwrap().params, start);
}

#[test]
fn short_scans_keep_a_valid_post_edge_range() {
    for above in [40.0, 200.0, 300.0] {
        let dataset = dataset(&spectrum(900, 7112.0 + above));
        dataset.params.validate().unwrap();
        let step = dataset.processed.edge.as_ref().unwrap().edge_step;
        assert!(
            (step - 1.0).abs() < 0.2,
            "{above} eV above the edge: {step}"
        );
    }
}

#[test]
fn checks_flag_a_short_post_edge_and_stay_quiet_on_a_full_scan() {
    assert_eq!(dataset(&spectrum(900, 8000.0)).checks(), Vec::new());
    let short = dataset(&spectrum(300, 7160.0));
    let codes: Vec<_> = short.checks().into_iter().map(|check| check.code).collect();
    assert_eq!(codes, ["short_post_edge"]);
}

#[test]
fn checks_flag_noise_at_high_k() {
    let rows: Vec<_> = spectrum(900, 8000.0)
        .into_iter()
        .enumerate()
        .map(|(index, (energy, mu))| {
            let sign = if index % 2 == 0 { 1.0 } else { -1.0 };
            (energy, mu + sign * 0.02)
        })
        .collect();
    let codes: Vec<_> = dataset(&rows)
        .checks()
        .into_iter()
        .map(|check| check.code)
        .collect();
    assert!(codes.contains(&"noisy_high_k"), "{codes:?}");
}

#[test]
fn e0_is_checked_against_the_edge_the_file_declares() {
    let mut text = String::from(
        "# XDI/1.0\n# Element.symbol: Cu\n# Element.edge: K\n# Column.1: energy eV\n\
         # Column.2: i0\n# Column.3: itrans\n#---\n",
    );
    for (energy, mu) in spectrum(900, 8000.0) {
        text.push_str(&format!("{energy} 1000 {}\n", 1000.0 * (-mu).exp()));
    }
    let codes: Vec<_> = XasDataset::load(parse(&text).unwrap())
        .checks()
        .into_iter()
        .map(|check| check.code)
        .collect();
    assert!(codes.contains(&"e0_far_from_table"), "{codes:?}");
}

#[test]
fn a_scheme_keeps_the_short_scan_ranges() {
    let long = dataset(&spectrum(1100, 8000.0));
    let short = dataset(&spectrum(600, 7312.0));
    let adopted = short.adopt_scheme(long.params);
    adopted.validate().unwrap();
    assert_eq!(adopted.pre_edge.norm2, short.params.pre_edge.norm2);
    assert_eq!(adopted.transform.kmax, short.params.transform.kmax);
    let same = long.adopt_scheme(long.params);
    assert_eq!(same, long.params);
}

#[test]
fn defaults_and_presets_pass_validation() {
    let dataset = dataset(&spectrum(900, 8000.0));
    dataset.params.validate().unwrap();
    for preset in plotium_processing::xas::XasPreset::ALL {
        preset
            .apply(&dataset.params, dataset.k_end())
            .validate()
            .unwrap();
    }
}

#[test]
fn a_post_edge_past_the_scan_says_so() {
    let mut dataset = dataset(&spectrum(900, 8000.0));
    let mut params = dataset.params;
    params.pre_edge.norm1 = Some(5000.0);
    params.pre_edge.norm2 = Some(6000.0);
    dataset.apply_processing(dataset.signal, params);
    assert!(
        dataset
            .issues()
            .iter()
            .any(|issue| issue.contains("post-edge"))
    );
}
