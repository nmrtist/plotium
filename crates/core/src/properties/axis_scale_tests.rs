use super::*;
use crate::automation::TargetRef;
use crate::state::{
    CanvasDocument, Dataset, FloatSeries, ObjectFrame, ObjectId, PlotiumApp,
    materialized_float_series_table,
};
use plotium_figure::AxisScale;

/// A table line plot whose y values run 0..=99 over x 1..=100, so the data
/// includes a zero that a log axis must leave out.
pub(super) fn line_app() -> (PlotiumApp, TargetRef, ObjectId) {
    let table = materialized_float_series_table(
        (
            "x".to_owned(),
            String::new(),
            (1..=100).map(|value| Some(f64::from(value))).collect(),
        ),
        vec![FloatSeries {
            name: "signal".to_owned(),
            unit: String::new(),
            values: (0..100).map(|value| Some(f64::from(value))).collect(),
            uncertainty: None,
            fit: None,
        }],
        "plotium.test.axis-scale.v1",
    )
    .expect("table fixture");
    let mut app = PlotiumApp::new();
    app.doc.datasets.push(Dataset::Table(Box::new(table)));
    let mut canvas = CanvasDocument::new("table".to_owned(), [120.0, 80.0]);
    let id = canvas.allocate_object_id();
    canvas.objects.push(app.build_plot_object(
        0,
        ObjectFrame::new(0.0, 0.0, 100.0, 70.0),
        id,
        "Plot".to_owned(),
    ));
    app.doc.canvases.push(canvas);
    app.session.active_canvas = Some(0);
    let target = app.object_target(0, id).expect("plot target");
    (app, target, id)
}

pub(super) fn figure(app: &PlotiumApp, id: ObjectId) -> &plotium_figure::Figure {
    app.doc.canvases[0]
        .object(id)
        .and_then(|object| object.plot())
        .expect("plot")
        .figure()
}

pub(super) fn write(
    app: &mut PlotiumApp,
    target: &TargetRef,
    id: PropertyId,
    value: PropertyValue,
) {
    let commit = app
        .plan_property_write(id, std::slice::from_ref(target), &value)
        .expect("write plans");
    assert!(commit.skipped.is_empty(), "{:?}", commit.skipped);
    app.commit_property(commit);
}

pub(super) fn manual(min: f64, max: f64) -> PropertyValue {
    PropertyValue::AxisRange(AxisRangeValue::Manual {
        min,
        max,
        unit: None,
    })
}

#[test]
fn log_y_fits_the_positive_data_and_undoes() {
    let (mut app, target, id) = line_app();
    let resolved = app
        .resolve_property(&PropertyAddress::new(target.clone(), axis::Y_SCALE))
        .expect("scale resolves");
    assert_eq!(resolved.availability, Availability::Editable);
    assert_eq!(
        resolved.default_value,
        Some(PropertyValue::Enum(axis::LINEAR))
    );

    write(
        &mut app,
        &target,
        axis::Y_SCALE,
        PropertyValue::Enum(axis::LOG),
    );
    let y = &figure(&app, id).y;
    assert_eq!(y.scale, AxisScale::Log10);
    // The zero sample is left out: the window starts just below 1.
    assert!(y.min > 0.7 && y.min < 1.0, "{}", y.min);
    assert!(y.max > 99.0 && y.max < 130.0, "{}", y.max);
    let resolved = app
        .resolve_property(&PropertyAddress::new(target.clone(), axis::Y_SCALE))
        .expect("scale resolves");
    assert_eq!(
        resolved.value,
        AggregateValue::Uniform(PropertyValue::Enum(axis::LOG))
    );
    assert_eq!(resolved.modified, Some(true));

    app.undo();
    assert_eq!(figure(&app, id).y.scale, AxisScale::Linear);
    assert!(figure(&app, id).y.min <= 0.0);
    app.redo();
    assert_eq!(figure(&app, id).y.scale, AxisScale::Log10);

    let commit = app
        .plan_property_reset(axis::Y_SCALE, std::slice::from_ref(&target))
        .expect("reset plans");
    app.commit_property(commit);
    assert_eq!(figure(&app, id).y.scale, AxisScale::Linear);
}

