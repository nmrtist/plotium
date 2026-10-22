use super::{PlotiumApp, ProjectContextId};
use crate::automation::{
    AutomationError, CallerType, EffectLevel, KIND_CANVAS, ProjectResourceProvider,
    ResourceProvider, ResourceRef, TargetOutcome, TaskId, TaskReceipt, TaskStatus, ToolRegistry,
    ToolResult,
};
use crate::operation::OperationOutcome;
use std::collections::{BTreeMap, VecDeque};

pub const BACKGROUND_ACTIVITY_CAPACITY: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ActivityId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivityChange {
    Created,
    Updated,
}

#[derive(Clone, Debug)]
pub struct ActivityOperation {
    pub id: ActivityId,
    pub title: String,
    pub tool_id: String,
    pub outcome: OperationOutcome,
    pub created: usize,
    pub updated: usize,
    pub failed: usize,
    pub skipped: usize,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ActivityResult {
    pub resource: ResourceRef,
    pub name: String,
    pub change: ActivityChange,
    pub operation: ActivityId,
    pub title: String,
    pub updates: usize,
}

/// Runtime activity belongs to one open project, independently of provenance.
#[derive(Default)]
pub struct BackgroundActivity {
    context: Option<ProjectContextId>,
    next_id: u64,
    operations: VecDeque<ActivityOperation>,
    results: VecDeque<ActivityResult>,
    pending: BTreeMap<TaskId, (ProjectContextId, String)>,
}

impl BackgroundActivity {
    pub fn operations(&self) -> &VecDeque<ActivityOperation> {
        &self.operations
    }

    pub fn results(&self) -> &VecDeque<ActivityResult> {
        &self.results
    }

    pub fn clear(&mut self) {
        self.operations.clear();
        self.results.clear();
    }

    fn push(
        &mut self,
        context: ProjectContextId,
        mut operation: ActivityOperation,
        resources: Vec<(ResourceRef, String, ActivityChange)>,
    ) {
        if self.context != Some(context) {
            self.clear();
            self.pending.clear();
            self.context = Some(context);
        }
        self.next_id = self.next_id.wrapping_add(1).max(1);
        operation.id = ActivityId(self.next_id);
        for (resource, name, change) in resources {
            let previous = self
                .results
                .iter()
                .position(|row| row.resource == resource)
                .and_then(|index| self.results.remove(index));
            let created = change == ActivityChange::Created
                || previous
                    .as_ref()
                    .is_some_and(|row| row.change == ActivityChange::Created);
            self.results.push_back(ActivityResult {
                resource,
                name,
                change: if created {
                    ActivityChange::Created
                } else {
                    ActivityChange::Updated
                },
                operation: operation.id,
                title: operation.title.clone(),
                updates: previous.map_or(1, |row| row.updates.saturating_add(1)),
            });
            while self.results.len() > BACKGROUND_ACTIVITY_CAPACITY {
                self.results.pop_front();
            }
        }
        self.operations.push_back(operation);
        while self.operations.len() > BACKGROUND_ACTIVITY_CAPACITY {
            self.operations.pop_front();
        }
    }
}

impl PlotiumApp {
    pub fn activity_result_available(&self, resource: &ResourceRef) -> bool {
        let exists = ProjectResourceProvider::new(self)
            .inspect(&resource.id)
            .is_some_and(|descriptor| descriptor.resource == *resource);
        exists
            && (resource.kind.0 != KIND_CANVAS
                || self.doc.canvases.iter().any(|canvas| {
                    canvas.resource_id.to_string() == resource.id && !canvas.objects.is_empty()
                }))
    }

