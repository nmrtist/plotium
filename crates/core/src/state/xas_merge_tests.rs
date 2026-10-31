use super::*;
use plotium_io::xas::parse;

fn scan(shift: f64, noise: f64, foil: bool) -> XasDataset {
    let mut text = String::from(if foil {
        "# Energy I0 It Ir\n"
    } else {
        "# Energy I0 It\n"
    });
    for index in 0..1500 {
        let energy = 6900.0 + index as f64 * 0.5;
        let edge = |at: f64| 0.5 + ((energy - at) / 2.0).atan() / std::f64::consts::PI;
        let wobble = if index % 2 == 0 { noise } else { -noise };
        let mu = 0.2 + edge(7120.0 + shift) + wobble;
        let it = 1000.0 * (-mu).exp();
        let ir = it * (-(0.2 + edge(7112.0 + shift))).exp();
        text.push_str(&if foil {
            format!("{energy} 1000 {it} {ir}\n")
        } else {
            format!("{energy} 1000 {it}\n")
        });
    }
    XasDataset::load(parse(&text).unwrap())
}

#[test]
fn averaging_cancels_opposite_noise() {
    let (up, down) = (scan(0.0, 0.01, false), scan(0.0, -0.01, false));
    let (merged, alignment) = merge_scans(&[&up, &down], "merge").unwrap();
    assert!(matches!(alignment, XasMergeAlignment::Unaligned(_)));
    let clean = scan(0.0, 0.0, false);
    let error = merged.columns[1]
        .values
        .iter()
        .zip(&clean.processed.mu)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    assert!(error < 1e-6, "{error}");
}

#[test]
fn reference_foils_align_the_scans() {
    let (first, second) = (scan(0.0, 0.0, true), scan(1.5, 0.0, true));
    let (merged, alignment) = merge_scans(&[&first, &second], "merge").unwrap();
    assert_eq!(alignment, XasMergeAlignment::Aligned);
    let merged = XasDataset::load(merged);
    let e0 = merged.edge_e0().unwrap();
    assert!((e0 - first.edge_e0().unwrap()).abs() < 0.3, "{e0}");
}

#[test]
fn a_single_scan_or_mixed_signals_are_refused() {
    let one = scan(0.0, 0.0, false);
    assert!(merge_scans(&[&one], "merge").is_err());
    let mut text = String::from("# Energy mu\n");
    for index in 0..300 {
        text.push_str(&format!(
            "{} {}\n",
            7000.0 + f64::from(index),
            f64::from(index)
        ));
    }
    let other = XasDataset::load(parse(&text).unwrap());
    assert!(merge_scans(&[&one, &other], "merge").is_err());
}

#[test]
fn merging_adds_a_dataset_and_a_canvas_in_one_undo_step() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    for shift in [0.0, 0.0] {
        app.doc
            .datasets
            .push(Dataset::Xas(Box::new(scan(shift, 0.0, false))));
    }
    app.session.ui.data_selection = vec![0, 1];
    assert!(app.merge_selected_xas());
    assert_eq!(app.doc.datasets.len(), 3);
    let merged = app.doc.datasets[2].as_xas().unwrap();
    assert!(matches!(merged.signal, Some(XasSignal::Absorption { .. })));
    assert_eq!(merged.lineage.as_ref().unwrap().sources.len(), 2);
    app.undo();
    assert_eq!(app.doc.datasets.len(), 2);
}

#[test]
fn a_stack_of_scans_is_labeled_by_dataset() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    for name in ["first.dat", "second.dat"] {
        let mut dataset = scan(0.0, 0.0, false);
        dataset.scan.source = name.to_owned();
        app.doc.datasets.push(Dataset::Xas(Box::new(dataset)));
    }
    app.session.ui.data_selection = vec![0, 1];
    app.stack_selected_data();
    let canvas = app.doc.canvases.last().unwrap();
    let plot = canvas.objects.iter().find_map(|o| o.plot()).unwrap();
    let names: Vec<_> = plot
        .figure()
        .series
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    assert_eq!(names, ["first.dat", "second.dat"]);
}