#[test]
fn a_non_positive_manual_range_is_rejected_on_a_log_axis() {
    let (mut app, target, id) = line_app();
    write(
        &mut app,
        &target,
        axis::Y_SCALE,
        PropertyValue::Enum(axis::LOG),
    );
    let rejected = app.plan_property_write(
        plot_display::Y_RANGE,
        std::slice::from_ref(&target),
        &manual(0.0, 50.0),
    );
    let message = format!("{rejected:?}");
    assert!(message.contains("positive range"), "{message}");

    write(&mut app, &target, plot_display::Y_RANGE, manual(0.5, 50.0));
    let y = &figure(&app, id).y;
    assert_eq!((y.min, y.max, y.scale), (0.5, 50.0, AxisScale::Log10));
}

#[test]
fn switching_to_log_y_clears_display_gain() {
    let (mut app, target, id) = line_app();
    let original = figure(&app, id).series[0].points.clone();
    let plot = app.doc.canvases[0].objects[0].plot_mut().unwrap();
    plot.viewport.auto_y = false;
    plot.viewport.display_gain = 2.;
    plot.apply_viewport();
    assert_ne!(figure(&app, id).series[0].points, original);
    write(
        &mut app,
        &target,
        axis::Y_SCALE,
        PropertyValue::Enum(axis::LOG),
    );
    let plot = app.doc.canvases[0].objects[0].plot().unwrap();
    assert_eq!(plot.viewport.display_gain, 1.);
    assert_eq!(plot.figure().display_gain, 1.);
    assert_eq!(plot.figure().series[0].points, original);
    app.undo();
    assert_eq!(figure(&app, id).display_gain, 1.);
    assert_eq!(figure(&app, id).series[0].points, original);
}

#[test]
fn switching_to_log_over_a_non_positive_manual_range_is_rejected() {
    let (mut app, target, _) = line_app();
    write(&mut app, &target, plot_display::X_RANGE, manual(-5.0, 50.0));
    let rejected = app.plan_property_write(
        axis::X_SCALE,
        std::slice::from_ref(&target),
        &PropertyValue::Enum(axis::LOG),
    );
    let message = format!("{rejected:?}");
    assert!(message.contains("positive range"), "{message}");
}

#[test]
fn log_scales_are_disabled_on_2d_maps_and_nmr_spectra() {
    let (app, series) = crate::properties::tests::contour_app();
    let target = TargetRef::resource(series.resource);
    let resolved = app
        .resolve_property(&PropertyAddress::new(target, axis::X_SCALE))
        .expect("scale resolves");
    assert!(matches!(resolved.availability, Availability::Disabled(_)));

    let mut app = PlotiumApp::new();
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
    app.doc.datasets.push(Dataset::Nmr(Box::new(
        crate::state::NmrDataset::load(spectrum).expect("spectrum loads"),
    )));
    let mut canvas = CanvasDocument::new("page".to_owned(), [120.0, 80.0]);
    let id = canvas.allocate_object_id();
    canvas.objects.push(app.build_plot_object(
        0,
        ObjectFrame::new(0.0, 0.0, 100.0, 70.0),
        id,
        "Plot".to_owned(),
    ));
    app.doc.canvases.push(canvas);
    let target = app.object_target(0, id).expect("plot target");
    let resolved = app
        .resolve_property(&PropertyAddress::new(target.clone(), axis::Y_SCALE))
        .expect("scale resolves");
    assert!(matches!(resolved.availability, Availability::Disabled(_)));
    let commit = app
        .plan_property_write(
            axis::Y_SCALE,
            std::slice::from_ref(&target),
            &PropertyValue::Enum(axis::LOG),
        )
        .expect("write plans with a skip");
    assert!(commit.applied.is_empty());
}
