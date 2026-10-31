use super::{
    DimensionReport, INSPECTION_SCHEMA, InspectionReport, ProvenanceReport, XpsReport,
    warning_report,
};
use plotium_io::{DataFormat, LoadWarning, Provenance, xps::XpsExperiment};

pub(super) fn inspection_report(
    format: DataFormat,
    provenance: &Provenance,
    warnings: &[LoadWarning],
    experiment: &XpsExperiment,
) -> InspectionReport {
    let points = experiment
        .regions
        .iter()
        .map(|region| region.intensity_cps.len())
        .sum();
    let binding = experiment
        .regions
        .iter()
        .filter(|region| region.binding_energy_ev.is_some())
        .count();
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
            count: 3,
            shape: vec![
                experiment.measurements.len(),
                experiment.regions.len(),
                points,
            ],
        },
        domain: "xps".into(),
        warnings: warnings.iter().map(warning_report).collect(),
        electrophysiology: None,
        afm: None,
        mass_spectrometry: None,
        xrd: None,
        xas: None,
        xps: Some(XpsReport {
            measurement_count: experiment.measurements.len(),
            region_count: experiment.regions.len(),
            point_count: points,
            binding_energy_region_count: binding,
            kinetic_only_region_count: experiment.regions.len() - binding,
            regions: experiment
                .regions
                .iter()
                .map(|region| region.name.clone())
                .collect(),
        }),
    }
}
