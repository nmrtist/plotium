use super::*;
use plotium_core::actions::Action;
use plotium_core::state::{
    AfmDataset, AnalysisSelection, AxisRange, DEFAULT_CANVAS_SIZE_MM, TaskDockTab,
};
use plotium_io::{AfmData, AfmForceSet, AfmScale};
use std::sync::Arc;

fn insert_afm(forces: Option<AfmForceSet>) -> PlotiumApp {
    let mut app = PlotiumApp::new_with_settings(plotium_core::settings::Settings::default());
    let data = AfmDataset::load(AfmData {
        images: Vec::new(),
        forces,
        source: "synthetic.spm".into(),
        import_warnings: Vec::new(),
    });
    app.execute_action(Action::insert_dataset_with_default_canvas(
        &app,
        Dataset::Afm(Box::new(data)),
        "AFM".into(),
        DEFAULT_CANVAS_SIZE_MM,
    ));
    app
}

#[test]
fn afm_fit_group_is_visible_but_disabled_without_force_data() {
    let app = insert_afm(None);
    for branch in [AfmForceBranch::Approach, AfmForceBranch::Retract] {
        let command = describe(&app, CommandId::FitAfmForce(branch));
        assert!(!command.enabled);
        assert!(command.disabled_reason.unwrap().contains("force curves"));
        assert_eq!(command.ribbon.unwrap().group, "AFM Force Fit");
    }
    let hertz = describe(&app, CommandId::AfmFit(AfmFitCommand::OpenHertz));
    assert!(!hertz.enabled);
    assert_eq!(hertz.ribbon.unwrap().group, "AFM Force Fit");
}

#[test]
fn manual_hertz_calculates_and_only_exports_current_inputs() {
    // At E* = 50 MPa and R = 225 nm, F(nN) = indentation(nm)^1.5.
    let z = [64, 88, 98, 100, 104, 110, 116, 120];
    let corrected_force = [27, 8, 1, 0, 0, 0, 0, 0];
    let raw = z
        .into_iter()
        .zip(corrected_force)
        .map(|(z, force)| 100 * force + 2 * z + 100)
        .collect::<Vec<_>>();
    let samples = z.len();
    let mut app = insert_afm(Some(AfmForceSet {
        grid_width: 1,
        grid_height: 1,
        samples_per_curve: samples * 2,
        raw: Arc::from(raw.repeat(2)),
        signal_scale: AfmScale {
            multiplier: 0.01,
            offset: 0.0,
            unit: "nm".into(),
        },
        sample_period_s: None,
        z_positions: Some(Arc::from(z.map(f64::from).repeat(2))),
        display_order: Arc::from((0..samples * 2).collect::<Vec<_>>()),
        approach_samples: samples,
        deflection_sensitivity_m_per_v: None,
        spring_constant_n_per_m: Some(1.0),
        tip_radius_nm: None,
    }));
    let ctx = crate::typography::test_context();
    let open = CommandId::AfmFit(AfmFitCommand::OpenHertz);
    let run = CommandId::AfmFit(AfmFitCommand::RunHertz);
    let output = CommandId::AfmFit(AfmFitCommand::HertzDataSheet);
    assert!(
        catalog(&app)
            .iter()
            .any(|entry| entry.id == open && entry.enabled)
    );
    execute_without_clipboard(open, &mut app, &ctx);
    assert_eq!(app.session.ui.task_dock_active, Some(TaskDockTab::CurveFit));
    assert!(!describe(&app, run).enabled);
    assert!(!describe(&app, output).enabled);
    let draft = app.session.ui.afm_hertz_task.as_mut().unwrap();
    assert_eq!(draft.target.branch, AfmForceBranch::Approach);
    assert!(draft.radius_nm.is_empty());
    draft.radius_nm = "225".into();
    draft.contact_z_nm = "100".into();
    draft.baseline_z_nm = ["104".into(), "120".into()];
    draft.fit_z_nm = ["64".into(), "98".into()];
    assert!(describe(&app, run).enabled);
    execute_without_clipboard(run, &mut app, &ctx);
    let result = app.afm_hertz_result().expect("Hertz result");
    assert!((result.reduced_modulus_mpa - 50.0).abs() < 1.0e-10);
    let frame = |app: &mut PlotiumApp, events| {
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(600.0, 460.0),
                )),
                events,
                ..Default::default()
            },
            |ui| crate::ui::tools::render_curve_fit_task(app, ui),
        )
    };
    let output_visible = |output: egui::FullOutput| {
        output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Text(text)
                if text.galley.text().ends_with("Create Hertz Data Sheet")
                    && shape.clip_rect.contains(text.pos + egui::vec2(3.0, 5.0)))
        })
    };
    for _ in 0..4 {
        let _ = frame(&mut app, Vec::new());
    }
    assert!(!output_visible(frame(&mut app, Vec::new())));
    let _ = frame(
        &mut app,
        vec![
            egui::Event::PointerMoved(egui::pos2(450.0, 200.0)),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                phase: egui::TouchPhase::Move,
                delta: egui::vec2(0.0, -2500.0),
                modifiers: Default::default(),
            },
        ],
    );
    assert!((0..30).any(|_| output_visible(frame(&mut app, Vec::new()))));
    assert!(describe(&app, output).enabled);
    execute_without_clipboard(output, &mut app, &ctx);
    assert_eq!(app.doc.datasets.len(), 2);
    let draft = app.session.ui.afm_hertz_task.as_mut().unwrap();
    draft.contact_z_nm = "99".into();
    assert!(app.afm_hertz_result().is_none());
    assert!(!describe(&app, output).enabled);
}

