use super::*;
use plotium_core::state::{
    CanvasDocument, Dataset, ElectrophysiologyDataset, ObjectFrame, PlotAnnotation,
};

/// A page holding one plot of a short recording.
pub(crate) fn app_with_plot() -> (PlotiumApp, ObjectId) {
    let mut app = PlotiumApp::new_with_settings(plotium_core::settings::Settings::default());
    let recording = plotium_io::ElectrophysiologyData {
        abf_version: "2.9.0.0".to_owned(),
        sample_rate_hz: 10_000.0,
        channels: vec![plotium_io::RecordedChannel {
            name: "Current".to_owned(),
            unit: plotium_io::ElectricalUnit::from_symbol("pA"),
        }],
        sweeps: vec![plotium_io::Sweep {
            start_time_s: 0.0,
            channels: vec![vec![0.0, -1.0, 0.5]],
            commands: Vec::new(),
        }],
        protocol: None,
        source: "annotations.abf".to_owned(),
        import_warnings: Vec::new(),
    };
    app.doc.datasets.push(Dataset::Electrophysiology(Box::new(
        ElectrophysiologyDataset::load(recording),
    )));
    app.doc
        .canvases
        .push(CanvasDocument::new("p".to_owned(), [400.0, 300.0]));
    app.session.active_canvas = Some(0);
    let id = app.doc.canvases[0].allocate_object_id();
    let plot = app.build_plot_object(
        0,
        ObjectFrame::new(20.0, 20.0, 300.0, 200.0),
        id,
        "plot".to_owned(),
    );
    app.doc.canvases[0].objects.push(plot);
    app.clear_history();
    (app, id)
}

pub(crate) fn annotations(app: &PlotiumApp, object: ObjectId) -> &PlotAnnotations {
    &app.doc.canvases[0]
        .object(object)
        .and_then(|item| item.plot())
        .expect("plot")
        .axis_overrides
        .annotations
}

fn point(app: &PlotiumApp, object: ObjectId) -> PlotMenuPoint {
    PlotMenuPoint {
        canvas: app.doc.canvases[0].resource_id,
        object,
        annotation: None,
        at: [0.0001, 0.25],
        tail: [0.00005, 0.5],
    }
}

#[test]
fn new_text_is_selected_and_opened_for_editing_as_one_undo_step() {
    let (mut app, object) = app_with_plot();
    let at = point(&app, object);
    create(&mut app, at, NewAnnotation::Text);
    let text: &PlotAnnotation = &annotations(&app, object).items[0];
    assert_eq!(text.at, [0.0001, 0.25]);
    let id = text.id;
    assert_eq!(
        app.session.ui.plot_text_edit.as_ref().map(|e| e.annotation),
        Some(id)
    );
    assert_eq!(selected(&app), Some((0, object, id)));

    set_text(&mut app, 0, object, id, "  90.1%  ");
    assert_eq!(annotations(&app, object).items[0].text, "90.1%");
    // A text annotation keeps its text when the new one is blank.
    set_text(&mut app, 0, object, id, " ");
    assert_eq!(annotations(&app, object).items[0].text, "90.1%");

    app.undo();
    assert_eq!(annotations(&app, object).items[0].text, NEW_TEXT);
    app.undo();
    assert!(annotations(&app, object).is_empty());
}

#[test]
fn arrows_and_lines_are_created_at_the_clicked_point() {
    let (mut app, object) = app_with_plot();
    let at = point(&app, object);
    create(&mut app, at, NewAnnotation::Arrow);
    create(
        &mut app,
        at,
        NewAnnotation::Line(ReferenceOrientation::Horizontal),
    );
    let annotations = annotations(&app, object);
    assert_eq!(annotations.arrows[0].to, [0.0001, 0.25]);
    assert_eq!(annotations.arrows[0].at, [0.00005, 0.5]);
    assert_eq!(annotations.reference_lines[0].value, 0.25);
    assert!(app.session.ui.plot_text_edit.is_none());
}

#[test]
fn delete_removes_the_selected_annotation_and_its_selection() {
    let (mut app, object) = app_with_plot();
    let at = point(&app, object);
    create(&mut app, at, NewAnnotation::Arrow);
    assert!(selected(&app).is_some());
    assert!(delete_selected(&mut app));
    assert!(annotations(&app, object).is_empty());
    assert!(selected(&app).is_none());
    assert!(!delete_selected(&mut app));
    app.undo();
    assert_eq!(annotations(&app, object).arrows.len(), 1);
}

#[test]
fn delete_is_left_to_other_tools_for_their_own_marks() {
    let (mut app, object) = app_with_plot();
    let at = point(&app, object);
    create(&mut app, at, NewAnnotation::Arrow);
    app.session.tool = Tool::Peaks;
    assert!(!delete_selected(&mut app));
    assert_eq!(annotations(&app, object).arrows.len(), 1);
}
