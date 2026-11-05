use super::*;

fn fixture() -> (PlotiumApp, DatasetId) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/bruker-3d-qf");
    let loaded = plotium_io::load_path(path).unwrap();
    let (dataset, _) =
        plotium_core::workflow::dataset_from_acquisition(loaded.acquisition).unwrap();
    let id = dataset.resource_id();
    let mut app = PlotiumApp::new();
    let action = plotium_core::actions::Action::insert_dataset_with_default_canvas(
        &app,
        dataset,
        "Preview fixture".into(),
        DEFAULT_CANVAS_SIZE_MM,
    );
    app.execute_action(action);
    app.focus_single(0);
    app.session.ui.processing_task_dataset = Some(id);
    app.session.ui.open_task_tab(TaskDockTab::Processing);
    let volume = app.doc.datasets[0]
        .as_nmr2d()
        .unwrap()
        .volume
        .as_ref()
        .unwrap();
    let mut phases = volume.phases;
    phases[2].p0_degrees = 41.0;
    app.session.ui.nmr_volume_phase_drafts.insert(
        id,
        NmrVolumePhaseDraft {
            canvas_axis: 2,
            source: Arc::downgrade(volume.source.dataset()),
            baseline: volume.phases,
            phases,
        },
    );
    (app, id)
}

#[test]
fn numeric_preview_remains_visible_after_exiting_canvas_phasing() {
    let (mut app, id) = fixture();
    let ctx = egui::Context::default();
    app.set_tool(Tool::ManualPhase);
    let before = request(&app).unwrap();
    app.toggle_tool(Tool::ManualPhase);
    assert!(request(&app).unwrap().matches(&before));
    assert_eq!(
        canvas_feedback(&app, &ctx),
        Some("Unapplied · updating plane preview…")
    );
    app.session.ui.close_task_tab(TaskDockTab::Processing);
    assert!(canvas_feedback(&app, &ctx).is_none());
    assert!(app.session.ui.nmr_volume_phase_drafts.contains_key(&id));
}

#[test]
fn requests_bind_target_project_baseline_draft_and_slice() {
    let (mut app, id) = fixture();
    let original = request(&app).unwrap();
    app.session
        .ui
        .nmr_volume_phase_drafts
        .get_mut(&id)
        .unwrap()
        .phases[2]
        .p1_degrees = 27.0;
    let edited = request(&app).unwrap();
    assert!(original.same_plane(&edited));
    assert!(!original.matches(&edited));
    let volume = app.doc.datasets[0]
        .as_nmr2d_mut()
        .unwrap()
        .volume
        .as_mut()
        .unwrap();
    volume.index += 1;
    assert!(!original.same_plane(&request(&app).unwrap()));
    app.session
        .ui
        .nmr_volume_phase_drafts
        .get_mut(&id)
        .unwrap()
        .baseline[2]
        .p0_degrees = 9.0;
    assert!(request(&app).is_none());
    let (mut app, _) = fixture();
    let first = request(&app).unwrap();
    app.session.project_context = PlotiumApp::new().session.project_context;
    assert!(!first.matches(&request(&app).unwrap()));
}

#[test]
fn closed_discarded_or_retargeted_previews_are_invisible() {
    let (mut app, id) = fixture();
    let current = request(&app).unwrap();
    let object = current.plot;
    let ctx = egui::Context::default();
    let figure = plotium_figure::Figure::new(
        "test",
        plotium_figure::Axis::new("x", 0.0, 1.0),
        plotium_figure::Axis::new("y", 0.0, 1.0),
    );
    ctx.data_mut(|data| {
        data.insert_temp(
            key(),
            Preview {
                shown: Some((current, Arc::new(figure))),
                companions: Some((Vec::new(), Vec::new())),
                ..Default::default()
            },
        )
    });
    assert!(contours(&app, &ctx, id, object).is_some());
    let before = (app.doc.edit_generation, app.session.undo_stack.len());
    app.session.ui.close_task_tab(TaskDockTab::Processing);
    assert!(contours(&app, &ctx, id, object).is_none());
    app.session.ui.processing_task_dataset = Some(id);
    app.session.ui.open_task_tab(TaskDockTab::Processing);
    assert!(contours(&app, &ctx, id, object).is_some());
    app.session.ui.nmr_volume_phase_drafts.remove(&id);
    assert!(contours(&app, &ctx, id, object).is_none());
    assert_eq!(
        (app.doc.edit_generation, app.session.undo_stack.len()),
        before
    );
}

#[test]
fn late_result_cannot_install_after_draft_changes() {
    let (mut app, id) = fixture();
    let current = request(&app).unwrap();
    let output = Arc::new(Mutex::new(Some(Err("obsolete worker failure".into()))));
    let ctx = egui::Context::default();
    ctx.data_mut(|data| {
        data.insert_temp(
            key(),
            Preview {
                running: Some((current, output)),
                ..Default::default()
            },
        )
    });
    app.session
        .ui
        .nmr_volume_phase_drafts
        .get_mut(&id)
        .unwrap()
        .phases[2]
        .p0_degrees += 1.0;
    update(&mut app, &ctx);
    let cache = ctx.data(|data| data.get_temp::<Preview>(key())).unwrap();
    assert!(
        cache
            .running
            .as_ref()
            .is_none_or(|(r, _)| r.matches(&request(&app).unwrap()))
    );
    assert!(cache.shown.is_none());
    assert!(cache.failed.is_none());
    assert!(!app.session.status.contains("obsolete"));
}

#[test]
fn preview_target_is_one_series_even_with_same_dataset_layers() {
    let (mut app, _) = fixture();
    let current = request(&app).unwrap();
    let target = target_series(&app, &current).unwrap();
    let plot = app
        .doc
        .canvases
        .iter_mut()
        .flat_map(|c| &mut c.objects)
        .find(|o| o.id == current.plot)
        .unwrap()
        .plot_mut()
        .unwrap();
    let mut second = target.clone();
    second.id = SeriesId::new(999);
    plot.binding.series.push(second.clone());
    assert_eq!(target_series(&app, &current).unwrap().id, target.id);
    let mut preceding = target.clone();
    preceding.id = SeriesId::new(998);
    let mut following = second.clone();
    following.id = SeriesId::new(1000);
    let (before, after) = companion_series(
        vec![
            preceding.clone(),
            target.clone(),
            second.clone(),
            following.clone(),
        ],
        Some(target.id),
    );
    assert_eq!(
        before.iter().map(|s| s.id).collect::<Vec<_>>(),
        vec![preceding.id]
    );
    assert_eq!(
        after.iter().map(|s| s.id).collect::<Vec<_>>(),
        vec![second.id, following.id]
    );
}
