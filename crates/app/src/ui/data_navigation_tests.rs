use super::*;
use plotium_core::state::{PrimaryView, SeriesBinding, ViewportMode};

#[test]
fn secondary_series_navigation_preserves_context_and_hidden_state() {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/jcamp-ppm.dx");
    app.load_from(&fixture);
    app.load_from(&fixture);
    let canvas = app.doc.canvases[0].resource_id;
    let first = app.doc.canvases[0].objects[0].id;
    let dataset = app.doc.datasets[1].resource_id();
    let mut series = SeriesBinding::from_dataset(&app.doc.datasets[1]).unwrap();
    series.id = SeriesId::new(1);
    series.visible = false;
    let plot = app.doc.canvases[0].objects[0].plot_mut().unwrap();
    plot.next_series_id = SeriesId::new(2);
    plot.binding.series.push(series);
    let panel_id = app.doc.canvases[0].parent_panel(first).unwrap();
    let panel = app.doc.canvases[0].panel_mut(panel_id).unwrap();
    panel.name = "Overlay panel".into();
    panel.visible = false;
    let second = PlotIndex::build(&app).occurrences(dataset)[1];
    let index = PlotIndex::build(&app);
    let target = index.occurrences(dataset)[0];
    assert_eq!(
        target,
        Destination {
            canvas,
            plot: first,
            series: SeriesId::new(1)
        }
    );
    assert_ne!(target.label(&app), second.label(&app));
    assert!(target.label(&app).unwrap().contains("Overlay panel"));
    assert!(target.label(&app).unwrap().contains("hidden"));
    let summary = &index.summaries[&canvas];
    assert!(summary.compact.ends_with("2 datasets"));
    app.session.view = PrimaryView::Data;
    app.session.ui.data_browser_filter = "sample search".into();
    let selection = app.session.ui.data_selection.clone();
    let generation = app.doc.edit_generation;
    let ctx = crate::typography::test_context();
    navigate(&mut app, &ctx, target);
    assert_eq!(target.hidden(&app), Some(true));
    assert!(matches!(
        app.session.viewport_mode,
        ViewportMode::Fit(BoardFitTarget::Region(_))
    ));
    assert_eq!(app.session.active_canvas, Some(0));
    assert_eq!(app.session.ui.selection.object(), Some(first));
    assert_eq!(app.session.ui.data_selection, selection);
    assert!(app.session.view == PrimaryView::Data);
    assert_eq!(app.session.ui.data_browser_filter, "sample search");
    assert_eq!(app.doc.edit_generation, generation);
    assert_eq!(&PlotIndex::build(&app).summaries[&canvas], summary);

    reveal(&mut app, &ctx, target);
    assert_eq!(target.hidden(&app), Some(false), "{}", app.session.status);
    assert_eq!(highlight(&app, &ctx, canvas), Some(target));
    app.undo();
    assert_eq!(target.hidden(&app), Some(true));
}
