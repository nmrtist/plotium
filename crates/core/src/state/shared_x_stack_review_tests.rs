//! Stacks whose members differ: x extents, x direction and capabilities.
use super::*;
use crate::properties::{PropertyValue, axis};
use crate::state::{Dataset, FloatSeries, PlotiumApp, materialized_float_series_table};

fn table(first: i32, last: i32, tag: &str) -> Dataset {
    let xs: Vec<_> = (first..=last).map(|x| Some(f64::from(x))).collect();
    let table = materialized_float_series_table(
        ("x".to_owned(), String::new(), xs.clone()),
        vec![FloatSeries {
            name: "signal".to_owned(),
            unit: String::new(),
            values: xs,
            uncertainty: None,
            fit: None,
        }],
        tag,
    )
    .expect("table fixture");
    Dataset::Table(Box::new(table))
}

fn spectrum() -> Dataset {
    let spectrum = plotium_io::NmrData {
        points: (0..32)
            .map(|value| num_complex::Complex64::new(f64::from(value), 0.0))
            .collect(),
        domain: plotium_io::Domain::Frequency,
        spectral_width_hz: 4_000.0,
        observe_freq_mhz: 400.0,
        carrier_ppm: 0.0,
        nucleus: "1H".to_owned(),
        source: "spectrum".to_owned(),
        group_delay: 0.0,
    };
    Dataset::Nmr(Box::new(
        crate::state::NmrDataset::load(spectrum).expect("spectrum loads"),
    ))
}

/// One plot per dataset, top to bottom, composed into a stacked Panel.
fn stacked(datasets: Vec<Dataset>) -> (PlotiumApp, PanelId, Vec<ObjectId>) {
    let mut app = PlotiumApp::new();
    let mut canvas = CanvasDocument::new("stack".to_owned(), [120.0, 120.0]);
    let mut ids = Vec::new();
    for (index, dataset) in datasets.into_iter().enumerate() {
        app.doc.datasets.push(dataset);
        let id = canvas.allocate_object_id();
        let frame = ObjectFrame::new(10.0, 10.0 + index as f32 * 60.0, 200.0, 50.0);
        canvas
            .objects
            .push(app.build_plot_object(index, frame, id, format!("plot {index}")));
        ids.push(id);
    }
    app.doc.canvases.push(canvas);
    app.session.active_canvas = Some(0);
    let (panel, action) = app
        .compose_panel_action(0, "Stack".to_owned(), &ids, 0.0)
        .expect("compose");
    app.execute_action(action);
    let action = app
        .set_shared_x_stack_action(0, panel, true, None)
        .expect("stack");
    app.execute_action(action);
    app.clear_history();
    (app, panel, ids)
}

fn x_axis(app: &PlotiumApp, id: ObjectId) -> (f64, f64, bool) {
    let x = &app.doc.canvases[0]
        .object(id)
        .and_then(|item| item.plot())
        .expect("plot")
        .figure()
        .x;
    (x.min, x.max, x.reversed)
}

#[test]
fn members_with_different_x_extents_share_one_window() {
    let (app, _, ids) = stacked(vec![
        table(10, 80, "plotium.test.stack-a.v1"),
        table(5, 90, "plotium.test.stack-b.v1"),
    ]);
    let (top, bottom) = (x_axis(&app, ids[0]), x_axis(&app, ids[1]));
    assert_eq!((top.0, top.1), (bottom.0, bottom.1));
    assert!(top.0 <= 5.0 && top.1 >= 90.0, "{top:?}");
}

#[test]
fn members_follow_the_bottom_plots_x_direction_until_they_leave() {
    // An NMR spectrum draws ppm descending; the table below it ascends.
    let (mut app, panel, ids) = stacked(vec![spectrum(), table(-2, 12, "plotium.test.stack-c.v1")]);
    let natural = |app: &PlotiumApp, id| {
        app.doc.canvases[0]
            .object(id)
            .and_then(|item| item.plot())
            .unwrap()
            .natural_x_reversed()
    };
    assert!(natural(&app, ids[0]) && !natural(&app, ids[1]));
    assert!(
        !x_axis(&app, ids[0]).2,
        "the spectrum follows the bottom plot"
    );
    assert!(!x_axis(&app, ids[1]).2);
    let overrides = app.doc.canvases[0]
        .object(ids[0])
        .and_then(|item| item.plot())
        .map(|plot| plot.axis_overrides.clone());

    // Reordering makes the spectrum the bottom plot and its direction wins.
    let page = &mut app.doc.canvases[0];
    page.panel_mut(panel).unwrap().item_order.reverse();
    app.rebuild_canvas(0);
    assert!(x_axis(&app, ids[0]).2 && x_axis(&app, ids[1]).2);

    let action = app
        .set_shared_x_stack_action(0, panel, false, None)
        .expect("unstack");
    app.execute_action(action);
    assert!(x_axis(&app, ids[0]).2 && !x_axis(&app, ids[1]).2);
    let after = app.doc.canvases[0]
        .object(ids[0])
        .and_then(|item| item.plot())
        .map(|plot| plot.axis_overrides.clone());
    assert_eq!(after, overrides, "direction is never written to overrides");
    app.undo();
    assert!(x_axis(&app, ids[0]).2 && x_axis(&app, ids[1]).2);
}

#[test]
fn a_member_that_cannot_take_a_log_x_axis_rejects_the_whole_edit() {
    let (mut app, _, ids) = stacked(vec![spectrum(), table(1, 40, "plotium.test.stack-d.v1")]);
    let target = app.object_target(0, ids[1]).expect("table plot");
    let range = PropertyValue::AxisRange(crate::properties::AxisRangeValue::Manual {
        min: 1.0,
        max: 40.0,
        unit: None,
    });
    let commit = app
        .plan_property_write(
            crate::properties::plot_display::X_RANGE,
            std::slice::from_ref(&target),
            &range,
        )
        .expect("positive shared range");
    app.commit_property(commit);
    let error = app
        .plan_property_write(
            axis::X_SCALE,
            std::slice::from_ref(&target),
            &PropertyValue::Enum(axis::LOG),
        )
        .expect_err("NMR member rejects log");
    assert!(format!("{error:?}").contains("plot 0"), "{error:?}");

    let commit = app
        .plan_property_write(
            axis::X_THOUSANDS_SEPARATOR,
            std::slice::from_ref(&target),
            &PropertyValue::Bool(true),
        )
        .expect("shared toggle");
    assert_eq!(commit.applied.len(), 2, "the peer edit is reported");
    app.commit_property(commit);
}

#[test]
fn members_beside_a_removed_member_stop_drawing_as_joined() {
    let (mut app, _, ids) = stacked(vec![
        table(1, 10, "plotium.test.stack-e.v1"),
        table(1, 10, "plotium.test.stack-f.v1"),
        table(1, 10, "plotium.test.stack-g.v1"),
    ]);
    let page = &app.doc.canvases[0];
    let placements = page.shared_x_stack_placements();
    assert!(placements[&ids[0]].joined_below && placements[&ids[2]].joined_above);
    let action = app
        .move_content_to_panel_action(0, ids[1], None, 0)
        .expect("move out");
    app.execute_action(action);
    let placements = app.doc.canvases[0].shared_x_stack_placements();
    assert!(!placements[&ids[0]].joined_below);
    assert!(!placements[&ids[2]].joined_above);
}
