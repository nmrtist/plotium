//! Freeze encoding, scientific results and linked tables as one history step.
use super::Action;
use crate::DiffusionInput;
use crate::state::{ComputeService, Dataset, DatasetId, PlotiumApp};

impl PlotiumApp {
    pub(crate) fn diffusion_input_action(
        &self,
        dataset: DatasetId,
        input: Option<DiffusionInput>,
    ) -> Result<Action, String> {
        let index = self
            .doc
            .dataset_index(dataset)
            .ok_or("Diffusion source no longer exists")?;
        let mut replacement = self.doc.datasets[index].clone();
        let data = replacement
            .as_nmr2d_mut()
            .ok_or("Source has no diffusion encoding")?;
        if data.diffusion_input == input {
            return Ok(Action::Composite(vec![]));
        }
        if let Some(input) = &input {
            input.b_values(data.data.rows)?;
        }
        data.diffusion_input = input;
        data.invalidate_dosy_results(
            "Diffusion analysis input changed and invalidated the selected DOSY map",
        );
        self.scientific_source_action(replacement)
    }

    pub(crate) fn scientific_source_action(&self, replacement: Dataset) -> Result<Action, String> {
        let id = replacement.resource_id();
        let index = self
            .doc
            .dataset_index(id)
            .ok_or("Scientific source no longer exists")?;
        let mut before = vec![self.doc.datasets[index].clone()];
        if let Some(table) = self.region_table_index(index) {
            before.push(self.doc.datasets[table].clone());
        }
        // Derive once against a detached document. Redo installs these exact
        // snapshots, preserving table column IDs and fit references.
        let mut candidate =
            PlotiumApp::with_compute(self.settings.clone(), ComputeService::inactive());
        candidate.doc.datasets = before.clone();
        candidate.doc.datasets[0] = replacement;
        candidate.sync_region_table(0);
        Ok(Action::InstallScientificState {
            before,
            after: candidate.doc.datasets.to_vec(),
        })
    }
}