#[test]
fn hertz_suggestion_requires_explicit_acceptance_and_calculation() {
    let z = 0i32..=100;
    let samples = z.clone().count();
    let raw = z.clone().map(|value| 1000 + (40 - value).max(0).pow(2));
    let mut app = insert_afm(Some(AfmForceSet {
        grid_width: 1,
        grid_height: 1,
        samples_per_curve: samples * 2,
        raw: raw.clone().chain(raw).collect(),
        signal_scale: AfmScale {
            multiplier: 0.001,
            offset: 0.0,
            unit: "nm".into(),
        },
        sample_period_s: None,
        z_positions: Some(z.clone().chain(z).map(f64::from).collect()),
        display_order: (0..samples * 2).collect(),
        approach_samples: samples,
        deflection_sensitivity_m_per_v: None,
        spring_constant_n_per_m: Some(1.0),
        tip_radius_nm: Some(1000.0),
    }));
    let ctx = crate::typography::test_context();
    let open = CommandId::AfmFit(AfmFitCommand::OpenHertz);
    let suggest = CommandId::AfmFit(AfmFitCommand::SuggestHertz);
    let apply = CommandId::AfmFit(AfmFitCommand::ApplyHertzSuggestion);
    let run = CommandId::AfmFit(AfmFitCommand::RunHertz);
    let output = CommandId::AfmFit(AfmFitCommand::HertzDataSheet);
    execute_without_clipboard(open, &mut app, &ctx);
    assert!(describe(&app, suggest).enabled);
    let draft = app.session.ui.afm_hertz_task.as_mut().unwrap();
    assert_eq!(draft.radius_nm, "1000");
    draft.contact_z_nm = "42".into();
    assert!(
        catalog(&app)
            .iter()
            .any(|entry| entry.id == suggest && entry.enabled)
    );
    assert!(!describe(&app, apply).enabled);
    execute_without_clipboard(suggest, &mut app, &ctx);
    let draft = app.session.ui.afm_hertz_task.as_ref().unwrap();
    assert_eq!(draft.contact_z_nm, "42");
    assert_eq!(draft.baseline_z_nm, ["", ""]);
    assert_eq!(draft.fit_z_nm, ["", ""]);
    let candidate = app.afm_hertz_suggestion().expect("Hertz suggestion").input;
    assert!(
        catalog(&app)
            .iter()
            .any(|entry| entry.id == apply && entry.enabled)
    );
    assert!(!describe(&app, run).enabled);

    let mut frame = |events| {
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 800.0),
                )),
                events,
                ..Default::default()
            },
            |ui| crate::ui::tools::render_curve_fit_task(&mut app, ui),
        )
    };
    let visible = |output: egui::FullOutput, label| {
        output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Text(text)
                if text.galley.text() == label
                    && shape.clip_rect.contains(text.pos + egui::vec2(3.0, 5.0)))
        })
    };
    for _ in 0..4 {
        let _ = frame(Vec::new());
    }
    assert!(visible(frame(Vec::new()), "Suggested inputs (not applied)"));
    let _ = frame(vec![
        egui::Event::PointerMoved(egui::pos2(750.0, 200.0)),
        egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            phase: egui::TouchPhase::Move,
            delta: egui::vec2(0.0, -2500.0),
            modifiers: Default::default(),
        },
    ]);
    assert!((0..30).any(|_| visible(frame(Vec::new()), "Dashed: suggested inputs")));

    execute_without_clipboard(apply, &mut app, &ctx);
    let draft = app.session.ui.afm_hertz_task.as_ref().unwrap();
    assert_eq!(draft.contact_z_nm, candidate.contact_z_nm.to_string());
    assert_eq!(
        draft.baseline_z_nm,
        candidate.baseline_z_nm.map(|v| v.to_string())
    );
    assert_eq!(draft.fit_z_nm, candidate.fit_z_nm.map(|v| v.to_string()));
    assert!(app.afm_hertz_suggestion().is_none());
    assert!(describe(&app, run).enabled);
    assert!(!describe(&app, output).enabled);
    assert!(app.afm_hertz_result().is_none());

    let draft = app.session.ui.afm_hertz_task.as_mut().unwrap();
    draft.radius_nm = "800".into();
    execute_without_clipboard(open, &mut app, &ctx);
    execute_without_clipboard(run, &mut app, &ctx);
    let result = app.afm_hertz_result().unwrap();
    assert_eq!(result.options.radius_nm, 800.0);
    assert_eq!(result.options.spring_constant_n_per_m, 1.0);
}

