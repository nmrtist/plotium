use super::*;
use crate::operation::OperationOutcome;
use plotium_processing::xrd::XrdProcessing;

fn pattern(name: &str) -> Dataset {
    let two_theta: Vec<f64> = (0..600).map(|i| 5.0 + f64::from(i) * 0.1).collect();
    let intensity = two_theta.iter().map(|x| 10.0 + x).collect();
    Dataset::Xrd(Box::new(XrdDataset::load(plotium_io::XrdData {
        two_theta_deg: two_theta,
        intensity,
        attenuation: None,
        source: name.to_owned(),
        instrument: None,
        target: None,
        wavelength_angstrom: Some(0.7),
        voltage_kv: None,
        current_ma: None,
        scan_step_deg: None,
        scan_speed_deg_min: None,
    })))
}

fn state_on(axis: XrdAxis) -> DatasetProcessingState {
    DatasetProcessingState::Xrd(XrdProcessing {
        axis,
        ..XrdProcessing::default()
    })
}

fn set_axis(app: &mut PlotiumApp, index: usize, axis: XrdAxis) {
    let before = DatasetProcessingState::from_dataset(&app.doc.datasets[index]);
    app.commit_processing_edit(index, before, state_on(axis));
}

#[test]
fn a_manual_range_follows_the_axis_and_undo_restores_it() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    app.doc.datasets.push(pattern("a.xy"));
    assert!(app.insert_dataset_canvas(0));
    let plot = |app: &mut PlotiumApp| {
        app.doc.canvases[0]
            .objects
            .iter_mut()
            .find_map(|o| o.plot_mut())
            .map(|plot| plot.axis_overrides.x_range)
            .unwrap()
    };
    app.doc.canvases[0]
        .objects
        .iter_mut()
        .find_map(|o| o.plot_mut())
        .unwrap()
        .axis_overrides
        .x_range = Some(AxisRange::new(25.0, 35.0));
    set_axis(&mut app, 0, XrdAxis::Q);
    let range = plot(&mut app).unwrap();
    let expected = XrdAxis::Q.position(25.0, 0.7).unwrap();
    assert!((range.min - expected).abs() < 1e-9, "{range:?}");
    app.undo();
    let range = plot(&mut app).unwrap();
    assert!((range.min - 25.0).abs() < 1e-9, "{range:?}");
    assert!((range.max - 35.0).abs() < 1e-9, "{range:?}");
}

#[test]
fn d_spacing_keeps_the_range_ordered() {
    let range = convert_range(
        AxisRange::new(25.0, 35.0),
        (XrdAxis::TwoTheta, Some(0.7)),
        (XrdAxis::D, Some(0.7)),
    )
    .unwrap();
    assert!(range.min < range.max);
}

#[test]
fn mixed_axes_cannot_stack_and_a_stack_warns_when_it_splits() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    app.doc.datasets.push(pattern("a.xy"));
    app.doc.datasets.push(pattern("b.xy"));
    app.session.ui.data_selection = vec![0, 1];
    assert!(app.stackable_selection().is_some());
    app.stack_selected_data();
    set_axis(&mut app, 0, XrdAxis::Q);
    let report = app.session.operation_history.operations().last().unwrap();
    assert_eq!(report.outcome, OperationOutcome::Warning);
    assert!(app.stackable_selection().is_none());
}

#[test]
fn a_stack_switched_one_pattern_at_a_time_converts_its_range_once() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    app.doc.datasets.push(pattern("a.xy"));
    app.doc.datasets.push(pattern("b.xy"));
    app.session.ui.data_selection = vec![0, 1];
    app.stack_selected_data();
    let canvas = app.doc.canvases.len() - 1;
    let range = |app: &PlotiumApp| {
        let plot = app.doc.canvases[canvas]
            .objects
            .iter()
            .find_map(|o| o.plot())
            .unwrap();
        (
            plot.axis_overrides.x_range.unwrap(),
            plot.figure().x.min,
            plot.figure().x.max,
        )
    };
    let (object, mut overrides) = app.doc.canvases[canvas]
        .objects
        .iter()
        .find_map(|o| o.plot().map(|plot| (o.id, plot.axis_overrides.clone())))
        .unwrap();
    overrides.x_range = Some(AxisRange::new(25.0, 35.0));
    app.set_axis_overrides_value(canvas, object, &overrides);
    set_axis(&mut app, 0, XrdAxis::Q);
    let expected = XrdAxis::Q.position(25.0, 0.7).unwrap();
    assert!((range(&app).0.min - expected).abs() < 1e-9);
    set_axis(&mut app, 1, XrdAxis::Q);
    assert!((range(&app).0.min - expected).abs() < 1e-9);
    assert!((range(&app).1 - expected).abs() < 1e-6, "{:?}", range(&app));
    app.undo();
    app.undo();
    let (restored, low, high) = range(&app);
    assert!((restored.min - 25.0).abs() < 1e-9);
    assert!((restored.max - 35.0).abs() < 1e-9);
    assert!(
        (low - 25.0).abs() < 1e-6 && (high - 35.0).abs() < 1e-6,
        "{low} {high}"
    );
}

#[test]
fn a_zoomed_view_follows_the_axis_too() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    app.doc.datasets.push(pattern("a.xy"));
    assert!(app.insert_dataset_canvas(0));
    let window = |app: &PlotiumApp| {
        let figure = app.doc.canvases[0]
            .objects
            .iter()
            .find_map(|o| o.plot())
            .unwrap()
            .figure();
        (figure.x.min, figure.x.max)
    };
    let plot = app.doc.canvases[0]
        .objects
        .iter_mut()
        .find_map(|o| o.plot_mut())
        .unwrap();
    plot.viewport.view_x = AxisRange::new(25.0, 35.0);
    plot.apply_viewport();
    set_axis(&mut app, 0, XrdAxis::Q);
    let expected = XrdAxis::Q.position(25.0, 0.7).unwrap();
    assert!(
        (window(&app).0 - expected).abs() < 1e-6,
        "{:?}",
        window(&app)
    );
    app.undo();
    assert!((window(&app).0 - 25.0).abs() < 1e-6, "{:?}", window(&app));
    assert!((window(&app).1 - 35.0).abs() < 1e-6, "{:?}", window(&app));
}

#[test]
fn an_unzoomed_plot_shows_the_whole_pattern_on_the_new_axis() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    app.doc.datasets.push(pattern("a.xy"));
    assert!(app.insert_dataset_canvas(0));
    set_axis(&mut app, 0, XrdAxis::D);
    let figure = app.doc.canvases[0]
        .objects
        .iter()
        .find_map(|o| o.plot())
        .unwrap()
        .figure();
    let xrd = app.doc.datasets[0].as_xrd().unwrap();
    let (x, _) = xrd.displayed();
    assert!(figure.x.min <= x[0] && figure.x.max >= *x.last().unwrap());
    app.undo();
    let figure = app.doc.canvases[0]
        .objects
        .iter()
        .find_map(|o| o.plot())
        .unwrap()
        .figure();
    assert!(
        figure.x.min <= 5.0 && figure.x.max >= 64.9,
        "{}",
        figure.x.max
    );
}
