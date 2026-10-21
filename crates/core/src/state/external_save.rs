//! Desktop-owned save requests and receipts; never part of project persistence.
use super::{PlotiumApp, ProjectContextId};
use crate::project::{
    self, ProjectSaveRequest, SaveAuthorization, SavePhase, SaveSnapshotIdentity, SaveTarget,
};
use serde::Serialize;
use std::{collections::VecDeque, path::PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalSaveStatus {
    AwaitingAuthorization,
    Queued,
    Saving,
    Completed,
}
#[derive(Clone, Debug, Serialize)]
pub struct SaveReceipt {
    pub save_id: String,
    pub project_context: ProjectContextId,
    pub path: PathBuf,
    pub expected_revision: u64,
    pub include_view_snapshots: bool,
    pub overwrite_required: bool,
    pub status: ExternalSaveStatus,
    pub phase: Option<SavePhase>,
    pub snapshot: Option<SaveSnapshotIdentity>,
    pub persisted_revision: Option<String>,
    pub state_applied: bool,
    pub dirty_after_completion: Option<bool>,
    pub error_code: Option<String>,
    pub message: String,
}
struct ExternalSave {
    receipt: SaveReceipt,
    authorization: SaveAuthorization,
}
#[derive(Default)]
pub struct ExternalSaves {
    requests: VecDeque<ExternalSave>,
}
impl ExternalSaves {
    pub fn receipts(&self) -> impl Iterator<Item = &SaveReceipt> {
        self.requests.iter().map(|s| &s.receipt)
    }
    pub fn receipt(&self, id: &str) -> Option<&SaveReceipt> {
        self.receipts().find(|r| r.save_id == id)
    }
    pub fn pending(&self) -> Option<&SaveReceipt> {
        self.receipts()
            .find(|r| r.status == ExternalSaveStatus::AwaitingAuthorization)
    }
    pub fn busy(&self) -> bool {
        self.receipts()
            .any(|r| r.status != ExternalSaveStatus::Completed)
    }
    pub fn reconcile(&mut self) {
        for saved in &mut self.requests {
            if saved.authorization.is_revoked()
                && matches!(
                    saved.receipt.status,
                    ExternalSaveStatus::AwaitingAuthorization | ExternalSaveStatus::Queued
                )
            {
                finish_denied(
                    &mut saved.receipt,
                    "request_cancelled",
                    "Connection or project stopped; target not installed. Inspect state before requesting another save.",
                );
            }
        }
    }
    pub fn revoke(&mut self, reason: &str) {
        for request in &mut self.requests {
            if request.receipt.status != ExternalSaveStatus::Completed {
                request.authorization.revoke();
                if request.receipt.status != ExternalSaveStatus::Saving {
                    finish_denied(&mut request.receipt, "request_cancelled", reason);
                }
            }
        }
    }
    fn find_mut(&mut self, id: &str) -> Option<&mut ExternalSave> {
        self.requests.iter_mut().find(|r| r.receipt.save_id == id)
    }
}
fn finish_denied(receipt: &mut SaveReceipt, code: &str, message: &str) {
    receipt.status = ExternalSaveStatus::Completed;
    receipt.phase = Some(SavePhase::NotInstalled);
    receipt.error_code = Some(code.into());
    receipt.message = message.into();
}
impl PlotiumApp {
    pub fn request_external_save(
        &mut self,
        id: String,
        target: SaveTarget,
        revision: u64,
        snapshots: bool,
    ) -> Result<SaveAuthorization, String> {
        if self.session.external_saves.busy() || self.session.ui.project_save_in_progress {
            return Err("Wait for the pending save".into());
        }
        if revision != self.doc.automation_revision {
            return Err("Document revision changed; replan".into());
        }
        let authorization = SaveAuthorization::pending(target.clone());
        while self.session.external_saves.requests.len() >= 16 {
            self.session.external_saves.requests.pop_front();
        }
        self.session.external_saves.requests.push_back(ExternalSave {
            receipt: SaveReceipt {
                save_id: id, project_context: self.session.project_context, path: target.path.clone(), expected_revision: revision,
                include_view_snapshots: snapshots, overwrite_required: target.exists(), status: ExternalSaveStatus::AwaitingAuthorization,
                phase: None, snapshot: None, persisted_revision: None, state_applied: false, dirty_after_completion: None, error_code: None,
                message: "Approve this exact path in Plotium; an existing file also requires replacement confirmation. No file has been written.".into(),
            }, authorization: authorization.clone(),
        });
        Ok(authorization)
    }
    pub fn decide_external_save(&mut self, id: &str, allow: bool, replace: bool) {
        let busy = self.session.ui.project_save_in_progress;
        let context = self.session.project_context;
        let revision = self.doc.automation_revision;
        let Some(request) = self.session.external_saves.find_mut(id) else {
            return;
        };
        if request.receipt.status != ExternalSaveStatus::AwaitingAuthorization {
            return;
        }
        if !allow {
            request.authorization.revoke();
            finish_denied(
                &mut request.receipt,
                "permission_denied",
                "Save denied in Plotium; target not installed. Ask before requesting another save.",
            );
            return;
        }
        if busy {
            request.authorization.revoke();
            finish_denied(
                &mut request.receipt,
                "busy",
                "Another desktop save started; wait for its result before requesting another save.",
            );
            return;
        }
        if context != request.receipt.project_context
            || revision != request.receipt.expected_revision
        {
            request.authorization.revoke();
            finish_denied(
                &mut request.receipt,
                "stale_revision",
                "Project changed before authorization; rediscover and replan.",
            );
            return;
        }
        if let Err(error) = request.authorization.grant(replace) {
            request.authorization.revoke();
            finish_denied(
                &mut request.receipt,
                "permission_denied",
                &error.to_string(),
            );
            return;
        }
        request.receipt.status = ExternalSaveStatus::Queued;
        request.receipt.message = "Authorized; waiting for the desktop save worker.".into();
        let path = request.receipt.path.clone();
        let snapshots = request.receipt.include_view_snapshots;
        self.queue_project_save(path, snapshots, false);
        self.session
            .ui
            .pending_project_save
            .as_mut()
            .expect("just queued")
            .external_save_id = Some(id.into());
    }
    pub fn capture_external_save(&mut self, id: &str) -> Option<ProjectSaveRequest> {
        let saved = self.session.external_saves.find_mut(id)?;
        if saved.receipt.status != ExternalSaveStatus::Queued {
            return None;
        }
        if !saved.authorization.is_valid()
            || saved.receipt.project_context != self.session.project_context
            || saved.receipt.expected_revision != self.doc.automation_revision
        {
            saved.authorization.revoke();
            finish_denied(
                &mut saved.receipt,
                "request_cancelled",
                "Save authorization or project changed before snapshot capture; target not installed.",
            );
            return None;
        }
        let path = saved.receipt.path.clone();
        let snapshots = saved.receipt.include_view_snapshots;
        let authorization = saved.authorization.clone();
        let mut request = project::prepare_project_save(self, &path, snapshots);
        request.authorize(authorization);
        let saved = self
            .session
            .external_saves
            .find_mut(id)
            .expect("same request");
        saved.receipt.snapshot = Some(request.identity().clone());
        saved.receipt.persisted_revision = Some(request.persisted_revision().into());
        saved.receipt.status = ExternalSaveStatus::Saving;
        saved.receipt.message =
            "Saving captured desktop snapshot; completion is not yet confirmed.".into();
        Some(request)
    }
    pub fn external_save_can_apply(&self, id: &str) -> bool {
        self.session.external_saves.requests.iter().any(|r| {
            r.receipt.save_id == id
                && r.receipt.project_context == self.session.project_context
                && r.authorization.is_valid()
        })
    }
    pub fn finish_external_save(
        &mut self,
        id: &str,
        phase: SavePhase,
        applied: bool,
        error: Option<String>,
    ) {
        let dirty = self.doc.dirty;
        let Some(saved) = self.session.external_saves.find_mut(id) else {
            return;
        };
        saved.authorization.revoke();
        saved.receipt.status = ExternalSaveStatus::Completed;
        saved.receipt.phase = Some(phase);
        saved.receipt.state_applied = applied;
        saved.receipt.dirty_after_completion = applied.then_some(dirty);
        saved.receipt.error_code = error.as_ref().map(|_| "save_failed".into());
        saved.receipt.message = match phase {
            SavePhase::Committed => "Captured snapshot durably committed. Later edits may remain unsaved. Undo does not restore disk files.".into(),
            SavePhase::NotInstalled => format!("Target not installed. Resolve the failure, inspect current state, then request fresh authorization. {}", error.unwrap_or_default()),
            SavePhase::InstalledUnconfirmed => format!("Snapshot installed at the target, but durable completion is unconfirmed. Keep the project open; inspect the target and storage before an explicitly authorized retry. {}", error.unwrap_or_default()),
        };
        if saved.receipt.project_context == self.session.project_context {
            self.session.status = format!(
                "{}: {}",
                saved.receipt.path.display(),
                saved.receipt.message
            );
        }
    }
}
