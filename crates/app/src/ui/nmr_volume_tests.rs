use super::*;

#[test]
fn volume_phase_settings_end_peak_editing_and_open_processing() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/bruker-3d-qf");
    let loaded = plotium_io::load_path(path).unwrap();
    let (dataset, _) =
        plotium_core::workflow::dataset_from_acquisition(loaded.acquisition).unwrap();
    let mut app = PlotiumApp::new();
    let owner = dataset.resource_id();
    app.doc.datasets.push(dataset);
    app.focus_single(0);
    app.session.ui.processing_task_collapsed = true;
    let id = CommandId::Tool(plotium_core::state::Tool::ManualPhase);
    let description = commands::describe(&app, id);
    assert!(description.enabled, "{:?}", description.disabled_reason);
    assert!(description.ribbon.is_some());
    assert_eq!(description.checked, None);
    assert_eq!(description.label, "Phase");
    app.set_tool(plotium_core::state::Tool::Peaks);
    commands::execute_without_clipboard(id, &mut app, &egui::Context::default());
    assert_eq!(app.session.tool, plotium_core::state::Tool::BrowseZoom);
    assert_eq!(app.session.ui.processing_task_dataset, Some(owner));
    assert!(!app.session.ui.processing_task_collapsed);
}

#[test]
fn volume_navigation_renders_and_disables_endpoint_steps() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/bruker-3d-qf");
    let loaded = plotium_io::load_path(&path).unwrap();
    let (dataset, _) =
        plotium_core::workflow::dataset_from_acquisition(loaded.acquisition).unwrap();
    let mut app = PlotiumApp::new();
    app.doc.datasets.push(dataset);
    app.focus_single(0);
    let id = app.doc.datasets[0].resource_id();
    app.select_nmr_volume_plane(id, 0, 0).unwrap();
    assert!(!commands::describe(&app, CommandId::PreviousNmrPlane).enabled);
    assert!(commands::describe(&app, CommandId::NextNmrPlane).enabled);
    let description = commands::describe(&app, CommandId::ApplyNmrVolumePhase);
    assert!(!description.enabled);
    assert!(description.disabled_reason.unwrap().contains("Open Phase"));
    let committed = app.doc.datasets[0]
        .as_nmr2d()
        .unwrap()
        .volume
        .as_ref()
        .unwrap()
        .phases;
    let mut draft = plotium_core::state::NmrVolumePhaseDraft {
        source: std::sync::Arc::downgrade(
            app.doc.datasets[0]
                .as_nmr2d()
                .unwrap()
                .volume
                .as_ref()
                .unwrap()
                .source
                .dataset(),
        ),
        baseline: committed,
        phases: committed,
    };
    app.session
        .ui
        .nmr_volume_phase_drafts
        .insert(id, draft.clone());
    assert!(!commands::describe(&app, CommandId::ApplyNmrVolumePhase).enabled);
    draft.phases[2].p0_degrees = 15.0;
    app.session.ui.nmr_volume_phase_drafts.insert(id, draft);
    assert!(commands::describe(&app, CommandId::ApplyNmrVolumePhase).enabled);
    // Readiness belongs to the selected dataset, not the last opened card.
    app.doc.datasets.push({
        let loaded = plotium_io::load_path(&path).unwrap();
        plotium_core::workflow::dataset_from_acquisition(loaded.acquisition)
            .unwrap()
            .0
    });
    app.focus_single(1);
    assert!(!commands::describe(&app, CommandId::ApplyNmrVolumePhase).enabled);
    app.focus_single(0);
    app.session
        .ui
        .nmr_volume_phase_drafts
        .get_mut(&id)
        .unwrap()
        .baseline[0]
        .p0_degrees = 90.0;
    assert!(!commands::describe(&app, CommandId::ApplyNmrVolumePhase).enabled);
    let ctx = egui::Context::default();
    let _ = ctx.run_ui(Default::default(), |ui| navigation(&mut app, ui));
    step(&mut app, true);
    assert_eq!(
        app.doc.datasets[0]
            .as_nmr2d()
            .unwrap()
            .volume
            .as_ref()
            .unwrap()
            .index,
        1
    );
}

