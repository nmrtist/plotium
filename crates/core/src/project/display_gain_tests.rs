use super::tests::{first_plot, first_plot_mut, synthetic_dosy_2d, temp_project};
use super::*;

#[test]
fn stack_display_gain_survives_rebuild_undo_and_both_project_save_modes() {
    let mut app = PlotiumApp::new();
    app.doc.datasets.push(Dataset::Nmr2D(Box::new(
        Nmr2DDataset::load(synthetic_dosy_2d()).unwrap(),
    )));
    let mut canvas = CanvasDocument::new("stack".into(), [120., 80.]);
    let id = canvas.allocate_object_id();
    canvas.objects.push(app.build_plot_object(
        0,
        ObjectFrame::new(0., 0., 340., 226.),
        id,
        "Stack".into(),
    ));
    app.doc.canvases.push(canvas);
    app.session.active_canvas = Some(0);
    let raw = stack_traces(&app);
    let original = first_plot(&app).figure().clone();
    assert!(original.series.len() > 1);
    assert!(original.series[1].display_baseline > 0.);
    let before = first_plot(&app).viewport.clone();
    let mut after = before.clone();
    after.auto_y = false;
    after.display_gain = 2.;
    after.view_x = AxisRange::new(original.x.min, original.x.max);
    after.view_y = AxisRange::new(original.y.min, original.y.max);
    let frozen = after.clone();
    app.commit_object_viewport(0, id, before.clone(), after);
    assert_amplified(first_plot(&app).figure(), &original);
    app.undo();
    assert_eq!(first_plot(&app).viewport, before);
    app.redo();
    app.rebuild_canvas(0);
    assert_eq!(first_plot(&app).viewport.view_x, frozen.view_x);
    assert_eq!(first_plot(&app).viewport.view_y, frozen.view_y);
    assert_amplified(first_plot(&app).figure(), &original);
    assert_eq!(stack_traces(&app), raw);

    for snapshots in [false, true] {
        let path = temp_project(if snapshots {
            "gain_snapshot"
        } else {
            "gain_recipe"
        });
        save_project(&app, &path, snapshots).unwrap();
        let mut loaded = load_project(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(first_plot(&loaded).viewport.display_gain, 2.);
        assert_amplified(first_plot(&loaded).figure(), &original);
        loaded.rebuild_canvas(0);
        assert_amplified(first_plot(&loaded).figure(), &original);
        let plot = first_plot_mut(&mut loaded);
        plot.viewport.display_gain = 1.;
        plot.apply_viewport();
        for (actual, expected) in plot.figure().series.iter().zip(&original.series) {
            for (a, b) in actual.points.iter().zip(&expected.points) {
                assert!((a[1] - b[1]).abs() < 1e-10);
            }
        }
    }
}

fn assert_amplified(actual: &plotium_figure::Figure, original: &plotium_figure::Figure) {
    assert_eq!(actual.series.len(), original.series.len());
    for (actual, original) in actual.series.iter().zip(&original.series) {
        assert_eq!(actual.display_baseline, original.display_baseline);
        for (a, b) in actual.points.iter().zip(&original.points) {
            assert_eq!(a[0], b[0]);
            assert!(
                (a[1] - (original.display_baseline + 2. * (b[1] - original.display_baseline)))
                    .abs()
                    < 1e-10
            );
        }
    }
}

fn stack_traces(app: &PlotiumApp) -> Vec<Vec<Complex64>> {
    match &app.doc.datasets[0].as_nmr2d().unwrap().processed {
        plotium_processing::Processed2D::Stack(stack) => stack.traces.clone(),
        _ => panic!("expected a processed spectrum stack"),
    }
}
