//! Frame driving and native application lifecycle.

use super::*;

pub(super) fn phase_after_polls(
    phase: plotium_core::state::ProjectTransitionPhase,
    dirty: bool,
    save_busy: bool,
) -> plotium_core::state::ProjectTransitionPhase {
    use plotium_core::state::ProjectTransitionPhase;
    if phase != ProjectTransitionPhase::CheckUnsaved || save_busy {
        return phase;
    }
    if dirty {
        ProjectTransitionPhase::NeedsConfirmation
    } else {
        ProjectTransitionPhase::Ready
    }
}

// Workers must be drained even when eframe skips ui() for a hidden window.
// Apply document results before save completion so generation checks see them.
fn poll_background_work(
    app: &mut PlotiumApp,
    save_job: &mut Option<ManualProjectSaveJob>,
    ctx: &egui::Context,
) {
    if app.poll_external_export() {
        ctx.request_repaint_after(Duration::from_millis(50));
    }
    app.poll_compute();
    if app.poll_data_import() {
        ctx.request_repaint_after(Duration::from_millis(16));
    }
    let fitting = app.poll_line_fit() | app.poll_xps_fit();
    let symmetry = app.poll_symmetry_audit();
    let transforming = app.poll_table_transform();
    if fitting || symmetry || transforming {
        ctx.request_repaint_after(Duration::from_millis(50));
    }
    if app.compute_busy() {
        ctx.request_repaint_after(Duration::from_millis(33));
    }
    poll_manual_project_save(app, save_job, ctx);
}

pub(super) fn poll_manual_project_save(
    app: &mut PlotiumApp,
    save_job: &mut Option<ManualProjectSaveJob>,
    ctx: &egui::Context,
) {
    let finished = save_job
        .as_ref()
        .is_some_and(|job| job.handle.is_finished());
    if !finished {
        if save_job.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        return;
    }
    let job = save_job
        .take()
        .expect("finished project save worker is present");
    let result = match job.handle.join() {
        Ok(result) => result,
        Err(_) => match job.progress.phase() {
            plotium_core::project::SavePhase::NotInstalled => {
                Err(plotium_core::project::ProjectError::Invalid(
                    "project save worker failed before target installation".into(),
                ))
            }
            plotium_core::project::SavePhase::InstalledUnconfirmed => {
                Err(plotium_core::project::ProjectError::InstalledUnconfirmed(
                    "project save worker failed after target installation".into(),
                ))
            }
            plotium_core::project::SavePhase::Committed => Ok(plotium_core::project::SaveOutcome {
                revision: job.persisted_revision.clone(),
                backup_warning: Some(
                    "Save worker ended after durable commit; inspect diagnostics.".into(),
                ),
            }),
        },
    };
    let phase = plotium_core::project::save_result_phase(&result);
    let error = result.as_ref().err().map(ToString::to_string);
    let same_project = app.session.project_context == job.identity.project_context;
    let apply = same_project
        && job
            .external_save_id
            .as_ref()
            .is_none_or(|id| app.external_save_can_apply(id));
    let saved = if apply {
        app.complete_captured_project_save(
            job.operation_id,
            &job.path,
            job.include_view_snapshots,
            &job.identity,
            &job.persisted_revision,
            result,
        )
    } else {
        false
    };
    if same_project && !apply && phase != plotium_core::project::SavePhase::NotInstalled {
        // Stop cannot undo an already installed file. Keep its recovery baseline
        // truthful, but never clear dirty after revoked authorization.
        app.retain_installed_save_baseline(
            &job.path,
            job.include_view_snapshots,
            &job.persisted_revision,
        );
    }
    if let Some(id) = &job.external_save_id {
        app.finish_external_save(
            id,
            phase,
            same_project && phase != plotium_core::project::SavePhase::NotInstalled,
            error,
        );
    }
    if !same_project {
        ctx.request_repaint();
        return;
    }
    app.session.ui.project_save_in_progress = false;
    if job.continue_transition {
        if let Some(transition) = app.session.ui.project_transition.as_mut() {
            transition.phase = if transition_ready_after_save(
                saved,
                job.captured_generation,
                app.doc.edit_generation,
            ) {
                plotium_core::state::ProjectTransitionPhase::Ready
            } else {
                plotium_core::state::ProjectTransitionPhase::NeedsConfirmation
            };
        }
    } else if !saved && job.external_save_id.is_none() {
        app.session.ui.save_project_options = true;
    }
    ctx.request_repaint();
}