#[test]
fn plane_commands_explain_missing_volume_and_phase_cancel_tracks_worker() {
    let mut app = PlotiumApp::new();
    for id in [
        CommandId::PreviousNmrPlane,
        CommandId::NextNmrPlane,
        CommandId::ApplyNmrVolumePhase,
    ] {
        let description = commands::describe(&app, id);
        assert!(!description.enabled);
        assert!(description.disabled_reason.unwrap().contains("Select"));
    }
    assert!(!commands::describe(&app, CommandId::CancelNmrVolumePhase).enabled);
    app.session.ui.nmr_volume_phase_busy = true;
    assert!(commands::describe(&app, CommandId::CancelNmrVolumePhase).enabled);
}

fn volume_fixture() -> Dataset {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/bruker-3d-qf");
    let loaded = plotium_io::load_path(path).unwrap();
    plotium_core::workflow::dataset_from_acquisition(loaded.acquisition)
        .unwrap()
        .0
}

#[test]
fn stale_draft_does_not_return_after_undo_when_card_is_closed() {
    let mut app = PlotiumApp::new();
    app.doc.datasets.push(volume_fixture());
    app.focus_single(0);
    let id = app.doc.datasets[0].resource_id();
    let (mut draft, _) = phase_draft(&app, id, volume(&app));
    let baseline = draft.baseline;
    draft.phases[2].p0_degrees = 15.0;
    app.session.ui.nmr_volume_phase_drafts.insert(id, draft);
    let mut changed = baseline;
    changed[2].p0_degrees = 30.0;
    app.set_nmr_volume_phases(id, changed).unwrap();
    assert!(!app.session.ui.nmr_volume_phase_drafts.contains_key(&id));
    app.undo();
    assert_eq!(volume(&app).phases, baseline);
    assert!(phase_gate(&app).is_err());
    assert_eq!(phase_draft(&app, id, volume(&app)).0.phases, baseline);
}

fn volume(app: &PlotiumApp) -> &plotium_core::state::NmrVolumeState {
    app.doc.datasets[0]
        .as_nmr2d()
        .unwrap()
        .volume
        .as_ref()
        .unwrap()
}

#[test]
fn draft_keeps_acquisition_axes_across_navigation_and_reopening() {
    let mut app = PlotiumApp::new();
    app.doc.datasets.push(volume_fixture());
    app.focus_single(0);
    let id = app.doc.datasets[0].resource_id();
    let mut committed = volume(&app).phases;
    committed[2].p0_degrees = 10.0;
    app.set_nmr_volume_phases(id, committed).unwrap();
    let (mut draft, stale) = phase_draft(&app, id, volume(&app));
    assert!(!stale);
    draft.phases[2].p1_degrees = 22.0;
    let edited = draft.phases;
    app.session.ui.nmr_volume_phase_drafts.insert(id, draft);
    for (fixed, roles) in [
        (0, ["fixed axis", "vertical axis", "horizontal axis"]),
        (1, ["vertical axis", "fixed axis", "horizontal axis"]),
        (2, ["vertical axis", "horizontal axis", "fixed axis"]),
    ] {
        app.select_nmr_volume_plane(id, fixed, 0).unwrap();
        assert_eq!(
            std::array::from_fn(|axis| axis_role(axis, volume(&app).fixed_axis)),
            roles
        );
        assert_eq!(phase_draft(&app, id, volume(&app)).0.phases, edited);
    }
    let state = volume(&app);
    let ppm = std::array::from_fn(|axis| {
        state
            .processed
            .axis_coordinate(axis, state.anchor[axis])
            .unwrap()
    });
    app.add_peak_3d(id, ppm).unwrap();
    assert_eq!(phase_draft(&app, id, volume(&app)).0.phases, edited);
    open_phase(&mut app);
    app.session
        .ui
        .close_task_tab(plotium_core::state::TaskDockTab::Processing);
    open_phase(&mut app);
    assert_eq!(phase_draft(&app, id, volume(&app)).0.phases, edited);
    let mark = (app.session.undo_stack.len(), app.doc.edit_generation);
    discard_phase_draft(&mut app, id);
    assert_eq!(phase_draft(&app, id, volume(&app)).0.phases, committed);
    assert_eq!(volume(&app).phases, committed);
    assert_eq!(
        mark,
        (app.session.undo_stack.len(), app.doc.edit_generation)
    );
    assert!(phase_gate(&app).is_err());
}

