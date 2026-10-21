use super::*;
use crate::operation::{
    Diagnostic, DiagnosticCode, OperationId, OperationKind, OperationReport, Severity,
};

impl PlotiumApp {
    /// Synchronous save retained for non-GUI callers and tests. The desktop
    /// shell uses the captured request on a background worker instead.
    pub fn save_project_to(
        &mut self,
        path: &std::path::Path,
        include_view_snapshots: bool,
    ) -> bool {
        let operation_id = self.session.begin_operation();
        let request = crate::project::prepare_project_save(self, path, include_view_snapshots);
        let identity = request.identity().clone();
        let persisted_revision = request.persisted_revision().to_owned();
        let result = crate::project::save_project_snapshot(request);
        self.complete_captured_project_save(
            operation_id,
            path,
            include_view_snapshots,
            &identity,
            &persisted_revision,
            result,
        )
    }

    /// Completion applies only to the project whose desktop snapshot was captured.
    pub fn complete_captured_project_save(
        &mut self,
        operation_id: OperationId,
        path: &std::path::Path,
        snapshots: bool,
        identity: &crate::project::SaveSnapshotIdentity,
        persisted_revision: &str,
        result: Result<crate::project::SaveOutcome, crate::project::ProjectError>,
    ) -> bool {
        if self.session.project_context != identity.project_context {
            return false;
        }
        if crate::project::save_result_phase(&result)
            == crate::project::SavePhase::InstalledUnconfirmed
        {
            // The installed file is now this recovery baseline, even though sync
            // failed. Keep dirty so later edits and the uncertainty remain visible.
            self.retain_installed_save_baseline(path, snapshots, persisted_revision);
        }
        self.complete_project_save(
            operation_id,
            path,
            snapshots,
            identity.edit_generation,
            result,
        )
    }

    /// Track a known installed snapshot without claiming all current edits are
    /// saved. Also used when connection revocation arrives after installation.
    pub fn retain_installed_save_baseline(
        &mut self,
        path: &std::path::Path,
        snapshots: bool,
        revision: &str,
    ) {
        self.doc.project_path = Some(path.to_owned());
        self.doc.project_revision = Some(revision.into());
        self.doc.save_include_view_snapshots = snapshots;
        self.doc.dirty = true;
    }

    /// Project a worker result back into the live document. A successful save
    /// only clears `dirty` when no edit landed after the worker captured state.
    pub fn complete_project_save(
        &mut self,
        operation_id: OperationId,
        path: &std::path::Path,
        include_view_snapshots: bool,
        captured_generation: u64,
        result: Result<crate::project::SaveOutcome, crate::project::ProjectError>,
    ) -> bool {
        match result {
            Ok(outcome) => {
                self.doc.project_path = Some(path.to_owned());
                self.doc.save_include_view_snapshots = include_view_snapshots;
                self.settings.export.include_view_snapshots = include_view_snapshots;
                if self.doc.edit_generation == captured_generation {
                    self.doc.dirty = false;
                }
                self.doc.project_revision = Some(outcome.revision.clone());
                let mut report = OperationReport::success(
                    operation_id,
                    OperationKind::ProjectSave,
                    format!("Saved project {}", path.display()),
                    (),
                )
                .with_diagnostic(
                    Diagnostic::new(
                        Severity::Info,
                        DiagnosticCode::ProjectSaveSucceeded,
                        "Project saved successfully.",
                    )
                    .with_source("core.project")
                    .with_context("path", path.display().to_string()),
                );
                if let Some(warning) = outcome.backup_warning {
                    report = report.with_diagnostic(
                        Diagnostic::new(
                            Severity::Warning,
                            DiagnosticCode::ProjectSaveSucceeded,
                            "The project was saved, but its backup could not be hidden.",
                        )
                        .with_source("core.project.backup")
                        .with_context("error", warning),
                    );
                }
                self.session.record_operation(report);
                self.persist_settings();
                self.note_recent_file(path);
                true
            }
            Err(error) => {
                self.session
                    .record_operation(OperationReport::<()>::failure(
                        operation_id,
                        OperationKind::ProjectSave,
                        format!("Save failed: {error}"),
                        Diagnostic::new(
                            Severity::Error,
                            DiagnosticCode::ProjectSaveFailed,
                            "Project could not be saved.",
                        )
                        .with_source("core.project")
                        .with_context("path", path.display().to_string())
                        .with_context("error", error.to_string()),
                    ));
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_completion_does_not_clear_a_newer_edit() {
        let mut app = PlotiumApp::new_with_settings(crate::settings::Settings::default());
        app.mark_document_dirty();
        let captured = app.doc.edit_generation;
        app.mark_document_dirty();
        let path = std::env::temp_dir().join(format!(
            "plotium-generation-save-{}.plotium",
            uuid::Uuid::new_v4()
        ));
        let request = crate::project::prepare_project_save(&app, &path, false);
        let outcome = crate::project::save_project_snapshot(request);
        let operation = app.session.begin_operation();

        assert!(app.complete_project_save(operation, &path, false, captured, outcome));
        assert!(app.doc.dirty);
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod completion_identity_tests {
    use super::*;
    use crate::project::{ProjectError, SaveOutcome};
    #[test]
    fn late_committed_completion_cannot_install_into_equal_revision_new_project() {
        let mut app = PlotiumApp::new_with_settings(Default::default());
        app.start_new_project();
        let path = std::path::Path::new("old.plotium");
        let request = crate::project::prepare_project_save(&app, path, false);
        let identity = request.identity().clone();
        let persisted = request.persisted_revision().to_owned();
        app.start_new_project();
        let operation = app.session.begin_operation();
        assert!(!app.complete_captured_project_save(
            operation,
            path,
            false,
            &identity,
            &persisted,
            Ok(SaveOutcome {
                revision: persisted.clone(),
                backup_warning: None
            })
        ));
        assert!(app.doc.project_path.is_none());
        assert!(!app.doc.dirty);
        assert_eq!(app.doc.automation_revision, identity.document_revision);
    }
    #[test]
    fn installed_unconfirmed_keeps_dirty_and_updates_recovery_baseline() {
        let mut app = PlotiumApp::new_with_settings(Default::default());
        app.start_new_project();
        let path = std::path::Path::new("unconfirmed.plotium");
        let request = crate::project::prepare_project_save(&app, path, false);
        let identity = request.identity().clone();
        let operation = app.session.begin_operation();
        assert!(!app.complete_captured_project_save(
            operation,
            path,
            false,
            &identity,
            "installed-revision",
            Err(ProjectError::InstalledUnconfirmed(
                "injected sync failure".into()
            ))
        ));
        assert!(app.doc.dirty);
        assert_eq!(app.doc.project_path.as_deref(), Some(path));
        assert_eq!(
            app.doc.project_revision.as_deref(),
            Some("installed-revision")
        );
        assert!(app.session.status.contains("installed"));
    }
}
