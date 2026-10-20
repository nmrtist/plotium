//! Main-thread registration and atomic commit for prepared scientific actions.
use super::compute::{ScientificOutput, ScientificWork};
use super::*;
use crate::automation::{TaskId, TaskReceipt, TaskStatus};
use serde_json::Value;
impl PlotiumApp {
    pub(crate) fn capture_task_field_versions(
        &mut self,
        receipt: &mut TaskReceipt,
    ) -> Result<(), String> {
        let fields =
            self.doc
                .datasets
                .iter()
                .filter(|d| {
                    receipt.targets.iter().any(|t| {
                        t.parent_id.as_deref().unwrap_or(&t.id) == d.resource_id().to_string()
                    })
                })
                .flat_map(|d| {
                    d.field_descriptors()
                        .into_iter()
                        .map(|field| FieldRef {
                            resource: d.resource_id(),
                            field: field.id,
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
        for field in fields {
            let version = self
                .session
                .compute
                .field_version_for(field)
                .map_err(|e| format!("Could not freeze source field version: {e:?}"))?;
            receipt.sources.push(crate::automation::TaskFieldVersion {
                dataset: field.resource,
                field: field.field,
                version: version.0,
            });
        }
        Ok(())
    }

    pub(crate) fn start_prepared_scientific(
        &mut self,
        dataset: DatasetId,
        mut receipt: TaskReceipt,
        work: ScientificWork,
    ) -> Result<TaskReceipt, String> {
        let id = receipt.task_id.clone();
        if let Some(existing) = self.session.scientific_tasks.get(&id) {
            return Ok(existing.clone());
        }
        if self.session.scientific_tasks.was_consumed(&id) {
            return Err("task_not_found: receipt evicted; do not replay".into());
        }
        if let Some(kind) = self.session.compute.blocking_work_for(dataset) {
            return Err(format!("{} is already running", kind.label()));
        }
        self.capture_task_field_versions(&mut receipt)?;
        self.session.scientific_tasks.register(receipt);
        match self.session.compute.enqueue_scientific(dataset, work) {
            Ok(generation) => self.session.scientific_tasks.bind(
                &id,
                dataset,
                ComputeKind::Scientific,
                generation,
            ),
            Err(error) => self.session.scientific_tasks.update(
                &id,
                TaskStatus::Failed,
                self.doc.automation_revision,
                Some("enqueue_failed"),
                Some(format!("{error:?}")),
                Value::Null,
            ),
        }
        self.session
            .scientific_tasks
            .get(&id)
            .cloned()
            .ok_or_else(|| "Task registration failed".into())
    }
    pub(crate) fn commit_prepared_scientific(
        &mut self,
        id: &TaskId,
        result: &Result<ScientificOutput, String>,
    ) {
        let output = match result {
            Ok(output) => output,
            Err(error) => {
                self.session.status = error.clone();
                self.session.scientific_tasks.update(
                    id,
                    TaskStatus::Failed,
                    self.doc.automation_revision,
                    Some("analysis_failed"),
                    Some(error.clone()),
                    Value::Null,
                );
                return;
            }
        };
        self.session.scientific_tasks.update(
            id,
            TaskStatus::Committing,
            self.doc.automation_revision,
            None,
            None,
            Value::Null,
        );
        let action = if output.create_canvas {
            match output.action.as_ref() {
                Action::InsertTableData { dataset } => Action::insert_dataset_with_default_canvas(
                    self,
                    dataset.as_ref().clone(),
                    format!("Canvas {} — Extracted curves", self.doc.canvases.len() + 1),
                    DEFAULT_CANVAS_SIZE_MM,
                ),
                action => action.clone(),
            }
        } else {
            output.action.as_ref().clone()
        };
        // Canvas insertion normally focuses its new dataset. A background
        // completion must leave the user's current selection and view in place.
        let focus = (
            self.session.active_canvas,
            self.session.ui.data_selection.clone(),
            self.session.view,
        );
        let outcome = self.try_execute_action(action);
        self.session.active_canvas = focus.0;
        self.session.ui.data_selection = focus.1;
        self.session.view = focus.2;
        match outcome {
            Ok(()) => {
                self.session
                    .scientific_tasks
                    .set_produced(id, output.produced.clone());
                self.session.scientific_tasks.update(
                    id,
                    TaskStatus::Succeeded,
                    self.doc.automation_revision,
                    None,
                    None,
                    output.summary.clone(),
                );
                self.session.status = if output.create_canvas {
                    "Region curves are ready. Open the table from the data browser or region tools."
                } else {
                    "Scientific analysis completed."
                }
                .into();
            }
            Err(error) => {
                self.session.status = error.to_string();
                self.session.scientific_tasks.update(
                    id,
                    TaskStatus::Failed,
                    self.doc.automation_revision,
                    Some("commit_failed"),
                    Some(error.to_string()),
                    Value::Null,
                );
            }
        }
    }
}

impl Dataset {
    /// Select the explicitly addressed field only in an isolated worker snapshot.
    pub(crate) fn region_snapshot(&self, field: FieldId) -> Result<Self, String> {
        let descriptor = self
            .field_descriptor(field)
            .ok_or("The source field no longer exists")?;
        if !descriptor
            .capabilities
            .contains(crate::automation::CAP_FIELD_REGION_SERIES)
        {
            return Err("The field has no region series capability".into());
        }
        let mut snapshot = self.clone();
        match &mut snapshot {
            Dataset::Nmr2D(data) => data.display = crate::PseudoDisplay::Stack,
            Dataset::Electrophysiology(data) => {
                let channel = self
                    .field_descriptors()
                    .iter()
                    .position(|d| d.id == field)
                    .ok_or("The selected channel no longer exists")?;
                data.selected_channel = channel;
            }
            _ => {}
        }
        if snapshot.region_source_field() != Some(field) {
            return Err("The region adapter cannot select this field".into());
        }
        Ok(snapshot)
    }
}
