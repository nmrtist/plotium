/// Ephemeral identity of one installed project; never serialized in project files.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct ProjectContextId(uuid::Uuid);
impl std::fmt::Display for ProjectContextId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl Default for ProjectContextId {
    fn default() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

use super::{Document, PlotiumApp};

/// A document-level operation that replaces or removes the current project.
/// File imports are deliberately absent: they add to the current project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectTransition {
    New,
    Close,
    Open(std::path::PathBuf),
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectTransitionPhase {
    /// A clean Quit must recheck after UI/compute polls and outstanding saves.
    CheckUnsaved,
    NeedsConfirmation,
    Saving,
    Ready,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingProjectTransition {
    pub target: ProjectTransition,
    pub phase: ProjectTransitionPhase,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingProjectSave {
    pub path: std::path::PathBuf,
    pub include_view_snapshots: bool,
    /// Continue the pending project transition only after this save succeeds.
    pub continue_transition: bool,
    pub external_save_id: Option<String>,
}

impl PlotiumApp {
    /// Record one persisted document mutation. The monotonically increasing
    /// generation lets background saves distinguish their captured revision
    /// from edits that happen while they are running.
    pub fn mark_document_dirty(&mut self) {
        self.doc.mark_dirty();
    }

    pub fn request_project_transition(&mut self, target: ProjectTransition) {
        if let Some(edit) = self.session.ui.table_cell_edit.as_mut() {
            let message = "Apply or discard the unfinished cell edit before leaving this project.";
            edit.error = Some(message.into());
            self.session.status = message.into();
            self.session.ui.project_transition = None;
            self.session.ui.sheet_open = self
                .doc
                .datasets
                .iter()
                .position(|dataset| dataset.resource_id() == edit.dataset);
            return;
        }
        self.session.external_exports.revoke();
        self.session
            .external_saves
            .revoke("Project transition requested; save authorization revoked.");
        let phase = if self.doc.dirty {
            ProjectTransitionPhase::NeedsConfirmation
        } else if target == ProjectTransition::Quit {
            ProjectTransitionPhase::CheckUnsaved
        } else {
            ProjectTransitionPhase::Ready
        };
        self.session.ui.project_transition = Some(PendingProjectTransition { target, phase });
    }

    pub fn queue_project_save(
        &mut self,
        path: std::path::PathBuf,
        include_view_snapshots: bool,
        continue_transition: bool,
    ) {
        self.session.ui.pending_project_save = Some(PendingProjectSave {
            path,
            include_view_snapshots,
            continue_transition,
            external_save_id: None,
        });
        self.session.ui.project_save_in_progress = true;
        if continue_transition && let Some(transition) = self.session.ui.project_transition.as_mut()
        {
            transition.phase = ProjectTransitionPhase::Saving;
        }
    }

    pub fn start_new_project(&mut self) {
        let fresh = Self::new_with_settings(self.settings.clone());
        self.install_loaded_project(fresh);
        self.session.status = "New project ready.".to_owned();
    }

    pub fn close_project(&mut self) {
        let fresh = Self::new_with_settings(self.settings.clone());
        self.install_loaded_project(fresh);
        self.session.status = "Project closed. Empty project ready.".to_owned();
    }
}

impl Document {
    pub(crate) fn mark_dirty(&mut self) {
        self.edit_generation = self
            .edit_generation
            .checked_add(1)
            .expect("document edit generation exhausted");
        self.dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirty_transition_waits_for_confirmation() {
        let mut app = PlotiumApp::new_with_settings(crate::settings::Settings::default());
        app.mark_document_dirty();
        app.request_project_transition(ProjectTransition::Close);

        let pending = app.session.ui.project_transition.as_ref().unwrap();
        assert_eq!(pending.target, ProjectTransition::Close);
        assert_eq!(pending.phase, ProjectTransitionPhase::NeedsConfirmation);
    }

    #[test]
    fn clean_transition_is_ready_immediately() {
        let mut app = PlotiumApp::new_with_settings(crate::settings::Settings::default());
        app.request_project_transition(ProjectTransition::New);

        assert_eq!(
            app.session.ui.project_transition.unwrap().phase,
            ProjectTransitionPhase::Ready
        );
    }

    #[test]
    fn clean_quit_defers_unsaved_check_until_the_shell_finishes_polling() {
        let mut app = PlotiumApp::new_with_settings(crate::settings::Settings::default());
        app.request_project_transition(ProjectTransition::Quit);
        assert_eq!(
            app.session.ui.project_transition.unwrap().phase,
            ProjectTransitionPhase::CheckUnsaved
        );
    }

    #[test]
    fn startup_new_and_close_have_clean_editable_empty_projects() {
        let mut app = PlotiumApp::new_with_settings(crate::settings::Settings::default());
        for reset in [
            None,
            Some(ProjectTransition::New),
            Some(ProjectTransition::Close),
        ] {
            let context = app.session.project_context;
            match reset {
                Some(ProjectTransition::New) => app.start_new_project(),
                Some(ProjectTransition::Close) => app.close_project(),
                _ => {}
            }
            if reset.is_some() {
                assert_ne!(app.session.project_context, context);
            }

            assert!(app.session.active_canvas.is_none());
            assert!(app.doc.datasets.is_empty() && app.doc.canvases.is_empty());
            assert!(app.doc.assets.is_empty() && app.doc.automation_runs.is_empty());
            assert!(app.doc.project_path.is_none() && app.doc.project_revision.is_none());
            assert!(!app.doc.dirty);
            assert_eq!(app.doc.edit_generation, 0);
            assert_eq!(app.doc.automation_revision, 0);
            assert!(app.session.undo_stack.is_empty() && app.session.redo_stack.is_empty());
            app.request_project_transition(ProjectTransition::Quit);
            assert_eq!(
                app.session.ui.project_transition.as_ref().unwrap().phase,
                ProjectTransitionPhase::CheckUnsaved
            );
            // Even content-free edits must still prompt before they are discarded.
            app.mark_document_dirty();
            app.request_project_transition(ProjectTransition::Close);
            assert_eq!(
                app.session.ui.project_transition.as_ref().unwrap().phase,
                ProjectTransitionPhase::NeedsConfirmation
            );
        }
    }

    #[test]
    fn opening_replaces_context_but_failed_open_preserves_empty_project() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.plotium");
        let mut app = PlotiumApp::new_with_settings(Default::default());
        crate::project::save_project_snapshot(crate::project::prepare_project_save(
            &app, &path, false,
        ))
        .unwrap();
        let initial = app.session.project_context;
        assert!(app.load_project_from(&path));
        assert_ne!(app.session.project_context, initial);

        assert_eq!(app.doc.project_path.as_deref(), Some(path.as_path()));
        assert!(!app.doc.dirty);
        assert!(app.session.active_canvas.is_none());
        app.close_project();
        let context = app.session.project_context;
        assert!(!app.load_project_from(&dir.path().join("missing.plotium")));
        assert_eq!(app.session.project_context, context);

        assert!(app.doc.project_path.is_none());
        assert!(!app.doc.dirty);
        assert!(app.session.undo_stack.is_empty());
    }
}
