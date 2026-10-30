use super::*;
use crate::state::{DatasetLineage, DerivationKind, TableDataset, XpsDataset, XrdDataset};
use plotium_data::{ColumnChunk, ColumnSchema, LogicalType};
use plotium_io::xps::{
    XpsEnergyKind, XpsExperiment, XpsMeasurement, XpsMeasurementId, XpsRegion, XpsRegionId,
};
use plotium_processing::arithmetic::SpectrumBinaryOp;
use std::collections::BTreeMap;

fn xrd(angles: Vec<f64>, intensity: Vec<f64>, wavelength: Option<f64>) -> Dataset {
    Dataset::Xrd(Box::new(XrdDataset::load(plotium_io::XrdData {
        two_theta_deg: angles,
        intensity,
        attenuation: None,
        source: "scan.rasx".to_owned(),
        instrument: None,
        target: None,
        wavelength_angstrom: wavelength,
        voltage_kv: None,
        current_ma: None,
        scan_step_deg: None,
        scan_speed_deg_min: None,
    })))
}

fn xps(intensity: Vec<f64>) -> Dataset {
    let energy: Vec<f64> = (0..intensity.len())
        .map(|index| 290.0 - index as f64)
        .collect();
    Dataset::Xps(Box::new(XpsDataset::load(XpsExperiment {
        source: "region.vms".into(),
        measurements: vec![XpsMeasurement {
            id: XpsMeasurementId(1),
            label: "Location 1".into(),
            position_mm: None,
            metadata: BTreeMap::new(),
        }],
        regions: vec![XpsRegion {
            id: XpsRegionId(1),
            measurement: XpsMeasurementId(1),
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
    })))
}

fn app_with(datasets: Vec<Dataset>) -> PlotiumApp {
    let mut app = PlotiumApp::new();
    app.doc.datasets.extend(datasets);
    app
}

fn intensity(app: &PlotiumApp, index: usize) -> Vec<f64> {
    app.doc.datasets[index]
        .as_xrd()
        .unwrap()
        .data
        .intensity
        .clone()
}

fn table(xs: &[Option<f64>], ys: &[Option<f64>], unit: &str) -> Dataset {
    let x = ColumnChunk::optional_f64(xs.iter().copied());
    let mut x_schema = ColumnSchema::new("q", LogicalType::Float64);
    x_schema.nullable = true;
    x_schema.unit = crate::state::unit_from_label(unit);
    let y = ColumnChunk::optional_f64(ys.iter().copied());
    let mut y_schema = ColumnSchema::new("I", LogicalType::Float64);
    y_schema.nullable = true;
    let x_binding = x_schema.id;
    let series = crate::state::TableSeriesBinding {
        value_column: y_schema.id,
        uncertainty_column: None,
        fit: None,
    };
    Dataset::Table(Box::new(
        TableDataset::from_materialized(
            vec![(x_schema, x), (y_schema, y)],
            Vec::new(),
            Some(x_binding),
            vec![series],
            "test.table",
        )
        .unwrap(),
    ))
}

#[test]
fn subtracting_an_empty_cell_creates_a_new_xrd_dataset() {
    let mut app = app_with(vec![
        xrd(vec![10.0, 11.0, 12.0], vec![5.0, 9.0, 5.0], Some(0.7)),
        xrd(vec![10.0, 12.0], vec![2.0, 4.0], Some(0.7)),
    ]);
    assert!(app.spectrum_arithmetic_compat(0, 1).unwrap().is_some());

    app.combine_spectra_datasets(0, 1, SpectrumBinaryOp::Subtract, 0.5);

    assert_eq!(app.doc.datasets.len(), 3);
    assert_eq!(intensity(&app, 2), vec![4.0, 7.5, 3.0]);
    assert_eq!(
        app.doc.datasets[2]
            .as_xrd()
            .unwrap()
            .data
            .wavelength_angstrom,
        Some(0.7)
    );
    assert_eq!(
        app.doc.datasets[2].lineage(),
        Some(&DatasetLineage::new(
            DerivationKind::SpectrumArithmetic,
            [
                app.doc.datasets[0].resource_id(),
                app.doc.datasets[1].resource_id(),
            ]
        ))
    );
    app.undo();
    assert_eq!(app.doc.datasets.len(), 2);
}

#[test]
fn negative_xrd_results_are_clipped_and_the_project_still_saves() {
    let mut app = app_with(vec![
        xrd(vec![1.0, 2.0], vec![1.0, 5.0], None),
        xrd(vec![1.0, 2.0], vec![3.0, 1.0], None),
    ]);
    app.combine_spectra_datasets(0, 1, SpectrumBinaryOp::Subtract, 1.0);

    assert_eq!(intensity(&app, 2), vec![0.0, 4.0]);
    assert!(app.session.status.contains("1 negative points"));
    app.doc.datasets[2]
        .as_xrd()
        .unwrap()
        .data
        .validate()
        .unwrap();
}

#[test]
fn scale_and_offset_apply_to_xrd_traces() {
    let mut app = app_with(vec![xrd(vec![1.0, 2.0], vec![2.0, 3.0], None)]);
    app.scale_spectrum_dataset(0, 2.0, 1.0);
    assert_eq!(intensity(&app, 1), vec![5.0, 7.0]);
}

#[test]
fn averaging_uses_the_first_grid_and_the_processed_traces() {
    let mut app = app_with(vec![
        xrd(vec![1.0, 2.0, 3.0], vec![2.0, 4.0, 6.0], None),
        xrd(vec![1.0, 2.0], vec![4.0, 8.0], None),
        xrd(vec![1.0, 2.0, 3.0], vec![0.0, 0.0, 6.0], None),
    ]);
    assert_eq!(app.trace_average_compat(&[0, 1, 2]), Ok(None));
    app.average_trace_datasets(&[0, 1, 2]);
    assert_eq!(intensity(&app, 3), vec![2.0, 4.0, 6.0]);
    assert!(
        app.doc.datasets[3]
            .name()
            .unwrap()
            .starts_with("Average of ")
    );
}

#[test]
fn xps_regions_average_into_a_binding_energy_region() {
    let mut app = app_with(vec![
        xps(vec![1.0, 3.0, 5.0, 3.0, 1.0, 1.0, 1.0, 1.0]),
        xps(vec![3.0, 5.0, 7.0, 5.0, 3.0, 3.0, 3.0, 3.0]),
    ]);
    assert_eq!(app.spectrum_arithmetic_compat(0, 1), Ok(None));

    app.average_trace_datasets(&[0, 1]);

    let trace = app.doc.datasets[2].displayed_trace(None).unwrap();
    assert_eq!(trace.ys, vec![2.0, 4.0, 6.0, 4.0, 2.0, 2.0, 2.0, 2.0]);
    assert_eq!(trace.xs[0], 290.0);
    assert!(trace.x_reversed);
}

#[test]
fn regions_that_share_no_energies_are_refused() {
    let mut far = xps(vec![1.0; 8]);
    if let Dataset::Xps(data) = &mut far {
        let mut experiment = (*data.experiment).clone();
        let energy: Vec<f64> = (0..8).map(|index| 540.0 - f64::from(index)).collect();
        experiment.regions[0].native_energy_ev = energy.clone();
        experiment.regions[0].binding_energy_ev = Some(energy);
        **data = XpsDataset::load(experiment);
    }
    let app = app_with(vec![xps(vec![1.0; 8]), far]);
    let reason = app.trace_average_compat(&[0, 1]).unwrap_err();
    assert!(reason.contains("do not overlap"));
    assert!(app.spectrum_arithmetic_compat(0, 1).is_err());
}

#[test]
fn kinetic_and_binding_regions_are_refused() {
    let mut kinetic = xps(vec![1.0; 8]);
    if let Dataset::Xps(data) = &mut kinetic {
        let mut experiment = (*data.experiment).clone();
        experiment.regions[0].native_energy_kind = XpsEnergyKind::Kinetic;
        experiment.regions[0].binding_energy_ev = None;
        experiment.regions[0].photon_energy_ev = None;
        **data = XpsDataset::load(experiment);
    }
    let app = app_with(vec![xps(vec![1.0; 8]), kinetic]);
    assert!(
        app.spectrum_arithmetic_compat(0, 1)
            .unwrap_err()
            .contains("energy scale")
    );
}

#[test]
fn nmr_spectra_are_not_averaged_as_real_traces() {
    let mut app = sample_app();
    app.doc.datasets.push(Dataset::Nmr(Box::new(
        NmrDataset::load(synthetic_1d()).unwrap(),
    )));
    assert!(
        app.trace_average_compat(&[0, 1])
            .unwrap_err()
            .contains("Average works")
    );
}

#[test]
fn incompatible_operands_explain_why() {
    let mut app = sample_app();
    app.doc
        .datasets
        .push(xrd(vec![1.0, 2.0], vec![1.0, 2.0], Some(0.7)));
    app.doc
        .datasets
        .push(xrd(vec![1.0, 2.0], vec![1.0, 2.0], Some(1.54)));
    assert!(
        app.spectrum_arithmetic_compat(0, 1)
            .unwrap_err()
            .contains("same kind")
    );
    assert!(
        app.spectrum_arithmetic_compat(1, 2)
            .unwrap_err()
            .contains("Wavelengths differ")
    );
    assert!(app.trace_average_compat(&[1]).is_err());
    let count = app.doc.datasets.len();
    app.combine_spectra_datasets(1, 2, SpectrumBinaryOp::Add, 1.0);
    assert_eq!(app.doc.datasets.len(), count);
}

#[test]
fn table_curves_skip_blank_rows_and_keep_the_axis_unit() {
    let mut app = app_with(vec![
        table(
            &[Some(1.0), None, Some(2.0)],
            &[Some(3.0), Some(9.0), Some(5.0)],
            "nm",
        ),
        table(&[Some(1.0), Some(2.0)], &[Some(1.0), Some(1.0)], "nm"),
        table(&[Some(1.0), Some(2.0)], &[Some(1.0), Some(1.0)], "s"),
    ]);
    assert_eq!(app.spectrum_arithmetic_targets(), vec![0, 1, 2]);
    assert!(
        app.spectrum_arithmetic_compat(0, 2)
            .unwrap_err()
            .contains("units differ")
    );

    app.combine_spectra_datasets(0, 1, SpectrumBinaryOp::Subtract, 1.0);

    let trace = app.doc.datasets[3].displayed_trace(None).unwrap();
    assert_eq!(trace.xs, vec![1.0, 2.0]);
    assert_eq!(trace.ys, vec![2.0, 4.0]);
    assert_eq!(app.doc.datasets[3].trace_x_unit(), "nm");
}

#[test]
fn blank_tables_cannot_be_scaled() {
    let mut app = app_with(vec![
        table(&[None, None], &[None, None], ""),
        table(&[Some(1.0), Some(2.0)], &[Some(1.0), Some(2.0)], "nm"),
    ]);
    assert!(app.spectrum_scale_compat(0).is_err());
    app.scale_spectrum_dataset(0, 2.0, 0.0);
    assert_eq!(app.doc.datasets.len(), 2);
    assert_eq!(app.spectrum_scale_compat(1), Ok(None));
}

#[test]
fn table_axes_must_be_monotonic() {
    let app = app_with(vec![
        table(
            &[Some(1.0), Some(3.0), Some(2.0)],
            &[Some(1.0), Some(1.0), Some(1.0)],
            "",
        ),
        table(&[Some(1.0), Some(2.0)], &[Some(1.0), Some(1.0)], ""),
    ]);
    assert!(
        app.spectrum_arithmetic_compat(0, 1)
            .unwrap_err()
            .contains("monotonic")
    );
}
