//! Stable scientific child resources, without embedding result arrays.
use super::*;
/// Stable address of the native table-local diffusion analysis identity.
pub fn diffusion_analysis_ref(table: crate::state::DatasetId, analysis: u64) -> ResourceRef {
    child_ref(table, &format!("diffusion_fit:{analysis}"), KIND_ANALYSIS)
}

pub(super) fn analysis_descriptors(
    table: &crate::state::TableDataset,
    revision: DocumentRevision,
) -> Vec<ResourceDescriptor> {
    let mut descriptors = Vec::new();
    for analysis in table
        .curve_fit_analyses
        .iter()
        .filter(|a| a.result.model.id == plotium_analysis::models::DIFFUSION_B_ID)
    {
        descriptors.push(ResourceDescriptor {
            resource: diffusion_analysis_ref(table.resource_id, analysis.id),
            name: analysis.name.clone(),
            capabilities: vec![cap(CAP_DIFFUSION_RESULT)],
            children: Vec::new(),
            dimensions: vec![analysis.result.points.len()],
            units: vec!["s/m²".into(), "m²/s".into()],
            metadata: BTreeMap::from([("model".into(), analysis.result.model.id.clone())]),
            lineage: vec![table.resource_id.to_string()],
            revision,
        });
    }
    descriptors
}
