use super::*;
use plotium_figure::Series;

fn gain_fixture() -> (PlotiumApp, egui::Context, egui::Rect, Pos2) {
    let (mut app, _, _) = zoomed_plot_fixture();
    let mut figure = Figure::new("stack", Axis::new("x", 0., 10.), Axis::new("y", -3., 16.));
    figure
        .series
        .push(Series::line("first", vec![[0., 0.], [5., 2.], [10., -1.]]));
    let mut second = Series::line("second", vec![[0., 10.], [5., 11.], [10., 9.5]]);
    second.display_baseline = 10.;
    figure.series.push(second);
    let mut viewport = CanvasViewport::from_figure(&figure);
    viewport.auto_y = false;
    let object = app.doc.canvases[0].object_mut(PLOT_ID).unwrap();
    object.kind = CanvasObjectKind::Plot(Box::new(PlotObject::new(
        None,
        plotium_core::state::SeriesId::new(1),
        plotium_core::state::DataBinding {
            coordinates: None,
            series: vec![],
        },
        plotium_core::state::ChartSpec::default(),
        plotium_core::state::StackSpec::default(),
        plotium_core::state::AxisProjections::default(),
        plotium_core::state::AxisOverrides::default(),
        figure,
        viewport,
    )));
    app.session.active_canvas = Some(0);
    let screen = egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(400., 300.));
    let plot = plot_inner_rect(&app, 0, PLOT_ID, screen).unwrap();
    let pointer = Pos2::new(plot.left + plot.width * 0.5, plot.top + plot.height * 0.5);
    let ctx = egui::Context::default();
    for _ in 0..2 {
        let _ = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |_| {},
        );
    }
    (app, ctx, screen, pointer)
}

fn wheel(
    app: &mut PlotiumApp,
    ctx: &egui::Context,
    screen: egui::Rect,
    pointer: Pos2,
    time: f64,
    modifiers: egui::Modifiers,
) {
    let input = egui::RawInput {
        screen_rect: Some(screen),
        time: Some(time),
        modifiers,
        events: vec![
            egui::Event::PointerMoved(pointer),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0., 80.),
                modifiers,
                phase: egui::TouchPhase::Move,
            },
        ],
        ..Default::default()
    };
    let _ = ctx.run_ui(input, |ui| {
        assert!(handle_navigation(app, 0, screen, ui));
    });
}

#[test]
fn wheel_amplifies_signed_stack_without_moving_baselines_and_coalesces_undo() {
    let (mut app, ctx, screen, pointer) = gain_fixture();
    let before = app.doc.canvases[0]
        .object(PLOT_ID)
        .unwrap()
        .plot()
        .unwrap()
        .clone();
    let before_rect = plot_inner_rect(&app, 0, PLOT_ID, screen).unwrap();
    for time in [0.1, 0.12, 0.14] {
        wheel(&mut app, &ctx, screen, pointer, time, egui::Modifiers::NONE);
    }
    let plot = app.doc.canvases[0].object(PLOT_ID).unwrap().plot().unwrap();
    let gain = plot.viewport.display_gain;
    assert!(gain > 1.);
    assert_eq!(plot.viewport.view_x, before.viewport.view_x);
    assert_eq!(plot.viewport.view_y, before.viewport.view_y);
    let after_rect = plot_inner_rect(&app, 0, PLOT_ID, screen).unwrap();
    for (series, old) in plot.figure().series.iter().zip(&before.figure().series) {
        assert_eq!(
            to_screen_y(series.display_baseline, after_rect, &plot.figure().y),
            to_screen_y(old.display_baseline, before_rect, &before.figure().y)
        );
        for (point, old_point) in series.points.iter().zip(&old.points) {
            assert!(
                (point[1] - (old.display_baseline + gain * (old_point[1] - old.display_baseline)))
                    .abs()
                    < 1e-10
            );
        }
    }
    assert!(app.session.undo_stack.is_empty());
    app.finish_pending_wheel_zoom(0.5, false);
    assert_eq!(app.session.undo_stack.len(), 1);
    app.undo();
    assert_eq!(
        app.doc.canvases[0]
            .object(PLOT_ID)
            .unwrap()
            .plot()
            .unwrap()
            .viewport,
        before.viewport
    );
    app.redo();
    assert_eq!(
        app.doc.canvases[0]
            .object(PLOT_ID)
            .unwrap()
            .plot()
            .unwrap()
            .viewport
            .display_gain,
        gain
    );
}

#[test]
fn axis_wheel_and_command_wheel_keep_coordinate_navigation() {
    let (mut app, ctx, screen, pointer) = gain_fixture();
    let (_, outer, plot) = plot_under_cursor(&app, 0, screen, pointer).unwrap();
    let axis = Pos2::new(pointer.x, (plot.bottom() + outer.bottom()) * 0.5);
    wheel(&mut app, &ctx, screen, axis, 0.1, egui::Modifiers::NONE);
    let plot = app.doc.canvases[0].object(PLOT_ID).unwrap().plot().unwrap();
    assert_eq!(plot.viewport.display_gain, 1.);
    assert!(plot.viewport.view_x.span() < plot.viewport.full_x.span());
    let old_zoom = app.session.board.zoom;
    wheel(
        &mut app,
        &ctx,
        screen,
        pointer,
        0.2,
        egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        },
    );
    assert_ne!(app.session.board.zoom, old_zoom);
    assert_eq!(
        app.doc.canvases[0]
            .object(PLOT_ID)
            .unwrap()
            .plot()
            .unwrap()
            .viewport
            .display_gain,
        1.
    );
}

#[test]
fn locked_plot_refuses_peak_height_changes() {
    let (mut app, ctx, screen, pointer) = gain_fixture();
    app.doc.canvases[0].object_mut(PLOT_ID).unwrap().locked = true;
    wheel(&mut app, &ctx, screen, pointer, 0.1, egui::Modifiers::NONE);
    assert_eq!(
        app.doc.canvases[0]
            .object(PLOT_ID)
            .unwrap()
            .plot()
            .unwrap()
            .viewport
            .display_gain,
        1.
    );
    assert!(app.session.undo_stack.is_empty());
}

#[test]
fn reset_peak_height_is_independent_and_undoable() {
    let (mut app, ctx, screen, pointer) = gain_fixture();
    wheel(&mut app, &ctx, screen, pointer, 0.1, egui::Modifiers::NONE);
    app.finish_pending_wheel_zoom(0.5, true);
    let before = app.doc.canvases[0]
        .object(PLOT_ID)
        .unwrap()
        .plot()
        .unwrap()
        .viewport
        .clone();
    crate::ui::commands::execute_without_clipboard(
        crate::ui::commands::CommandId::ResetPeakHeight,
        &mut app,
        &ctx,
    );
    let plot = app.doc.canvases[0].object(PLOT_ID).unwrap().plot().unwrap();
    assert_eq!(plot.viewport.display_gain, 1.);
    assert_eq!(plot.viewport.view_x, before.view_x);
    assert_eq!(plot.viewport.view_y, before.view_y);
    app.undo();
    assert_eq!(
        app.doc.canvases[0]
            .object(PLOT_ID)
            .unwrap()
            .plot()
            .unwrap()
            .viewport,
        before
    );
}