    pub(crate) fn record_background_tool(
        &mut self,
        context: ProjectContextId,
        caller: CallerType,
        tool_id: &str,
        outcome: Result<&ToolResult, &AutomationError>,
    ) {
        if context != self.session.project_context || caller == CallerType::Human {
            return;
        }
        let registry = ToolRegistry::built_in();
        let Some(descriptor) = registry.get(tool_id) else {
            return;
        };
        if descriptor.effect == EffectLevel::ReadOnly {
            return;
        }
        if self.session.background_activity.context != Some(context) {
            self.session.background_activity.clear();
            self.session.background_activity.pending.clear();
            self.session.background_activity.context = Some(context);
        }
        if let Some(receipt) = outcome
            .ok()
            .and_then(|result| result.value.get("task"))
            .and_then(|value| serde_json::from_value::<TaskReceipt>(value.clone()).ok())
        {
            self.session
                .background_activity
                .pending
                .insert(receipt.task_id, (context, tool_id.into()));
            self.poll_background_activity();
            return;
        }
        let mut operation = ActivityOperation {
            id: ActivityId(0),
            title: descriptor.title.clone(),
            tool_id: tool_id.into(),
            outcome: OperationOutcome::Success,
            created: 0,
            updated: 0,
            failed: 0,
            skipped: 0,
            diagnostics: Vec::new(),
        };
        let mut resources = Vec::new();
        match outcome {
            Ok(result) => {
                for (items, change) in [
                    (&result.produced, ActivityChange::Created),
                    (&result.modified, ActivityChange::Updated),
                ] {
                    for resource in items {
                        if !resources
                            .iter()
                            .any(|(existing, _, _)| existing == resource)
                            && let Some(descriptor) = ProjectResourceProvider::new(self)
                                .inspect(&resource.id)
                                .filter(|descriptor| descriptor.resource == *resource)
                        {
                            resources.push((resource.clone(), descriptor.name, change));
                        }
                    }
                }
                operation.created = resources
                    .iter()
                    .filter(|(_, _, change)| *change == ActivityChange::Created)
                    .count();
                operation.updated = resources.len() - operation.created;
                for target in &result.targets {
                    match target.outcome {
                        TargetOutcome::Failed => {
                            operation.failed += 1;
                            operation.diagnostics.push(target.message.clone());
                        }
                        TargetOutcome::Skipped => operation.skipped += 1,
                        TargetOutcome::Succeeded => {}
                    }
                }
                operation.diagnostics.extend(result.diagnostics.clone());
                if operation.failed > 0 {
                    operation.outcome = if resources.is_empty()
                        && !result
                            .targets
                            .iter()
                            .any(|target| target.outcome == TargetOutcome::Succeeded)
                    {
                        OperationOutcome::Failure
                    } else {
                        OperationOutcome::Warning
                    };
                }
            }
            Err(error) => {
                operation.failed = 1;
                operation.outcome = OperationOutcome::Failure;
                operation.diagnostics.push(error.to_string());
            }
        }
        self.session
            .background_activity
            .push(context, operation, resources);
    }

    pub(crate) fn poll_background_activity(&mut self) {
        let ready = self
            .session
            .background_activity
            .pending
            .iter()
            .filter_map(|(id, (context, tool))| {
                self.session
                    .scientific_tasks
                    .get(id)
                    .filter(|receipt| receipt.status.terminal())
                    .map(|receipt| (id.clone(), *context, tool.clone(), receipt.clone()))
            })
            .collect::<Vec<_>>();
        for (id, context, tool, receipt) in ready {
            self.session.background_activity.pending.remove(&id);
            let outcome = if receipt.status == TaskStatus::Succeeded {
                Ok(ToolResult {
                    tool_id: tool.clone(),
                    before_revision: receipt.start_revision,
                    after_revision: receipt.end_revision.unwrap_or(receipt.start_revision),
                    targets: Vec::new(),
                    produced: receipt.produced,
                    modified: receipt.modified,
                    diagnostics: Vec::new(),
                    verification: Vec::new(),
                    value: serde_json::Value::Null,
                })
            } else {
                Err(AutomationError::Execution(receipt.message.unwrap_or_else(
                    || {
                        format!(
                            "Operation {}: {}",
                            receipt.phase,
                            receipt
                                .failure_code
                                .as_deref()
                                .unwrap_or("no result committed")
                        )
                    },
                )))
            };
            self.record_background_tool(context, CallerType::Agent, &tool, outcome.as_ref());
        }
    }
}

#[cfg(test)]
#[path = "background_activity_tests.rs"]
mod tests;