#[test]
fn phase_jobs_report_their_original_target_and_cancel_before_install() {
    let mut app = PlotiumApp::new();
    app.doc
        .datasets
        .extend([volume_fixture(), volume_fixture()]);
    app.doc.datasets[0].set_name(Some("First volume".into()));
    app.doc.datasets[1].set_name(Some("Second volume".into()));
    let id = app.doc.datasets[0].resource_id();
    let (mut draft, _) = phase_draft(&app, id, volume(&app));
    let baseline = draft.baseline;
    draft.phases[2].p0_degrees = 10.0;
    let mut worker = app.doc.datasets[0].as_nmr2d().unwrap().clone();
    worker.set_volume_phases(draft.phases).unwrap();
    app.session
        .ui
        .nmr_volume_phase_drafts
        .insert(id, draft.clone());
    app.focus_single(1);
    let ctx = egui::Context::default();
    let mut job = PhaseJob {
        project: app.session.project_context,
        id,
        name: "First volume".into(),
        expected: baseline,
        cancellation: nmr::CancellationToken::new(),
        output: std::sync::Arc::new(std::sync::Mutex::new(Some(Ok(worker.clone())))),
    };
    let key = egui::Id::new("volume_phase_job");
    ctx.data_mut(|data| data.insert_temp(key, job.clone()));
    app.session.ui.nmr_volume_phase_busy = true;
    cancel_phase(&mut app, &ctx);
    poll_phase(&mut app, &ctx);
    assert_eq!(volume(&app).phases, baseline);
    assert_eq!(
        app.session.ui.nmr_volume_phase_drafts[&id].phases,
        draft.phases
    );
    assert!(
        app.session
            .status
            .contains("Stopped phase calculation for First volume")
    );

    job.cancellation = nmr::CancellationToken::new();
    *job.output.lock().unwrap() = Some(Err("test worker error".into()));
    ctx.data_mut(|data| data.insert_temp(key, job.clone()));
    poll_phase(&mut app, &ctx);
    assert!(
        app.session
            .status
            .contains("First volume: test worker error")
    );

    let mut changed = baseline;
    changed[2].p0_degrees = 20.0;
    app.set_nmr_volume_phases(id, changed).unwrap();
    *job.output.lock().unwrap() = Some(Ok(worker.clone()));
    ctx.data_mut(|data| data.insert_temp(key, job.clone()));
    poll_phase(&mut app, &ctx);
    assert!(
        app.session
            .status
            .contains("First volume: Discarded an outdated phase result")
    );
    assert_eq!(volume(&app).phases, changed);
    assert!(!app.session.ui.nmr_volume_phase_drafts.contains_key(&id));
    app.select_nmr_volume_plane(id, 2, 1).unwrap();
    let anchor = volume(&app).anchor;
    let other = app.doc.datasets[1]
        .as_nmr2d()
        .unwrap()
        .volume
        .as_ref()
        .unwrap()
        .phases;
    job.expected = changed;
    *job.output.lock().unwrap() = Some(Ok(worker));
    ctx.data_mut(|data| data.insert_temp(key, job));
    poll_phase(&mut app, &ctx);
    assert!(app.session.status.contains("Applied phase to First volume"));
    assert_eq!(volume(&app).phases, draft.phases);
    assert_eq!(volume(&app).fixed_axis, 2);
    assert_eq!(volume(&app).anchor, anchor);
    assert_eq!(
        app.doc.datasets[1]
            .as_nmr2d()
            .unwrap()
            .volume
            .as_ref()
            .unwrap()
            .phases,
        other
    );
    assert_eq!(app.active_dataset(), Some(1));
}

