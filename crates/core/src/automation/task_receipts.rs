//! Process-owned receipts for scientific work submitted to ComputeService.
use super::{DocumentRevision, ResourceRef, TaskId};
use crate::state::{ComputeKind, DatasetId, ProjectContextId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeSet, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Running,
    Committing,
    Succeeded,
    Failed,
    Cancelled,
    Stale,
}
impl TaskStatus {
    pub fn terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::Stale
        )
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TaskReceipt {
    #[serde(default)]
    pub sources: Vec<TaskFieldVersion>,
    pub task_id: TaskId,
    pub plan_id: Option<String>,
    pub operation: String,
    pub project_context: ProjectContextId,
    pub targets: Vec<ResourceRef>,
    pub start_revision: DocumentRevision,
    pub end_revision: Option<DocumentRevision>,
    pub status: TaskStatus,
    pub phase: String,
    pub completed: Option<usize>,
    pub total: Option<usize>,
    pub failure_code: Option<String>,
    pub message: Option<String>,
    pub produced: Vec<ResourceRef>,
    pub modified: Vec<ResourceRef>,
    pub summary: Value,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TaskFieldVersion {
    pub dataset: crate::state::DatasetId,
    pub field: crate::state::FieldId,
    pub version: u64,
}
struct Entry {
    receipt: TaskReceipt,
    binding: Option<(DatasetId, ComputeKind, u64)>,
    permission: super::TaskCancellation,
    external_permission: bool,
}
#[derive(Default)]
pub struct ScientificTasks {
    entries: VecDeque<Entry>,
    consumed: BTreeSet<TaskId>,
    consumed_plans: BTreeSet<String>,
}
impl ScientificTasks {
    pub(crate) fn progress(&mut self, id: &TaskId, completed: usize, total: usize) {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|e| e.receipt.task_id == *id && !e.receipt.status.terminal())
        {
            entry.receipt.phase = "columns".into();
            entry.receipt.completed = Some(completed);
            entry.receipt.total = Some(total);
        }
    }

    pub(crate) fn set_produced(&mut self, id: &TaskId, produced: Vec<ResourceRef>) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.receipt.task_id == *id) {
            entry.receipt.produced = produced;
        }
    }
    pub fn was_consumed(&self, id: &TaskId) -> bool {
        self.consumed.contains(id)
    }
    pub fn bind_permission(&mut self, id: &TaskId, token: super::TaskCancellation) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.receipt.task_id == *id) {
            entry.permission = token;
            entry.external_permission = true;
        }
    }
    /// Also revoke leases held by disconnected clients. Receipts and submitted
    /// work outlive the connection that originally granted permission.
    pub fn revoke_external_permissions(&self) {
        for entry in &self.entries {
            if entry.external_permission && !entry.receipt.status.terminal() {
                entry.permission.cancel();
            }
        }
    }
    pub(crate) fn permitted(&self, id: &TaskId) -> bool {
        self.entries
            .iter()
            .find(|e| e.receipt.task_id == *id)
            .is_some_and(|e| !e.permission.is_cancelled())
    }
    pub fn plan_was_consumed(&self, plan_id: &str) -> bool {
        self.consumed_plans.contains(plan_id)
    }
    pub fn for_plan(&self, plan_id: &str) -> Option<&TaskReceipt> {
        self.entries
            .iter()
            .find(|e| e.receipt.plan_id.as_deref() == Some(plan_id))
            .map(|e| &e.receipt)
    }
    pub fn get(&self, id: &TaskId) -> Option<&TaskReceipt> {
        self.entries
            .iter()
            .find(|e| e.receipt.task_id == *id)
            .map(|e| &e.receipt)
    }
    pub fn list(&self, offset: usize, limit: usize) -> Vec<TaskReceipt> {
        self.entries
            .iter()
            .rev()
            .skip(offset)
            .take(limit.min(128))
            .map(|e| e.receipt.clone())
            .collect()
    }
    pub(crate) fn register(&mut self, receipt: TaskReceipt) -> bool {
        if !self.consumed.insert(receipt.task_id.clone()) {
            return false;
        }
        if let Some(id) = &receipt.plan_id {
            self.consumed_plans.insert(id.clone());
        }
        self.entries.push_back(Entry {
            receipt,
            binding: None,
            permission: super::TaskCancellation::default(),
            external_permission: false,
        });
        true
    }
    pub(crate) fn bind(
        &mut self,
        id: &TaskId,
        dataset: DatasetId,
        kind: ComputeKind,
        generation: u64,
    ) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.receipt.task_id == *id) {
            entry.binding = Some((dataset, kind, generation));
        }
    }
    pub(crate) fn find(
        &self,
        dataset: DatasetId,
        kind: ComputeKind,
        generation: u64,
    ) -> Option<TaskId> {
        self.entries
            .iter()
            .find(|e| e.binding == Some((dataset, kind, generation)))
            .map(|e| e.receipt.task_id.clone())
    }
    pub(crate) fn binding(&self, id: &TaskId) -> Option<(DatasetId, ComputeKind, u64)> {
        self.entries
            .iter()
            .find(|e| e.receipt.task_id == *id)
            .and_then(|e| e.binding)
    }
    pub(crate) fn update(
        &mut self,
        id: &TaskId,
        status: TaskStatus,
        revision: u64,
        code: Option<&str>,
        message: Option<String>,
        summary: Value,
    ) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.receipt.task_id == *id) {
            if entry.receipt.status.terminal() {
                return;
            }
            entry.receipt.status = status;
            entry.receipt.phase = format!("{status:?}").to_lowercase();
            entry.receipt.failure_code = code.map(str::to_owned);
            entry.receipt.message = message;
            entry.receipt.summary = summary;
            if status.terminal() {
                entry.receipt.end_revision = Some(DocumentRevision(revision));
            }
        }
        if status.terminal()
            && let Some(index) = self.entries.iter().position(|e| e.receipt.task_id == *id)
            && let Some(entry) = self.entries.remove(index)
        {
            self.entries.push_back(entry);
        }
        while self
            .entries
            .iter()
            .filter(|e| e.receipt.status.terminal())
            .count()
            > 128
        {
            if let Some(index) = self
                .entries
                .iter()
                .position(|e| e.receipt.status.terminal())
            {
                self.entries.remove(index);
            }
        }
    }
    pub(crate) fn invalidate_project(&mut self, revision: u64) {
        let ids = self
            .entries
            .iter()
            .filter(|e| !e.receipt.status.terminal())
            .map(|e| e.receipt.task_id.clone())
            .collect::<Vec<_>>();
        for id in ids {
            self.update(
                &id,
                TaskStatus::Stale,
                revision,
                Some("project_changed"),
                Some("The source project was replaced.".into()),
                Value::Null,
            );
        }
    }
}

#[cfg(test)]
#[path = "task_receipts_tests.rs"]
mod tests;
