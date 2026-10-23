use super::*;
use plotium_core::state::{ProjectTransition, ProjectTransitionPhase};

fn app(dirty: bool) -> PlotiumApp {
    let mut app = PlotiumApp::new_with_settings(plotium_core::settings::Settings::default());
    if dirty {
        app.mark_document_dirty();
    }
    app
}

fn commands(output: &egui::FullOutput) -> &[egui::ViewportCommand] {
    &output.viewport_output[&egui::ViewportId::ROOT].commands
}

#[test]
fn native_close_and_restart_preserve_a_clean_projects_unfinished_cell_edit() {
    use plotium_core::data::LiteralValue;
    use plotium_core::state::TableCellEditDraft;

    for (macos, restarting) in [(false, false), (true, true)] {
        let ctx = egui::Context::default();
        let mut app = app(false);
        app.new_table_dataset();
        app.doc.dirty = false;
        let table = app.doc.datasets[0].as_table().unwrap();
        let revision = &table.typed_state.envelope.revision;
        app.session.ui.table_cell_edit = Some(TableCellEditDraft {
            dataset: table.resource_id,
            table: revision.table_id,
            revision: revision.id,
            row: table.typed_rows(1, &[]).unwrap().row_ids[0],
            column: revision.snapshot.schema.columns[0].id,
            original: LiteralValue::Null,
            original_text: "0".into(),
            text: "unfinished".into(),
            error: None,
            take_focus: false,
        });
        for id in [
            crate::ui::commands::CommandId::NewProject,
            crate::ui::commands::CommandId::OpenProject,
            crate::ui::commands::CommandId::CloseProject,
            crate::ui::commands::CommandId::Quit,
        ] {
            let descriptor = crate::ui::commands::describe(&app, id);
            assert!(!descriptor.enabled);
            assert!(
                descriptor
                    .disabled_reason
                    .unwrap()
                    .contains("Apply or discard")
            );
        }
        let output = ctx.run_ui(Default::default(), |ui| {
            handle_root_close(&mut app, ui.ctx(), macos, restarting);
        });
        assert!(commands(&output).contains(&egui::ViewportCommand::CancelClose));
        assert!(!commands(&output).contains(&egui::ViewportCommand::Close));
        assert!(!app.session.allow_close);
        assert!(app.session.ui.project_transition.is_none());
        let draft = app.session.ui.table_cell_edit.as_ref().unwrap();
        assert_eq!(draft.text, "unfinished");
        assert!(draft.error.as_ref().unwrap().contains("Apply or discard"));
        assert!(!app.doc.dirty);
    }
}

#[test]
fn macos_window_close_retains_clean_and_dirty_workspaces_without_quitting() {
    for dirty in [false, true] {
        let ctx = egui::Context::default();
        let mut app = app(dirty);
        let generation = app.doc.edit_generation;
        let output = ctx.run_ui(Default::default(), |ui| {
            handle_root_close(&mut app, ui.ctx(), true, false);
        });
        assert!(commands(&output).contains(&egui::ViewportCommand::CancelClose));
        assert!(commands(&output).contains(&egui::ViewportCommand::Visible(false)));
        assert!(app.session.ui.project_transition.is_none());
        assert!(!app.session.allow_close);
        assert_eq!(app.doc.dirty, dirty);
        assert_eq!(app.doc.edit_generation, generation);
    }
}

#[test]
fn native_close_guard_only_intercepts_unapproved_close_events() {
    for (close_event, allowed) in [(false, false), (true, true)] {
        let ctx = egui::Context::default();
        let mut app = app(true);
        app.session.allow_close = allowed;
        let mut input = egui::RawInput::default();
        if close_event {
            input
                .viewports
                .get_mut(&egui::ViewportId::ROOT)
                .unwrap()
                .events
                .push(egui::ViewportEvent::Close);
        }
        let output = ctx.run_ui(input, |ui| handle_close_request(&mut app, ui.ctx()));
        assert!(!commands(&output).contains(&egui::ViewportCommand::CancelClose));
        assert!(!commands(&output).contains(&egui::ViewportCommand::Visible(false)));
    }
}

#[cfg(target_os = "macos")]
#[test]
fn macos_logic_vetoes_close_even_without_a_visible_ui() {
    let ctx = egui::Context::default();
    let mut app = app(true);
    let mut input = egui::RawInput::default();
    let viewport = input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap();
    viewport.events.push(egui::ViewportEvent::Close);
    viewport.minimized = Some(true);
    let output = ctx.run_ui(input, |ui| {
        // Only the logic hook runs for an invisible root; no application UI.
        handle_close_request(&mut app, ui.ctx());
    });
    assert!(commands(&output).contains(&egui::ViewportCommand::CancelClose));
    assert!(commands(&output).contains(&egui::ViewportCommand::Visible(false)));
    assert!(app.doc.dirty);
    assert!(app.session.ui.project_transition.is_none());
}

#[test]
fn update_restart_and_other_platforms_keep_the_unsaved_changes_guard() {
    for (macos, restarting) in [(true, true), (false, false)] {
        for dirty in [false, true] {
            let ctx = egui::Context::default();
            let mut app = app(dirty);
            let output = ctx.run_ui(Default::default(), |ui| {
                handle_root_close(&mut app, ui.ctx(), macos, restarting);
            });
            assert!(!commands(&output).contains(&egui::ViewportCommand::Visible(false)));
            assert_eq!(
                commands(&output).contains(&egui::ViewportCommand::CancelClose),
                dirty || macos
            );
            if dirty || macos {
                let pending = app.session.ui.project_transition.as_ref().unwrap();
                assert_eq!(pending.target, ProjectTransition::Quit);
                assert_eq!(
                    pending.phase,
                    if dirty {
                        ProjectTransitionPhase::NeedsConfirmation
                    } else {
                        ProjectTransitionPhase::CheckUnsaved
                    }
                );
            }
        }
    }
}

#[test]
fn quit_command_uses_the_shared_guard_without_closing_the_window_early() {
    for dirty in [false, true] {
        let ctx = egui::Context::default();
        let mut app = app(dirty);
        let output = ctx.run_ui(Default::default(), |ui| {
            crate::ui::commands::execute_without_clipboard(
                crate::ui::commands::CommandId::Quit,
                &mut app,
                ui.ctx(),
            );
            handle_close_request(&mut app, ui.ctx());
        });
        assert!(!commands(&output).contains(&egui::ViewportCommand::Close));
        let pending = app.session.ui.project_transition.as_ref().unwrap();
        assert_eq!(pending.target, ProjectTransition::Quit);
        assert_eq!(
            pending.phase,
            if dirty {
                ProjectTransitionPhase::NeedsConfirmation
            } else {
                ProjectTransitionPhase::CheckUnsaved
            }
        );
    }
}

#[test]
fn repeated_quit_does_not_replace_a_save_in_progress() {
    let ctx = egui::Context::default();
    let mut app = app(true);
    app.request_project_transition(ProjectTransition::Quit);
    app.session.ui.project_transition.as_mut().unwrap().phase = ProjectTransitionPhase::Saving;
    crate::ui::commands::execute_without_clipboard(
        crate::ui::commands::CommandId::Quit,
        &mut app,
        &ctx,
    );
    handle_close_request(&mut app, &ctx);
    assert_eq!(
        app.session.ui.project_transition.unwrap().phase,
        ProjectTransitionPhase::Saving
    );
}
