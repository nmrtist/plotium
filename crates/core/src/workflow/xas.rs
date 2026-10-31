use super::{
    DimensionReport, INSPECTION_SCHEMA, InspectionReport, ProvenanceReport, warning_report,
};
use plotium_io::{DataFormat, LoadWarning, Provenance, xas::XasScan};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct XasReport {
    pub beamline: Option<String>,
    pub sample: Option<String>,
    pub element: Option<String>,
    pub edge: Option<String>,
    pub energy_range_ev: [f64; 2],
    pub point_count: usize,
    pub columns: Vec<String>,
}

pub(super) fn inspection_report(
    format: DataFormat,
    provenance: &Provenance,
    warnings: &[LoadWarning],
    scan: &XasScan,
) -> InspectionReport {
    InspectionReport {
        schema: INSPECTION_SCHEMA,
        format: format.as_str().to_owned(),
        provenance: ProvenanceReport {
            selected_path: provenance.selected_path.clone(),
            data_path: provenance.data_path.clone(),
            parameter_paths: provenance.parameter_paths.clone(),
            companion_paths: provenance.companion_paths.clone(),
        },
        dimension: DimensionReport {
            count: 1,
            shape: vec![scan.len()],
        },
        domain: "xas".into(),
        warnings: warnings.iter().map(warning_report).collect(),
        electrophysiology: None,
        afm: None,
        mass_spectrometry: None,
        xrd: None,
        xps: None,
        xas: Some(XasReport {
            beamline: scan.beamline.clone(),
            sample: scan.sample.clone(),
            element: scan.element.clone(),
            edge: scan.edge.clone(),
            energy_range_ev: [
                scan.energy().first().copied().unwrap_or(0.0),
                scan.energy().last().copied().unwrap_or(0.0),
            ],
            point_count: scan.len(),
            columns: scan.columns.iter().map(|c| c.name.clone()).collect(),
        }),
    }
}
