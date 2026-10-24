use super::tests::fixture;
use super::*;
use crate::state::*;

fn plots(app: &PlotiumApp) -> Vec<ObjectId> {
    app.doc.canvases[0]
        .objects
        .iter()
        .filter(|object| object.plot().is_some())
        .map(|object| object.id)
        .collect()
}

#[test]
fn state_reads_the_selected_plot_and_the_part_clicked_inside_it() {
    let mut app = fixture();
    let plot = plots(&app)[0];
    app.select_object(0, plot);
    let reading = canvas_selection_reading(&app, 16);
    assert_eq!(reading["total"], 1);
    assert_eq!(reading["items"][0]["kind"], "plot");
    assert!(reading["part"].is_null());
    let target = reading["items"][0]["target"].clone();

    app.select_plot_part(0, plot, PlotPart::Legend);
    let part = &canvas_selection_reading(&app, 16)["part"];
    assert_eq!(part["part"], "legend");
    assert_eq!(part["property"], "object.guides.position");
    assert_eq!(part["target"], target);

    let label = app.doc.canvases[0]
        .object_mut(plot)
        .and_then(|object| object.plot_mut())
        .unwrap()
        .axis_overrides
        .annotations
        .add_text([1.0, 2.0], "peak".into(), 7.0)
        .unwrap();
    app.select_plot_part(0, plot, PlotPart::Label(label));
    let part = &canvas_selection_reading(&app, 16)["part"];
    assert_eq!(part["part"], "label");
    assert_eq!(part["kind"], "text");
    assert_eq!(part["annotation_id"], serde_json::json!(label));
    assert_eq!(part["placement"], "auto");

    // Clicking the plot body again selects the plot, not the part.
    app.select_object(0, plot);
    assert!(canvas_selection_reading(&app, 16)["part"].is_null());
}

#[test]
fn state_reads_selected_panels_and_drops_a_part_whose_plot_is_no_longer_lead() {
    let mut app = fixture();
    let [first, second] = plots(&app)[..] else {
        panic!("two plots expected");
    };
    app.select_plot_part(0, first, PlotPart::Legend);
    app.session.ui.selection = Selection::single(second);
    assert!(app.selected_plot_part().is_none());

    let panel = app.doc.canvases[0].parent_panel(second).unwrap();
    app.select_panel(0, panel);
    let reading = canvas_selection_reading(&app, 16);
    assert_eq!(reading["total"], 1);
    assert_eq!(reading["items"][0]["kind"], "panel");
    assert!(reading["part"].is_null());
}
