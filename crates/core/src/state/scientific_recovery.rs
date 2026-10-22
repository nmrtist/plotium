//! Explicit recovery creates a new plan from current state, never replays a receipt.
use super::PlotiumApp;
use crate::automation::*;

impl PlotiumApp {
    pub fn scientific_task_recompute_unavailable(&self, id: &TaskId) -> Option<&'static str> {
        let Some(receipt) = self.session.scientific_tasks.get(id) else {
            return Some(
                "Submit the analysis from its controls because this task is no longer retained.",
            );
        };
        if !receipt.status.terminal() || receipt.status == TaskStatus::Succeeded {
            return Some(
                "Wait for the task to finish without applying a result before recomputing here.",
            );
        }
        if receipt.project_context != self.session.project_context {
            return Some("Return to the source project and submit the analysis again.");
        }
        if !self.session.scientific_tasks.permitted(id) {
            return Some(
                "Review the analysis and submit a new task from its controls because permission was withdrawn.",
            );
        }
        let Some(recovery) = self.session.scientific_tasks.recovery(id) else {
            return Some("Review the inputs and submit the analysis from its controls.");
        };
        if recovery.request.tool_id == "processing.apply_scheme" {
            return Some("Select the processing scheme again to review its current contents.");
        }
        let provider = ProjectResourceProvider::new(self);
        if recovery.targets.iter().any(|resource| {
            provider
                .inspect(&resource.id)
                .is_none_or(|d| d.resource != *resource)
        }) {
            return Some(
                "Select an available source and submit the analysis again; the original source is unavailable.",
            );
        }
        if recovery.targets.iter().any(|resource| {
            self.doc.datasets.iter().any(|dataset| {
                dataset.resource_id().to_string()
                    == resource.parent_id.as_deref().unwrap_or(&resource.id)
                    && self
                        .session
                        .compute
                        .blocking_work_for(dataset.resource_id())
                        .is_some()
            })
        }) {
            return Some(
                "Wait for this source's current computation to finish before recomputing.",
            );
        }
        None
    }

    /// Reuses the submitted parameters, with freshly captured project and source versions.
    pub fn recompute_scientific_task(&mut self, id: &TaskId) -> Result<TaskReceipt, String> {
        if let Some(reason) = self.scientific_task_recompute_unavailable(id) {
            return Err(reason.into());
        }
        let recovery = self
            .session
            .scientific_tasks
            .recovery(id)
            .cloned()
            .ok_or("The task inputs are no longer retained.")?;
        let tool_id = recovery.request.tool_id.clone();
        let mut request = recovery.request;
        request.expected_revision = DocumentRevision(self.doc.automation_revision);
        request.caller = CallerType::Human;
        request.targets = TargetSelector::Explicit {
            ids: recovery
                .targets
                .iter()
                .map(|resource| resource.id.clone())
                .collect(),
        };
        let mut plan = match plan_tool(self, request) {
            Ok(plan) => plan,
            Err(error) => {
                self.record_background_tool(
                    self.session.project_context,
                    CallerType::Agent,
                    &tool_id,
                    Err(&error),
                );
                return Err(error.to_string());
            }
        };
        plan.desktop_region = recovery.desktop_region;
        let result = match execute_tool(self, plan, ExecutionAuthority::ReversibleModify) {
            Ok(result) => result,
            Err(error) => {
                self.record_background_tool(
                    self.session.project_context,
                    CallerType::Agent,
                    &tool_id,
                    Err(&error),
                );
                return Err(error.to_string());
            }
        };
        self.record_background_tool(
            self.session.project_context,
            CallerType::Agent,
            &tool_id,
            Ok(&result),
        );
        serde_json::from_value(result.value["task"].clone()).map_err(|error| error.to_string())
    }
}
