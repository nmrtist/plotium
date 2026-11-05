use super::*;

#[test]
fn volume_canvas_drag_edits_the_numeric_draft_without_committing_or_undo() {
    use plotium_core::state::NmrVolumePhaseDraft;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/bruker-3d-qf");
    let loaded = plotium_io::load_path(path).unwrap();
    let (dataset, _) =
        plotium_core::workflow::dataset_from_acquisition(loaded.acquisition).unwrap();
    let id = dataset.resource_id();
    let mut app = PlotiumApp::new();
    app.doc.datasets.push(dataset);
    app.focus_single(0);
    app.set_tool(Tool::ManualPhase);
    let volume = app.doc.datasets[0]
        .as_nmr2d()
        .unwrap()
        .volume
        .as_ref()
        .unwrap();
    let baseline = volume.phases;
    app.session.ui.nmr_volume_phase_drafts.insert(
        id,
        NmrVolumePhaseDraft {
            canvas_axis: 2,
            source: std::sync::Arc::downgrade(volume.source.dataset()),
            baseline,
            phases: baseline,
        },
    );
    let mark = (app.doc.edit_generation, app.session.undo_stack.len());
    let egui_ctx = egui::Context::default();
    let context = PhaseDragCtx {
        axis: PhaseAxis::F2,
        orient: PhaseOrient::Vertical,
        piv_px: 20.0,
        pivot_ppm: 5.0,
        map_min: 0.0,
        map_span: 10.0,
        map_rev: false,
    };
    let mut frame = |events| {
        let input = egui::RawInput {
            events,
            ..Default::default()
        };
        let _ = egui_ctx.run_ui(input, |ui| {
            volume_phase_drag(
                &mut app,
                0,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 100.0)),
                PlotRect::new(0.0, 0.0, 100.0, 100.0),
                &context,
                ui,
            )
        });
    };
    frame(vec![
        egui::Event::PointerMoved(egui::pos2(60.0, 60.0)),
        egui::Event::PointerButton {
            pos: egui::pos2(60.0, 60.0),
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        },
    ]);
    frame(vec![egui::Event::PointerMoved(egui::pos2(60.0, 40.0))]);
    frame(vec![egui::Event::PointerButton {
        pos: egui::pos2(60.0, 40.0),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Default::default(),
    }]);
    assert!(app.session.ui.nmr_volume_phase_drafts[&id].phases[2].p0_degrees > 0.0);
    assert_eq!(
        app.doc.datasets[0]
            .as_nmr2d()
            .unwrap()
            .volume
            .as_ref()
            .unwrap()
            .phases,
        baseline
    );
    assert_eq!(
        (app.doc.edit_generation, app.session.undo_stack.len()),
        mark
    );
    assert!(
        egui_ctx
            .data(|data| data.get_temp::<VolumeDrag>(egui::Id::new("volume_phase_drag")))
            .is_none()
    );
}

#[test]
fn cancel_volume_drag_restores_only_the_drag_and_keeps_mode() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/bruker-3d-qf");
    let loaded = plotium_io::load_path(path).unwrap();
    let (dataset, _) =
        plotium_core::workflow::dataset_from_acquisition(loaded.acquisition).unwrap();
    let id = dataset.resource_id();
    let volume = dataset.as_nmr2d().unwrap().volume.as_ref().unwrap();
    let mut before = plotium_core::state::NmrVolumePhaseDraft {
        canvas_axis: 2,
        source: std::sync::Arc::downgrade(volume.source.dataset()),
        baseline: volume.phases,
        phases: volume.phases,
    };
    before.phases[2].p0_degrees = 13.0;
    let mut app = PlotiumApp::new();
    app.doc.datasets.push(dataset);
    app.set_tool(Tool::ManualPhase);
    let ctx = egui::Context::default();
    ctx.data_mut(|data| {
        data.insert_temp(
            egui::Id::new("volume_phase_drag"),
            VolumeDrag {
                id,
                project: app.session.project_context,
                axis: 2,
                fixed: 0,
                index: 0,
                kind: PhaseDragKind::Ph0,
                before: before.clone(),
            },
        )
    });
    let mut changed = before.clone();
    changed.phases[2].p0_degrees = 42.0;
    app.session.ui.nmr_volume_phase_drafts.insert(id, changed);
    assert!(cancel_volume_drag(&mut app, &ctx));
    assert_eq!(app.session.tool, Tool::ManualPhase);
    assert_eq!(
        app.session.ui.nmr_volume_phase_drafts[&id].phases,
        before.phases
    );
    assert!(!cancel_volume_drag(&mut app, &ctx));
}
