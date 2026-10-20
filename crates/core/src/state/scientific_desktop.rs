//! Desktop commands freeze the same semantic operations as headless clients.
use super::*;
use crate::automation::*;
use serde_json::{Value, json};
impl PlotiumApp {
    fn desktop_scientific_plan(
        &self,
        operation: &str,
        parameters: Value,
        ids: Vec<String>,
    ) -> Result<ToolPlan, AutomationError> {
        plan_tool(
            self,
            ToolRequest {
                tool_id: operation.into(),
                tool_version: 1,
                parameters,
                targets: TargetSelector::Explicit { ids },
                expected_revision: DocumentRevision(self.doc.automation_revision),
                caller: CallerType::Human,
            },
        )
    }
    fn submit_desktop_scientific(&mut self, plan: ToolPlan) {
        match execute_tool(self, plan, ExecutionAuthority::ReversibleModify) {
            Ok(result) => {
                self.session.status = format!(
                    "Queued scientific task {}.",
                    result.value["task"]["task_id"]
                        .as_str()
                        .unwrap_or("unknown")
                )
            }
            Err(error) => self.session.status = error.to_string(),
        }
    }
    pub fn request_processing_scheme(
        &mut self,
        path: &std::path::Path,
        targets: &[DatasetId],
        compatible_only: bool,
    ) {
        match self.desktop_scientific_plan(
            "processing.apply_scheme",
            json!({"path":path,"compatible_only":compatible_only}),
            targets.iter().map(ToString::to_string).collect(),
        ) {
            Ok(plan) => self.submit_desktop_scientific(plan),
            Err(error) => self.session.status = error.to_string(),
        }
    }
    pub fn request_region_table_operation(&mut self, dataset: usize, live: bool) {
        let Some(source) = self.doc.datasets.get(dataset) else {
            return;
        };
        if live && self.region_table_index(dataset).is_some() {
            self.session.status = "This dataset already has a linked series table.".into();
            return;
        }
        let Some(field) = source.region_source_field() else {
            self.session.status = "Select a field with region series capability.".into();
            return;
        };
        let Some(state) = source.region_analysis() else {
            return;
        };
        let regions = state
            .regions
            .iter()
            .map(|r| json!({"source":"existing","id":r.id}))
            .collect::<Vec<_>>();
        let owner = source.resource_id().to_string();
        let target = ProjectResourceProvider::new(self)
            .descriptors()
            .into_iter()
            .find(|d| {
                d.resource.parent_id.as_deref() == Some(owner.as_str())
                    && d.resource.local_id.as_deref()
                        == source
                            .field_descriptor(field)
                            .as_ref()
                            .map(|f| f.local_id.as_str())
            })
            .map(|d| d.resource.id);
        let Some(target) = target else {
            self.session.status = "The source field no longer exists.".into();
            return;
        };
        match self.desktop_scientific_plan(
            REGIONS_EXTRACT,
            json!({"regions":regions}),
            vec![target],
        ) {
            Ok(mut plan) => {
                plan.desktop_region = Some(live);
                self.submit_desktop_scientific(plan);
            }
            Err(error) => self.session.status = error.to_string(),
        }
    }
    pub fn request_diffusion_curve_fit(
        &mut self,
        dataset: usize,
        column: plotium_data::ColumnId,
        options: plotium_analysis::fit_model::FitOptions,
    ) {
        let Some(id) = self.doc.datasets.get(dataset).map(Dataset::resource_id) else {
            return;
        };
        match self.desktop_scientific_plan(
            DIFFUSION_FIT,
            json!({"response_column":column,"options":options}),
            vec![id.to_string()],
        ) {
            Ok(plan) => self.submit_desktop_scientific(plan),
            Err(error) => self.session.status = error.to_string(),
        }
    }
}
