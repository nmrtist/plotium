use super::{
    DimensionReport, INSPECTION_SCHEMA, InspectionReport, ProvenanceReport, XrdReport,
    warning_report,
};
use plotium_io::{DataFormat, LoadWarning, Provenance, XrdData};

pub(super) fn inspection_report(
    format: DataFormat,
    provenance: &Provenance,
    warnings: &[LoadWarning],
    data: &XrdData,
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
            shape: vec![data.len()],
        },
        domain: "xrd".to_owned(),
        warnings: warnings.iter().map(warning_report).collect(),
        electrophysiology: None,
        afm: None,
        mass_spectrometry: None,
        xrd: Some(XrdReport {
            instrument: data.instrument.clone(),
            target: data.target.clone(),
            wavelength_angstrom: data.wavelength_angstrom,
            two_theta_range_deg: [
                data.two_theta_deg.first().copied().unwrap_or(0.0),
                data.two_theta_deg.last().copied().unwrap_or(0.0),
            ],
            point_count: data.len(),
        }),
        xps: None,
        xas: None,
    }
}
