use super::align_plot_area_left_edges;
use crate::state::{
    CanvasDocument, Dataset, FloatSeries, ObjectFrame, PlotiumApp, materialized_float_series_table,
};

fn table(scale: f64) -> Dataset {
    let xs: Vec<_> = (1..=10).map(|x| Some(f64::from(x))).collect();
    let table = materialized_float_series_table(
        ("x".to_owned(), String::new(), xs.clone()),
        vec![FloatSeries {
            name: "signal".to_owned(),
            unit: String::new(),
            values: xs.iter().map(|x| x.map(|x| x * scale)).collect(),
            uncertainty: None,
            fit: None,
        }],
        "plotium.test.align-plot-areas.v1",
    )
    .expect("table fixture");
    Dataset::Table(Box::new(table))
}

/// Page x of each plot's plot area.
fn plot_lefts(page: &CanvasDocument) -> Vec<f32> {
    page.objects
        .iter()
        .filter_map(|item| {
            let figure = item.plot()?.figure();
            let layout = plotium_render::axis_layout(figure, item.frame.width, item.frame.height);
            let panel = page.panel(page.parent_panel(item.id)?)?;
            Some(panel.frame.x + item.frame.x + layout.margins.left)
        })
        .collect()
}

#[test]
fn panels_of_one_column_share_the_plot_area_left_edge() {
    let mut app = PlotiumApp::new();
    let mut page = CanvasDocument::new("fit".to_owned(), [180.0, 150.0]);
    // Tick labels such as "10000" are wider than "1", which shifts the
    // upper plot area right inside an equally placed frame.
    for (index, (scale, top, height)) in [(1000.0, 50.0, 235.0), (1.0, 305.0, 90.0)]
        .into_iter()
        .enumerate()
    {
        app.doc.datasets.push(table(scale));
        let id = page.allocate_object_id();
        let frame = ObjectFrame::new(36.0, top, 438.0, height);
        page.objects
            .push(app.build_plot_object(index, frame, id, format!("plot {index}")));
        page.create_panel_for_plot(id).expect("panel");
    }
    let before = plot_lefts(&page);
    assert!((before[0] - before[1]).abs() > 1.0, "{before:?}");
    align_plot_area_left_edges(&mut page);
    let after = plot_lefts(&page);
    assert!((after[0] - after[1]).abs() < 0.1, "{after:?}");
    assert_eq!(after[0], before[0], "the wider axis band stays put");
    for panel in &page.panels {
        assert_eq!(panel.frame.x + panel.frame.width, 36.0 + 438.0);
    }
}