#[test]
fn afm_fit_command_uses_the_selected_branch_and_range() {
    let mut app = insert_afm(Some(AfmForceSet {
        grid_width: 1,
        grid_height: 1,
        samples_per_curve: 12,
        raw: Arc::from([5, 7, 9, 11, 13, 15, 20, 17, 14, 11, 8, 5]),
        signal_scale: AfmScale {
            multiplier: 1.0,
            offset: 0.0,
            unit: "nm".into(),
        },
        sample_period_s: None,
        z_positions: Some(Arc::from([0., 1., 2., 3., 4., 5., 0., 1., 2., 3., 4., 5.])),
        display_order: Arc::from((0..12).collect::<Vec<_>>()),
        approach_samples: 6,
        deflection_sensitivity_m_per_v: None,
        spring_constant_n_per_m: Some(1.0),
        tip_radius_nm: None,
    }));
    let source = app.doc.datasets[0].resource_id();
    let canvas = &app.doc.canvases[0];
    app.session.ui.analysis_selection = Some(AnalysisSelection {
        dataset: source,
        canvas: canvas.resource_id,
        object: canvas.objects[0].id,
        x_range: AxisRange::new(1.0, 4.0),
        y_range: None,
        field: None,
        unit: None,
        source_stream: None,
    });

    let ctx = crate::typography::test_context();
    assert!(describe(&app, CommandId::SelectRange).enabled);
    let command = CommandId::FitAfmForce(AfmForceBranch::Retract);
    assert!(
        catalog(&app)
            .iter()
            .any(|entry| entry.id == command && entry.enabled)
    );
    execute_without_clipboard(command, &mut app, &ctx);
    assert_eq!(app.active_dataset(), Some(0));
    assert_eq!(app.session.ui.task_dock_active, Some(TaskDockTab::CurveFit));
    let draft = app.session.ui.afm_fit_task.as_ref().unwrap();
    assert_eq!(draft.model, plotium_analysis::models::LINEAR_ID);
    let target = draft.target;
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1200.0, 900.0),
        )),
        ..Default::default()
    };
    let _ = ctx.run_ui(input.clone(), |ui| {
        crate::ui::tools::render_curve_fit_task(&mut app, ui);
    });
    let rendered = ctx.run_ui(input, |ui| {
        crate::ui::tools::render_curve_fit_task(&mut app, ui);
    });
    for label in [
        "Retract",
        "X: Relative Z (nm), not indentation | Y: Force (nN)",
        "Deflection: stored length",
        "Spring constant: 1 N/m (file, applied)",
        "Empirical model",
    ] {
        assert!(
            rendered.shapes.iter().any(|shape| matches!(
                &shape.shape,
                egui::Shape::Text(text) if text.galley.job.text == label
            )),
            "missing AFM fit label: {label}"
        );
    }
    let viewport = app.afm_fit_plot(target).unwrap().viewport.clone();
    let active = app.session.active_canvas;
    let run = |app: &mut PlotiumApp, command| {
        execute_without_clipboard(CommandId::AfmFit(command), app, &ctx);
    };
    run(&mut app, AfmFitCommand::Run);
    let result = app.afm_fit_result(target).unwrap();
    assert!(result.converged);
    assert_eq!(result.rows.len(), 4);
    for ([x, y, predicted], [expected_x, expected_y]) in
        result
            .rows
            .iter()
            .zip([[1., 17.], [2., 14.], [3., 11.], [4., 8.]])
    {
        assert_eq!(*x, expected_x);
        assert!((y - expected_y).abs() < 1e-12);
        assert!((predicted - y).abs() < 1e-6);
    }
    assert_eq!(app.doc.datasets.len(), 1);
    assert_eq!(app.doc.canvases.len(), 1);
    assert_eq!(app.session.active_canvas, active);
    assert!(app.session.ui.sheet_open.is_none());
    let plot = app.afm_fit_plot(target).unwrap();
    assert_eq!(plot.viewport, viewport);
    assert_eq!(plot.figure().series.len(), 3);
    assert_eq!(plot.figure().series[2].name, "Retract fit");
    app.undo();
    assert!(app.afm_fit_result(target).is_none());
    assert_eq!(app.afm_fit_plot(target).unwrap().figure().series.len(), 2);
    app.redo();

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("afm.plotium");
    plotium_core::project::save_project(&app, &path, true).unwrap();
    let reopened = plotium_core::project::load_project(&path).unwrap();
    assert!(reopened.afm_fit_result(target).unwrap().converged);
    assert_eq!(
        reopened.afm_fit_plot(target).unwrap().figure().series.len(),
        3
    );

    run(&mut app, AfmFitCommand::ToggleOverlay);
    assert_eq!(app.afm_fit_plot(target).unwrap().figure().series.len(), 2);
    run(&mut app, AfmFitCommand::Duplicate);
    assert_eq!(app.doc.datasets.len(), 1);
    assert_eq!(app.session.active_canvas, Some(1));
    assert_eq!(
        app.doc.canvases[1].objects[0]
            .plot()
            .unwrap()
            .figure()
            .series
            .len(),
        3
    );
    assert!(!app.afm_fit_result(target).unwrap().shown);
    run(&mut app, AfmFitCommand::ResidualPlot);
    assert_eq!(app.session.active_canvas, Some(2));
    let residual = app.doc.canvases[2].objects[0].plot().unwrap().figure();
    assert_eq!(residual.series[0].name, "Residual");
    assert_eq!(residual.x.label, "Relative Z (nm)");
    assert_eq!(residual.y.label, "Residual (nN)");
    assert!(app.session.ui.sheet_open.is_none());
    run(&mut app, AfmFitCommand::DataSheet);
    assert_eq!(app.doc.canvases.len(), 3);
    assert_eq!(app.session.ui.sheet_open, Some(2));
    assert_eq!(
        app.doc.datasets[2]
            .as_table()
            .unwrap()
            .series_bindings
            .len(),
        3
    );
}
