//! Stacks whose members differ: x extents, x direction and capabilities.
use super::*;
use crate::properties::{PropertyValue, axis};
use crate::state::{Dataset, FloatSeries, PlotiumApp, materialized_float_series_table};

fn table(first: i32, last: i32, tag: &str) -> Dataset {
    table_in_unit(first, last, tag, "")
}

fn table_in_unit(first: i32, last: i32, tag: &str, unit: &str) -> Dataset {
    let xs: Vec<_> = (first..=last).map(|x| Some(f64::from(x))).collect();
    let table = materialized_float_series_table(
        ("x".to_owned(), unit.to_owned(), xs.clone()),
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

#[test]
fn outside_legends_line_up_beside_the_widest_right_axis() {
    let (mut app, _, ids) = stacked(vec![
        table(1, 10, "plotium.test.stack-h.v1"),
        table(1, 10, "plotium.test.stack-i.v1"),
    ]);
    // The upper member alone draws a right axis; the lower one has the
    // wider legend, so neither member alone sizes the shared right margin.
    for (index, id) in ids.iter().enumerate() {
        let plot = app.doc.canvases[0]
            .object_mut(*id)
            .and_then(|item| item.plot_mut())
            .expect("plot");
        plot.axis_overrides.guide_visibility = Some(plotium_figure::GuideVisibility::Show);
        plot.axis_overrides.guide_placement = Some(plotium_figure::GuidePlacement::OutsideRight);
        let series = &mut plot.binding.series[0];
        if let plotium_figure::SeriesEncoding::Line(line) = &mut series.encoding
            && index == 0
        {
            line.appearance.y_axis = plotium_figure::YAxisSide::Right;
        }
        if index == 1 {
            series.label = Some("Longer entry".to_owned());
        }
    }
    app.rebuild_canvases_for(0);
    app.rebuild_canvases_for(1);
    let page = &app.doc.canvases[0];
    let placements = page.shared_x_stack_placements();
    let legends: Vec<_> = ids
        .iter()
        .map(|id| {
            let item = page.object(*id).unwrap();
            let figure = item.plot().unwrap().figure();
            let placement = placements[id];
            let layout = plotium_render::axis_layout_placed(
                figure,
                item.frame.width,
                item.frame.height,
                Some(&placement),
            );
            let outer = plotium_render::Rect::new(0.0, 0.0, item.frame.width, item.frame.height);
            let plot = plotium_render::Projector::new(figure, outer, &layout.margins).plot;
            let legend =
                plotium_render::legend_rect_placed(figure, plot, 1.0, Some(&placement)).unwrap();
            (legend, item.frame.width)
        })
        .collect();
    assert!(
        page.object(ids[0])
            .unwrap()
            .plot()
            .unwrap()
            .figure()
            .y2
            .is_some()
    );
    assert!((legends[0].0.left - legends[1].0.left).abs() < 1e-3);
    for (legend, width) in legends {
        assert!(legend.right() <= width, "{legend:?} overflows {width}");
    }
}

fn range_value(min: f64, max: f64) -> PropertyValue {
    PropertyValue::AxisRange(crate::properties::AxisRangeValue::Manual {
        min,
        max,
        unit: None,
    })
}

fn range_overrides(app: &PlotiumApp, ids: &[ObjectId]) -> Vec<AxisOverrides> {
    ids.iter()
        .map(|id| {
            app.doc.canvases[0]
                .object(*id)
                .unwrap()
                .plot()
                .unwrap()
                .axis_overrides
                .clone()
        })
        .collect()
}

#[test]
fn conflicting_shared_x_batch_is_rejected_without_changes() {
    use crate::properties::plot_display::X_RANGE;
    let (app, _, ids) = stacked(vec![
        table(1, 10, "plotium.test.stack-conflict-a.v1"),
        table(1, 10, "plotium.test.stack-conflict-b.v1"),
    ]);
    let before = range_overrides(&app, &ids);
    let revision = app.doc.automation_revision;
    let history = app.session.undo_stack.len();
    let writes = vec![
        (
            app.object_target(0, ids[0]).unwrap(),
            X_RANGE,
            range_value(0.0, 10.0),
        ),
        (
            app.object_target(0, ids[1]).unwrap(),
            X_RANGE,
            range_value(0.0, 20.0),
        ),
    ];
    for writes in [writes.clone(), writes.into_iter().rev().collect()] {
        let error = app
            .plan_property_writes(&writes)
            .expect_err("conflicting shared ranges");
        assert!(
            error.to_string().contains("Conflicting explicit settings"),
            "{error}"
        );
        assert_eq!(range_overrides(&app, &ids), before);
        assert_eq!(app.doc.automation_revision, revision);
        assert_eq!(app.session.undo_stack.len(), history);
    }
}

#[test]
fn same_value_shared_x_request_cannot_be_overridden_by_another_member() {
    use crate::properties::plot_display::X_RANGE;
    let (app, _, ids) = stacked(vec![
        table(1, 10, "plotium.test.stack-noop-a.v1"),
        table(1, 10, "plotium.test.stack-noop-b.v1"),
    ]);
    let range = range_overrides(&app, &ids)[0].x_range.unwrap();
    let error = app
        .plan_property_writes(&[
            (
                app.object_target(0, ids[0]).unwrap(),
                X_RANGE,
                range_value(range.min, range.max),
            ),
            (
                app.object_target(0, ids[1]).unwrap(),
                X_RANGE,
                range_value(0.0, 20.0),
            ),
        ])
        .expect_err("the first request is explicit even though it changes nothing");
    assert!(
        error.to_string().contains("Conflicting explicit settings"),
        "{error}"
    );
}

#[test]
fn compatible_shared_x_batch_commits_and_undoes_together() {
    use crate::properties::plot_display::X_RANGE;
    let (mut app, _, ids) = stacked(vec![
        table(1, 10, "plotium.test.stack-compatible-a.v1"),
        table(1, 10, "plotium.test.stack-compatible-b.v1"),
    ]);
    let before = range_overrides(&app, &ids);
    let writes: Vec<_> = ids
        .iter()
        .map(|id| {
            (
                app.object_target(0, *id).unwrap(),
                X_RANGE,
                range_value(0.0, 20.0),
            )
        })
        .collect();
    let commit = app
        .plan_property_writes(&writes)
        .expect("equivalent shared ranges");
    app.commit_property(commit);
    assert_eq!(app.session.undo_stack.len(), 1);
    for overrides in range_overrides(&app, &ids) {
        assert_eq!(overrides.x_range, Some(AxisRange::new(0.0, 20.0)));
    }
    app.undo();
    assert_eq!(range_overrides(&app, &ids), before);
    app.redo();
    assert_eq!(x_axis(&app, ids[0]), x_axis(&app, ids[1]));
    assert_eq!(x_axis(&app, ids[0]).1, 20.0);
}

#[test]
fn shared_x_batch_and_bulk_writes_respect_coordinate_units() {
    use crate::properties::plot_display::X_RANGE;
    let (mut app, panel, ids) = stacked(vec![
        table_in_unit(1, 10, "plotium.test.stack-units-a.v1", "s"),
        table_in_unit(1, 10, "plotium.test.stack-units-b.v1", "ms"),
    ]);
    let registry = plotium_data::UnitRegistry::plotium_v1();
    for (id, code) in ids.iter().zip(["s", "ms"]) {
        let plot = app.doc.canvases[0]
            .object_mut(*id)
            .unwrap()
            .plot_mut()
            .unwrap();
        plot.binding
            .coordinates
            .get_or_insert_with(crate::state::CurveCoordinates::default)
            .x = Some(registry.resolve(code).unwrap());
    }
    app.doc.canvases[0].unify_shared_x_window(panel).unwrap();
    let targets: Vec<_> = ids
        .iter()
        .map(|id| app.object_target(0, *id).unwrap())
        .collect();
    let before = range_overrides(&app, &ids);
    let revision = app.doc.automation_revision;
    let history = app.session.undo_stack.len();
    app.plan_property_write(X_RANGE, &targets, &range_value(0.0, 20.0))
        .expect_err("known coordinate units require an explicit input unit");
    assert_eq!(range_overrides(&app, &ids), before);
    assert_eq!(app.doc.automation_revision, revision);
    assert_eq!(app.session.undo_stack.len(), history);
    let with_unit = |max, unit: &str| {
        PropertyValue::AxisRange(crate::properties::AxisRangeValue::Manual {
            min: 0.0,
            max,
            unit: Some(unit.into()),
        })
    };
    let error = app
        .plan_property_writes(&[
            (targets[0].clone(), X_RANGE, with_unit(20.0, "s")),
            (targets[1].clone(), X_RANGE, with_unit(30_000.0, "ms")),
        ])
        .expect_err("different physical windows still conflict across units");
    assert!(
        error.to_string().contains("Conflicting explicit settings"),
        "{error}"
    );
    assert_eq!(range_overrides(&app, &ids), before);
    assert_eq!(app.doc.automation_revision, revision);
    assert_eq!(app.session.undo_stack.len(), history);
    let commit = app
        .plan_property_writes(&[
            (targets[0].clone(), X_RANGE, with_unit(20.0, "s")),
            (targets[1].clone(), X_RANGE, with_unit(20_000.0, "ms")),
        ])
        .expect("physically equivalent ranges in native units");
    app.commit_property(commit);
    let overrides = range_overrides(&app, &ids);
    assert_eq!(overrides[0].x_range, Some(AxisRange::new(0.0, 20.0)));
    assert_eq!(overrides[1].x_range, Some(AxisRange::new(0.0, 20_000.0)));
    let commit = app
        .plan_property_write(X_RANGE, &targets, &with_unit(30.0, "s"))
        .expect("bulk writes convert one explicit unit for each member");
    app.commit_property(commit);
    let overrides = range_overrides(&app, &ids);
    assert_eq!(overrides[0].x_range, Some(AxisRange::new(0.0, 30.0)));
    assert_eq!(overrides[1].x_range, Some(AxisRange::new(0.0, 30_000.0)));
}

#[test]
fn shared_x_break_endpoints_and_fraction_compose_across_members() {
    use crate::properties::plot_display::{X_BREAK, X_BREAK_FRACTION};
    let (mut app, _, ids) = stacked(vec![
        table(1, 10, "plotium.test.stack-break-a.v1"),
        table(1, 10, "plotium.test.stack-break-b.v1"),
    ]);
    let targets: Vec<_> = ids
        .iter()
        .map(|id| app.object_target(0, *id).unwrap())
        .collect();
    let commit = app
        .plan_property_write(X_BREAK, &targets[..1], &range_value(3.0, 5.0))
        .expect("initial shared break");
    app.commit_property(commit);
    let writes = vec![
        (targets[0].clone(), X_BREAK, range_value(4.0, 6.0)),
        (
            targets[1].clone(),
            X_BREAK_FRACTION,
            PropertyValue::Float(0.3),
        ),
    ];
    for writes in [writes.clone(), writes.into_iter().rev().collect()] {
        let commit = app
            .plan_property_writes(&writes)
            .expect("disjoint break edits compose");
        app.commit_property(commit);
        for overrides in range_overrides(&app, &ids) {
            let brk = overrides.x_break.unwrap();
            assert_eq!((brk.from, brk.to), (4.0, 6.0));
            assert_eq!(brk.lower_fraction, Some(0.3));
        }
        app.undo();
    }
}

#[test]
fn shared_x_scale_and_separator_conflicts_are_rejected() {
    let (app, _, ids) = stacked(vec![
        table(1, 10, "plotium.test.stack-settings-a.v1"),
        table(1, 10, "plotium.test.stack-settings-b.v1"),
    ]);
    for (property, first, second) in [
        (
            axis::X_SCALE,
            PropertyValue::Enum(axis::LOG),
            PropertyValue::Enum(axis::LINEAR),
        ),
        (
            axis::X_THOUSANDS_SEPARATOR,
            PropertyValue::Bool(true),
            PropertyValue::Bool(false),
        ),
    ] {
        let error = app
            .plan_property_writes(&[
                (app.object_target(0, ids[0]).unwrap(), property, first),
                (app.object_target(0, ids[1]).unwrap(), property, second),
            ])
            .expect_err("one shared axis cannot keep contradictory settings");
        assert!(
            error.to_string().contains("Conflicting explicit settings"),
            "{error}"
        );
    }
}