#[test]
fn changing_project_detaches_a_running_phase_job_without_replacing_status() {
    let mut app = PlotiumApp::new();
    let ctx = egui::Context::default();
    let job = PhaseJob {
        project: app.session.project_context,
        id: volume_fixture().resource_id(),
        name: "Previous project".into(),
        expected: Default::default(),
        cancellation: nmr::CancellationToken::new(),
        output: std::sync::Arc::new(std::sync::Mutex::new(None)),
    };
    let key = egui::Id::new("volume_phase_job");
    ctx.data_mut(|data| data.insert_temp(key, job.clone()));

    let mut loaded = PlotiumApp::new();
    loaded.doc.datasets.push(volume_fixture());
    loaded.focus_single(0);
    app.install_loaded_project(loaded);
    app.session.status = "Project opened".into();
    poll_phase(&mut app, &ctx);
    assert!(job.cancellation.is_cancelled());
    assert!(ctx.data(|data| data.get_temp::<PhaseJob>(key)).is_none());
    assert!(!app.session.ui.nmr_volume_phase_busy);
    *job.output.lock().unwrap() = Some(Err("Old worker finished".into()));
    poll_phase(&mut app, &ctx);
    assert_eq!(app.session.status, "Project opened");

    let id = app.doc.datasets[0].resource_id();
    let (mut draft, _) = phase_draft(&app, id, volume(&app));
    draft.phases[2].p0_degrees = 15.0;
    app.session.ui.nmr_volume_phase_drafts.insert(id, draft);
    apply_phase(&mut app, &ctx);
    let next = ctx.data(|data| data.get_temp::<PhaseJob>(key)).unwrap();
    assert_eq!(next.project, app.session.project_context);
    assert_eq!(next.id, id);
    cancel_phase(&mut app, &ctx);
    assert!(next.cancellation.is_cancelled());
}

#[test]
fn repivot_preserves_real_and_imaginary_volume_samples() {
    let dataset = volume_fixture();
    let mut before = dataset.as_nmr2d().unwrap().clone();
    let mut after = before.clone();
    let mut phases = before.volume.as_ref().unwrap().phases;
    phases[2].p0_degrees = 23.0;
    phases[2].p1_degrees = -137.0;
    phases[2].pivot_fraction = 0.31;
    before.set_volume_phases(phases).unwrap();
    phases[2].repivot(0.79).unwrap();
    after.set_volume_phases(phases).unwrap();
    let before = &before.volume.as_ref().unwrap().processed;
    let after = &after.volume.as_ref().unwrap().processed;
    let shape = before.shape();
    for x in [0, shape[0] / 2, shape[0] - 1] {
        for y in [0, shape[1] / 2, shape[1] - 1] {
            for z in 0..shape[2] {
                for component in [0, 1] {
                    let sample = |volume: &plotium_processing::nmr_volume::NmrVolume| {
                        volume
                            .processed()
                            .data()
                            .get(&[x, y, z], &[0, 0, component])
                            .unwrap()
                    };
                    let expected = sample(before);
                    assert!((sample(after) - expected).abs() <= 1e-10 * (1.0 + expected.abs()));
                }
            }
        }
    }
}

#[test]
fn phase_routes_volume_selection_by_scope() {
    use plotium_core::state::{SelectionScope, Tool};

    let mut app = PlotiumApp::new();
    let action = plotium_core::actions::Action::insert_dataset_with_default_canvas(
        &app,
        crate::ui::properties::fixture::homonuclear_frequency_2d(),
        "Contour".to_owned(),
        plotium_core::state::DEFAULT_CANVAS_SIZE_MM,
    );
    app.execute_action(action);
    let plotted = app.active_dataset().unwrap();
    let di = app.doc.datasets.len();
    app.doc.datasets.push(volume_fixture());
    app.focus_single(di);
    let command = CommandId::Tool(Tool::ManualPhase);
    let ctx = egui::Context::default();
    app.session.ui.selection_scope = SelectionScope::CanvasObjects;
    commands::execute_without_clipboard(command, &mut app, &ctx);
    assert_eq!(
        phase_target(&app),
        Some(app.doc.datasets[plotted].resource_id())
    );
    assert_eq!(app.session.tool, Tool::ManualPhase);

    app.focus_single(di);
    app.session.ui.selection_scope = SelectionScope::DataList;
    commands::execute_without_clipboard(command, &mut app, &ctx);
    assert_eq!(app.session.tool, Tool::BrowseZoom);
    assert_eq!(
        app.session.ui.processing_task_dataset,
        Some(app.doc.datasets[di].resource_id())
    );
}
