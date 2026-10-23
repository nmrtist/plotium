//! Scientific task submission and main-thread terminal decisions.
use super::*;
use crate::automation::{DocumentRevision, ResourceRef, TaskId, TaskReceipt, TaskStatus};
use serde_json::{Value, json};

impl PlotiumApp {
    pub fn start_dosy_task(
        &mut self,
        dataset: DatasetId,
        task_id: TaskId,
        plan_id: Option<String>,
        params: Option<IltParams>,
        explicit: Option<crate::DiffusionInput>,
    ) -> Result<TaskReceipt, String> {
        if let Some(receipt) = self.session.scientific_tasks.get(&task_id) {
            return Ok(receipt.clone());
        }
        if self.session.scientific_tasks.was_consumed(&task_id) {
            return Err(
                "task_not_found: the submitted task receipt was evicted; do not replay it.".into(),
            );
        }
        if let Some(params) = params {
            validate_ilt_params(params)?;
        }
        let data = self
            .doc
            .dataset_index(dataset)
            .and_then(|i| self.doc.datasets[i].as_nmr2d())
            .ok_or_else(|| "The diffusion source no longer exists.".to_owned())?;
        if let Some(reason) = data.dosy_input_error() {
            return Err(reason.into());
        }
        let input = data.resolve_diffusion_input(explicit.as_ref())?;
        let Processed2D::Stack(stack) = &data.processed else {
            return Err("Select a spectrum stack.".into());
        };
        if let Some(kind) = self.session.compute.blocking_work_for(dataset) {
            return Err(format!("{} is already running.", kind.label()));
        }
        let (stack, nucleus, source) = (
            stack.clone(),
            data.data.direct.nucleus.clone(),
            stack.source.clone(),
        );
        let mut receipt = TaskReceipt {
            sources: Vec::new(),
            task_id: task_id.clone(),
            plan_id,
            operation: "analysis.dosy.build".into(),
            project_context: self.session.project_context,
            targets: vec![ResourceRef::from(dataset)],
            start_revision: DocumentRevision(self.doc.automation_revision),
            end_revision: None,
            status: TaskStatus::Queued,
            phase: "queued".into(),
            completed: None,
            total: None,
            failure_code: None,
            message: None,
            produced: vec![],
            modified: vec![ResourceRef::from(dataset)],
            summary: json!({"method":if params.is_some(){"ilt"}else{"mono_exp"}}),
        };
        self.capture_task_field_versions(&mut receipt)?;
        self.register_background_task(&receipt);
        self.session.scientific_tasks.register(receipt);
        use crate::automation::{CAP_DIFFUSION_INPUT, ProjectResourceProvider, ResourceProvider};
        let owner = dataset.to_string();
        let recovery_targets = ProjectResourceProvider::new(self)
            .descriptors()
            .into_iter()
            .find(|descriptor| {
                descriptor.resource.parent_id.as_deref() == Some(owner.as_str())
                    && descriptor
                        .capabilities
                        .iter()
                        .any(|capability| capability.0 == CAP_DIFFUSION_INPUT)
            })
            .map(|descriptor| vec![descriptor.resource])
            .unwrap_or_else(|| vec![ResourceRef::from(dataset)]);
        self.session.scientific_tasks.set_recovery(&task_id, crate::automation::ScientificRecovery {
            request: crate::automation::ToolRequest {
                tool_id: crate::automation::DOSY_BUILD.into(),
                tool_version: 1,
                parameters: json!({"method":if params.is_some(){"ilt"}else{"mono_exp"},"params":params,"input":input}),
                targets: crate::automation::TargetSelector::Explicit { ids: vec![dataset.to_string()] },
                expected_revision: DocumentRevision(self.doc.automation_revision),
                caller: crate::automation::CallerType::Human,
            },
            targets: recovery_targets,
            desktop_region: None,
        });
        let kind = if params.is_some() {
            ComputeKind::Ilt
        } else {
            ComputeKind::Dosy
        };
        let outcome = if let Some(params) = params {
            self.session.compute.enqueue_ilt(
                dataset,
                self.session.dataset_epoch,
                stack,
                log_grid(params.d_min, params.d_max, params.n_grid),
                params.lambda,
                params,
                input,
                nucleus,
                source,
            )
        } else {
            self.session.compute.enqueue_dosy(
                dataset,
                self.session.dataset_epoch,
                stack,
                input,
                nucleus,
                source,
            )
        };
        match outcome {
            Ok(()) => {
                if let Some(generation) = self.session.compute.latest_generation(dataset, kind) {
                    self.session
                        .scientific_tasks
                        .bind(&task_id, dataset, kind, generation);
                } else {
                    self.session.scientific_tasks.update(
                        &task_id,
                        TaskStatus::Failed,
                        self.doc.automation_revision,
                        Some("worker_unavailable"),
                        Some("The worker did not register this computation.".into()),
                        Value::Null,
                    );
                }
            }
            Err(error) => self.session.scientific_tasks.update(
                &task_id,
                TaskStatus::Failed,
                self.doc.automation_revision,
                Some("enqueue_failed"),
                Some(format!("{error:?}")),
                Value::Null,
            ),
        }
        self.session
            .scientific_tasks
            .get(&task_id)
            .cloned()
            .ok_or_else(|| "Task receipt was not retained.".into())
    }
    pub fn cancel_scientific_task(&mut self, id: &TaskId) -> Option<TaskReceipt> {
        let receipt = self.session.scientific_tasks.get(id)?;
        if !receipt.status.terminal() {
            if let Some((dataset, kind, _)) = self.session.scientific_tasks.binding(id) {
                self.session.compute.cancel(dataset, kind);
            }
            self.session.scientific_tasks.update(
                id,
                TaskStatus::Cancelled,
                self.doc.automation_revision,
                Some("request_cancelled"),
                None,
                Value::Null,
            );
        }
        self.session.scientific_tasks.get(id).cloned()
    }
    /// Returns true when the retained task owns this completion. The legacy
    /// branch remains for direct ComputeService callers that did not submit a task.
    pub(crate) fn finish_scientific_compute(&mut self, done: &Done) -> bool {
        let (dataset, kind, generation) = match done {
            Done::Scientific {
                dataset,
                generation,
                ..
            } => (*dataset, ComputeKind::Scientific, *generation),
            Done::Dosy {
                dataset,
                generation,
                ..
            } => (*dataset, ComputeKind::Dosy, *generation),
            Done::Ilt {
                dataset,
                generation,
                ..
            } => (*dataset, ComputeKind::Ilt, *generation),
            Done::Cancelled {
                dataset,
                kind,
                generation,
            }
            | Done::Failed {
                dataset,
                kind,
                generation,
            }
            | Done::AnalysisProgress {
                dataset,
                kind,
                generation,
                ..
            }
            | Done::AnalysisStarted {
                dataset,
                kind,
                generation,
            } => (*dataset, *kind, *generation),
            _ => return false,
        };
        let Some(id) = self
            .session
            .scientific_tasks
            .find(dataset, kind, generation)
        else {
            return false;
        };
        let Some(receipt) = self.session.scientific_tasks.get(&id).cloned() else {
            return false;
        };
        if receipt.status.terminal() {
            return true;
        }
        if let Done::AnalysisProgress {
            completed, total, ..
        } = done
        {
            self.session
                .scientific_tasks
                .progress(&id, *completed, *total);
            return true;
        }
        let revision = self.doc.automation_revision;
        if matches!(done, Done::AnalysisStarted { .. }) {
            self.session.scientific_tasks.update(
                &id,
                TaskStatus::Running,
                revision,
                None,
                None,
                receipt.summary,
            );
            return true;
        }
        let failure = if !self.session.scientific_tasks.permitted(&id) {
            Some((TaskStatus::Failed, "permission_denied"))
        } else if receipt.project_context != self.session.project_context {
            Some((TaskStatus::Stale, "project_changed"))
        } else if self.doc.dataset_index(dataset).is_none() {
            Some((TaskStatus::Stale, "target_deleted"))
        } else if receipt.sources.iter().any(|source| {
            self.session
                .compute
                .current_field_version(FieldRef {
                    resource: source.dataset,
                    field: source.field,
                })
                .is_none_or(|v| v.0 != source.version)
        }) {
            Some((TaskStatus::Stale, "source_version_changed"))
        } else if receipt.start_revision.0 != revision {
            Some((TaskStatus::Stale, "stale_revision"))
        } else if matches!(done, Done::Cancelled { .. }) {
            Some((TaskStatus::Cancelled, "request_cancelled"))
        } else if matches!(done, Done::Failed { .. }) {
            Some((TaskStatus::Failed, "worker_failed"))
        } else if !self.session.compute.is_current(dataset, kind, generation) {
            Some((TaskStatus::Stale, "superseded"))
        } else {
            None
        };
        if let Some((status, code)) = failure {
            self.session.status = crate::automation::task_failure_message(code).into();
            self.session.scientific_tasks.update(
                &id,
                status,
                revision,
                Some(code),
                None,
                Value::Null,
            );
            return true;
        }
        if let Done::Scientific { result, .. } = done {
            self.commit_prepared_scientific(&id, result);
            return true;
        }
        let index = self
            .doc
            .dataset_index(dataset)
            .expect("target checked above");
        let Some(data) = self.doc.datasets[index].as_nmr2d() else {
            self.session.scientific_tasks.update(
                &id,
                TaskStatus::Stale,
                revision,
                Some("target_changed"),
                None,
                Value::Null,
            );
            return true;
        };
        let mut next = data.clone();
        let (summary, epoch, provenance) = match done {
            Done::Dosy {
                result,
                provenance,
                figure,
                epoch,
                ..
            } => {
                let accepted = result.d.iter().filter(|v| v.is_finite()).count();
                next.dosy_map = Some(result.clone());
                next.dosy_provenance = Some(provenance.clone());
                next.dosy_figure = Some(figure.clone());
                next.dosy_method = DosyMethod::MonoExp;
                (
                    json!({"method":"mono_exp","status":if accepted==0{"empty"}else{"available"},"accepted_columns":accepted,"rejected_columns":result.d.len()-accepted,"diffusion_unit":"m²/s"}),
                    *epoch,
                    provenance,
                )
            }
            Done::Ilt {
                result,
                params,
                provenance,
                figure,
                epoch,
                ..
            } => {
                let accepted = result
                    .amp
                    .iter()
                    .filter(|column| column.iter().any(|v| *v > 0.))
                    .count();
                next.ilt_map = Some(result.clone());
                next.ilt_provenance = Some(provenance.clone());
                next.ilt_figure = Some(figure.clone());
                next.dosy_method = DosyMethod::Ilt(*params);
                (
                    json!({"method":"ilt","status":if accepted==0{"empty"}else{"available"},"accepted_columns":accepted,"rejected_columns":result.amp.len()-accepted,"diffusion_unit":"m²/s","lambda":params.lambda}),
                    *epoch,
                    provenance,
                )
            }
            _ => return true,
        };
        let current = match (&data.processed, provenance.encoding.as_ref()) {
            (Processed2D::Stack(stack), Some(input)) => {
                dosy_data_fingerprint(stack, input) == provenance.data_fingerprint
            }
            _ => false,
        };
        if epoch != self.session.dataset_epoch || !current {
            self.session.scientific_tasks.update(
                &id,
                TaskStatus::Stale,
                revision,
                Some("source_changed"),
                None,
                Value::Null,
            );
            return true;
        }
        next.dosy_provenance_warning = None;
        let installed = if kind == ComputeKind::Ilt {
            next.ilt_provenance.as_mut()
        } else {
            next.dosy_provenance.as_mut()
        };
        if let Some(provenance) = installed {
            provenance.source_fields = receipt.sources.clone();
        }
        self.session.scientific_tasks.update(
            &id,
            TaskStatus::Committing,
            revision,
            None,
            None,
            Value::Null,
        );
        let outcome = self
            .scientific_source_action(Dataset::Nmr2D(Box::new(next)))
            .and_then(|action| self.try_execute_action(action).map_err(|e| e.to_string()));
        match outcome {
            Ok(()) => {
                let key = if kind == ComputeKind::Ilt {
                    "nmr.ilt_map"
                } else {
                    "nmr.dosy_map"
                };
                use crate::automation::{ProjectResourceProvider, ResourceProvider};
                let owner = dataset.to_string();
                let produced = ProjectResourceProvider::new(self)
                    .descriptors()
                    .into_iter()
                    .filter(|d| {
                        d.resource.parent_id.as_deref() == Some(owner.as_str())
                            && d.resource.local_id.as_deref() == Some(key)
                    })
                    .map(|d| d.resource)
                    .collect();
                self.session.scientific_tasks.set_produced(&id, produced);
                self.session.status = if summary["status"] == "empty" {
                    "DOSY result is empty: no accepted columns."
                } else {
                    "Built DOSY result."
                }
                .into();
                self.session.scientific_tasks.update(
                    &id,
                    TaskStatus::Succeeded,
                    self.doc.automation_revision,
                    None,
                    None,
                    summary,
                );
            }
            Err(error) => self.session.scientific_tasks.update(
                &id,
                TaskStatus::Failed,
                self.doc.automation_revision,
                Some("commit_failed"),
                Some(error.to_string()),
                Value::Null,
            ),
        }
        true
    }
}