impl eframe::App for Shell {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(target_os = "macos")]
        {
            // Native menus and Dock events must also work with no visible window.
            if self.pending_recovery.is_none() {
                self.open_documents.poll(&mut self.app, ctx);
                self.native_menu
                    .poll(&mut self.app, &mut self.clipboard_table_paste, ctx);
            }
            // Fullscreen transitions can make the root temporarily non-visible:
            // eframe still calls logic, but skips ui and would otherwise exit.
            ui::handle_close_request(&mut self.app, ctx);
        }
        poll_background_work(&mut self.app, &mut self.manual_save_job, ctx);
        // A finished recovery snapshot may be holding a queued manual save.
        self.tick_recovery(ctx);
        self.start_pending_project_save(ctx);
        self.agent_connections.poll(
            &mut self.app,
            ctx,
            self.pending_recovery.is_some() || self.manual_save_job.is_some(),
        );
    }

    #[cfg(windows)]
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        ui::clipboard_native::restore_missing_paste_shortcut(raw_input);
    }
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        observability::show_pending_crash_dialog();
        self.scale.drive(&mut self.app, &ctx, frame);
        let ribbon_chrome = ui::current_ribbon_chrome(&ctx, frame);
        let recovery_blocked = self.pending_recovery.is_some();
        if !recovery_blocked && let Some(driver) = &mut self.shot {
            driver.drive(&mut self.app, &ctx);
        }
        self.show_recovery_prompt(&ctx);
        if self.app.session.updates.tick() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        } else if let Some(delay) = self.app.session.updates.next_check_delay() {
            ctx.request_repaint_after(delay.max(std::time::Duration::from_millis(100)));
        }
        if let plotium_core::update::UpdateStatus::Installed { plan, .. } =
            self.app.session.updates.status()
        {
            *PENDING_INSTALL.lock().unwrap() = Some(plan.clone());
        }
        ui::render(
            &mut self.app,
            &mut self.clipboard_table_paste,
            &mut self.batch_workflow,
            &mut self.agent_connections,
            &mut self.last_window_title,
            ui,
            recovery_blocked,
            ribbon_chrome,
        );
        // Apply save completion after every edit-producing poll. This makes the
        // generation check below cover compute results that arrived this frame.
        poll_manual_project_save(&mut self.app, &mut self.manual_save_job, &ctx);
        self.start_pending_project_save(&ctx);
        self.agent_connections.poll(
            &mut self.app,
            &ctx,
            recovery_blocked || self.manual_save_job.is_some(),
        );
        self.drive_project_transition(&ctx);
        if self.app.data_export_busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }
        self.tick_recovery(&ctx);
    }

    #[cfg(windows)]
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.shutdown();
    }

    #[cfg(not(windows))]
    fn on_exit(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plotium_core::state::ProjectTransitionPhase::*;

    #[test]
    fn clean_quit_checks_changes_after_polls_and_save_barriers() {
        assert_eq!(phase_after_polls(CheckUnsaved, false, false), Ready);
        assert_eq!(
            phase_after_polls(CheckUnsaved, true, false),
            NeedsConfirmation
        );
        for dirty in [false, true] {
            assert_eq!(phase_after_polls(CheckUnsaved, dirty, true), CheckUnsaved);
        }
        let waiting = phase_after_polls(CheckUnsaved, false, true);
        // A background result arrives while the recovery/manual save finishes.
        assert_eq!(phase_after_polls(waiting, true, false), NeedsConfirmation);
    }

    #[test]
    fn confirmed_discard_and_save_phases_are_not_rechecked() {
        for phase in [Ready, Saving, NeedsConfirmation] {
            for save_busy in [false, true] {
                assert_eq!(phase_after_polls(phase, true, save_busy), phase);
            }
        }
    }

    #[test]
    fn nonvisual_poll_finishes_save_and_unblocks_project_requests() {
        let mut app = PlotiumApp::new_with_settings(Default::default());
        app.start_new_project();
        let ctx = egui::Context::default();
        let (send, receive) = std::sync::mpsc::channel();
        let mut save_job = Some(ManualProjectSaveJob {
            operation_id: app.session.begin_operation(),
            path: "test.plotium".into(),
            include_view_snapshots: false,
            captured_generation: app.doc.edit_generation,
            identity: plotium_core::project::SaveSnapshotIdentity {
                project_context: app.session.project_context,
                document_revision: app.doc.automation_revision,
                edit_generation: app.doc.edit_generation,
            },
            persisted_revision: "test".into(),
            progress: Default::default(),
            external_save_id: None,
            continue_transition: false,
            handle: std::thread::spawn(move || {
                receive.recv().unwrap();
                Err(plotium_core::project::ProjectError::Invalid(
                    "test failure".into(),
                ))
            }),
        });
        app.session.ui.project_save_in_progress = true;
        let mut session = plotium_agent::Session::default();
        poll_background_work(&mut app, &mut save_job, &ctx);
        assert!(save_job.is_some());
        assert_eq!(
            session
                .call(&mut app, "plotium_get_state", serde_json::json!({}))
                .unwrap()["availability"]["project_read"]["reason"]["code"],
            "busy"
        );
        send.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !save_job.as_ref().unwrap().handle.is_finished() {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        // No ui() call is needed to drain the completed job and its busy flag.
        poll_background_work(&mut app, &mut save_job, &ctx);
        assert!(save_job.is_none());
        assert!(!app.session.ui.project_save_in_progress);
        assert!(app.session.ui.save_project_options);
        assert!(
            session
                .call(&mut app, "plotium_get_state", serde_json::json!({}))
                .is_ok()
        );
    }

    #[test]
    fn nonvisual_poll_drains_import_completion_and_unblocks_project_requests() {
        let mut app = PlotiumApp::new_with_settings(Default::default());
        app.start_new_project();
        let ctx = egui::Context::default();
        let mut save_job = None;
        let mut session = plotium_agent::Session::default();
        app.queue_data_import("test".into(), || Err("test import failure".into()));
        assert_eq!(
            session
                .call(&mut app, "plotium_get_state", serde_json::json!({}))
                .unwrap()["availability"]["project_read"]["reason"]["code"],
            "busy"
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.session.data_imports.is_pending() {
            assert!(Instant::now() < deadline);
            poll_background_work(&mut app, &mut save_job, &ctx);
            std::thread::yield_now();
        }
        assert!(
            session
                .call(&mut app, "plotium_get_state", serde_json::json!({}))
                .is_ok()
        );
        assert!(app.session.ui.settings_dialog.is_none());
    }
}

#[cfg(test)]
mod save_lifecycle_tests {
    use super::*;
    #[test]
    fn installed_external_save_after_stop_retains_dirty_baseline_but_never_crosses_projects() {
        for change_project in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("captured.plotium");
            let mut app = PlotiumApp::new_with_settings(Default::default());
            app.start_new_project();
            let target = plotium_core::project::SaveTarget::inspect(&path).unwrap();
            let path = target.path.clone();
            let authorization = app
                .request_external_save("save".into(), target, 0, false)
                .unwrap();
            app.decide_external_save("save", true, false);
            let request = app.capture_external_save("save").unwrap();
            let identity = request.identity().clone();
            let persisted = request.persisted_revision().to_owned();
            let progress = request.progress();
            let (send, receive) = std::sync::mpsc::channel();
            let mut job = Some(ManualProjectSaveJob {
                operation_id: app.session.begin_operation(),
                path: path.clone(),
                include_view_snapshots: false,
                captured_generation: identity.edit_generation,
                identity,
                persisted_revision: persisted.clone(),
                progress,
                external_save_id: Some("save".into()),
                continue_transition: false,
                handle: std::thread::spawn(move || {
                    let result = plotium_core::project::save_project_snapshot(request);
                    send.send(()).unwrap();
                    result
                }),
            });
            receive.recv_timeout(Duration::from_secs(5)).unwrap();
            authorization.revoke();
            if change_project {
                app.start_new_project();
            }
            while !job.as_ref().unwrap().handle.is_finished() {
                std::thread::yield_now();
            }
            poll_manual_project_save(&mut app, &mut job, &egui::Context::default());
            let receipt = app.session.external_saves.receipt("save").unwrap();
            assert_eq!(
                receipt.phase,
                Some(plotium_core::project::SavePhase::Committed)
            );
            assert_eq!(receipt.state_applied, !change_project);
            assert!(path.exists());
            if change_project {
                assert!(app.doc.project_path.is_none());
                assert!(!app.doc.dirty);
            } else {
                assert_eq!(
                    app.doc.project_revision.as_deref(),
                    Some(persisted.as_str())
                );
                assert!(app.doc.dirty);
            }
        }
    }
}
