use super::*;
use plotium_core::state::Region;

#[test]
fn extraction_requires_an_explicit_view_action_after_completion() {
    let mut app = app_with_electrophysiology();
    let state = app.doc.datasets[0].region_analysis_mut().unwrap();
    let id = state.allocate_region_id().unwrap();
    state.regions.push(Region {
        id,
        lo: 0.,
        hi: 0.0003,
        name: "Signal".into(),
        metric: Some(RegionMetric::Height),
        label_position: None,
        color: [31, 111, 235],
    });
    let source = app.doc.datasets[0].resource_id();
    let canvas = app.session.active_canvas;
    assert_eq!(curve_action_label(&app, 0), "Extract curves");
    open_region_table(&mut app, 0);
    assert!(app.compute_busy(), "{}", app.session.status);
    assert_eq!(app.session.ui.region_task_dataset, Some(source));
    assert!(!commands::describe(&app, CommandId::SeriesTable).enabled);
    let task_count = app.session.scientific_tasks.list(0, 128).len();
    open_region_table(&mut app, 0);
    assert_eq!(app.session.scientific_tasks.list(0, 128).len(), task_count);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while app.compute_busy() {
        assert!(std::time::Instant::now() < deadline);
        app.poll_compute();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let table = app.region_table_index(0).expect("linked table committed");
    assert_eq!(app.active_dataset(), Some(0));
    assert_eq!(app.session.active_canvas, canvas);
    assert_eq!(app.session.ui.region_task_dataset, Some(source));
    assert_eq!(curve_action_label(&app, 0), "View extracted curves");
    assert!(commands::describe(&app, CommandId::SeriesTable).enabled);
    open_region_table(&mut app, 0);
    assert_eq!(app.active_dataset(), Some(table));
    assert_eq!(app.session.status, "Viewing extracted region curves.");
}

fn app_with_electrophysiology() -> PlotiumApp {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let recording = plotium_io::ElectrophysiologyData {
        abf_version: "2.9.0.0".to_owned(),
        sample_rate_hz: 10_000.0,
        channels: vec![plotium_io::RecordedChannel {
            name: "Current".to_owned(),
            unit: plotium_io::ElectricalUnit::from_symbol("pA"),
        }],
        sweeps: vec![plotium_io::Sweep {
            start_time_s: 0.0,
            channels: vec![vec![0.0, -1.0, -2.0, 0.0]],
            commands: Vec::new(),
        }],
        protocol: None,
        source: "synthetic.abf".to_owned(),
        import_warnings: Vec::new(),
    };
    let action = Action::insert_dataset_with_default_canvas(
        &app,
        Dataset::Electrophysiology(Box::new(
            plotium_core::state::ElectrophysiologyDataset::load(recording),
        )),
        "Canvas — patch clamp".to_owned(),
        plotium_core::state::DEFAULT_CANVAS_SIZE_MM,
    );
    app.execute_action(action);
    app
}