#[test]
fn scans_of_different_edges_are_refused() {
    let (near, far) = (scan(0.0, 0.0, true), scan(40.0, 0.0, true));
    assert!(merge_scans(&[&near, &far], "merge").is_err());
    let (near, far) = (scan(0.0, 0.0, false), scan(40.0, 0.0, false));
    assert!(merge_scans(&[&near, &far], "merge").is_err());
}

#[test]
fn only_the_shared_energies_are_averaged() {
    let mut text = String::from("# Energy I0 It\n");
    for index in 400..1500 {
        let energy = 6900.0 + f64::from(index) * 0.5;
        let mu = 0.2 + 0.5 + ((energy - 7120.0) / 2.0).atan() / std::f64::consts::PI;
        text.push_str(&format!("{energy} 1000 {}\n", 1000.0 * (-mu).exp()));
    }
    let short = XasDataset::load(parse(&text).unwrap());
    let long = scan(0.0, 0.0, false);
    let (merged, _) = merge_scans(&[&long, &short], "merge").unwrap();
    assert_eq!(merged.columns[0].values[0], 7100.0);
    assert_eq!(merged.columns[0].values.len(), 1100);
}

#[test]
fn two_fields_of_one_scan_keep_their_field_names() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    app.doc
        .datasets
        .push(Dataset::Xas(Box::new(scan(0.0, 0.0, false))));
    app.session.ui.data_selection = vec![0];
    app.insert_dataset_canvas(0);
    let dataset = app.doc.datasets[0].clone();
    let fields: Vec<_> = dataset.field_descriptors().iter().map(|f| f.id).collect();
    let mut series: Vec<_> = fields[..2]
        .iter()
        .filter_map(|field| SeriesBinding::from_field_item(&dataset, *field, None, 0))
        .collect();
    for (index, item) in series.iter_mut().enumerate() {
        item.id = SeriesId::new(index as u64);
    }
    let binding = DataBinding {
        coordinates: None,
        series,
    };
    let chart = ChartSpec::default_for(dataset.domain());
    let figure = app.build_binding_figure(&binding, &chart, &StackSpec::default(), [89.0, 60.0]);
    let names: Vec<_> = figure.series.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["Normalized μ(E)", "μ(E)"]);
}

#[test]
fn an_unaligned_merge_warns_and_a_refused_merge_fails_visibly() {
    use crate::operation::OperationOutcome;
    let mut app = PlotiumApp::new_with_settings(Default::default());
    for shift in [0.0, 0.0, 40.0] {
        app.doc
            .datasets
            .push(Dataset::Xas(Box::new(scan(shift, 0.0, false))));
    }
    let outcome = |app: &PlotiumApp| {
        let report = app.session.operation_history.operations().last().unwrap();
        (report.outcome, report.summary.clone())
    };
    app.session.ui.data_selection = vec![0, 1];
    assert!(app.merge_selected_xas());
    let (result, summary) = outcome(&app);
    assert_eq!(result, OperationOutcome::Warning);
    assert!(summary.contains("no reference channel"), "{summary}");
    app.session.ui.data_selection = vec![0, 2];
    assert!(!app.merge_selected_xas());
    let (result, summary) = outcome(&app);
    assert_eq!(result, OperationOutcome::Failure);
    assert!(summary.contains("same absorption edge"), "{summary}");
}

#[test]
fn different_edges_are_named_before_different_signals() {
    let fe = scan(0.0, 0.0, false);
    let mut text = String::from("# Energy mu\n");
    for index in 0..1500 {
        let energy = 11900.0 + f64::from(index) * 0.5;
        let mu = 0.2 + 0.5 + ((energy - 12284.0) / 2.0).atan() / std::f64::consts::PI;
        text.push_str(&format!("{energy} {mu}\n"));
    }
    let hg = XasDataset::load(parse(&text).unwrap());
    let error = merge_scans(&[&fe, &hg], "merge").unwrap_err();
    assert_eq!(error, "Pick scans of the same absorption edge.");
}
