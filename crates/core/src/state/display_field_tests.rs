//! Native field defaults must describe the geometry that the renderer emits.
use super::*;
use crate::properties::{Availability, SkipReason, line_display};
use plotium_figure::SeriesKind;

fn resolved(
    app: &PlotiumApp,
    target: &TargetRef,
    key: properties::PropertyId,
) -> properties::ResolvedProperty {
    app.resolve_property(&PropertyAddress::new(target.clone(), key))
        .unwrap()
}

#[test]
fn r6_default_table_marker_matches_native_points_and_resets_to_circle() {
    let mut app = super::super::curve_tests::fixture(false);
    let t = target(&app, 0, 0);
    let before = plot(&app, 0).binding.series[0].clone();
    let SeriesEncoding::Line(line) = &before.encoding else {
        panic!()
    };
    assert_eq!(
        line.kind,
        SeriesKind::Line,
        "preserve the existing persisted default"
    );
    assert_eq!(plot(&app, 0).figure().series[0].kind, SeriesKind::Points);
    let marker = resolved(&app, &t, line_display::MARKER);
    assert_eq!(marker.value.uniform(), Some(&PropertyValue::Enum("circle")));
    assert_eq!(marker.default_value, Some(PropertyValue::Enum("circle")));
    assert!(matches!(
        resolved(&app, &t, line_display::STYLE).availability,
        Availability::Disabled(_)
    ));
    let skipped = app
        .plan_property_write(
            line_display::STYLE,
            std::slice::from_ref(&t),
            &PropertyValue::Enum("dashed"),
        )
        .unwrap();
    assert!(skipped.applied.is_empty());
    assert_eq!(skipped.skipped[0].reason, SkipReason::NotApplicable);
    write(
        &mut app,
        line_display::MARKER,
        PropertyValue::Enum("square"),
        std::slice::from_ref(&t),
    );
    let rendered = &plot(&app, 0).figure().series[0];
    assert_eq!(rendered.kind, SeriesKind::Points);
    assert_eq!(rendered.appearance.marker(rendered.kind), Marker::Square);
    let reset = app
        .plan_property_reset(line_display::MARKER, std::slice::from_ref(&t))
        .unwrap();
    app.commit_property(reset);
    assert_eq!(plot(&app, 0).binding.series[0], before);
    let rendered = &plot(&app, 0).figure().series[0];
    assert_eq!(rendered.appearance.marker(rendered.kind), Marker::Circle);
}

#[test]
fn r6_default_mass_spectrum_rejects_markers_and_style_but_keeps_stroke_edits() {
    let mut ms = MassSpecDataset::load(super::super::mass_spec::sample_mass_spec_run());
    let (_, field) = ms
        .add_extraction(
            plotium_io::AcquisitionStreamId::new(3),
            0.5,
            1.0,
            MassSpectrumExtractionMethod::Mean,
        )
        .unwrap();
    let dataset = Dataset::MassSpec(Box::new(ms));
    let binding = SeriesBinding::from_field_all(&dataset, field);
    let mut app = PlotiumApp::new();
    app.doc.datasets.push(dataset);
    let mut canvas = CanvasDocument::new("spectrum".into(), [120., 80.]);
    let id = canvas.allocate_object_id();
    let mut object = app.build_plot_object(
        0,
        ObjectFrame::new(0., 0., 120., 80.),
        id,
        "spectrum".into(),
    );
    let p = object.plot_mut().unwrap();
    p.display_owner = None;
    p.binding.series = binding;
    canvas.objects.push(object);
    app.doc.canvases.push(canvas);
    app.rebuild_canvas(0);
    app.clear_history();
    let t = target(&app, 0, 0);
    let original = plot(&app, 0).binding.series[0].clone();
    let SeriesEncoding::Line(line) = &original.encoding else {
        panic!()
    };
    assert_eq!(line.kind, SeriesKind::Line);
    assert_eq!(plot(&app, 0).figure().series[0].kind, SeriesKind::Stick);
    for (key, value) in [
        (line_display::MARKER, PropertyValue::Enum("square")),
        (line_display::MARKER_SIZE, PropertyValue::Float(4.)),
        (line_display::STYLE, PropertyValue::Enum("dashed")),
        (
            line_display::MARKER_FILL,
            PropertyValue::Color(Color::BLACK),
        ),
        (
            line_display::MARKER_EDGE_COLOR,
            PropertyValue::Color(Color::BLACK),
        ),
        (line_display::MARKER_EDGE_WIDTH, PropertyValue::Float(0.5)),
    ] {
        assert!(matches!(
            resolved(&app, &t, key).availability,
            Availability::Disabled(_)
        ));
        let plan = app
            .plan_property_write(key, std::slice::from_ref(&t), &value)
            .unwrap();
        assert!(plan.applied.is_empty());
        assert_eq!(plan.skipped[0].reason, SkipReason::NotApplicable);
        app.commit_property(plan);
    }
    assert!(!app.can_undo());
    assert_eq!(plot(&app, 0).binding.series[0], original);
    write(
        &mut app,
        properties::line::STROKE_WIDTH,
        PropertyValue::Float(2.),
        &[t],
    );
    assert_eq!(plot(&app, 0).figure().series[0].width, 2.);
    assert_eq!(plot(&app, 0).figure().series[0].kind, SeriesKind::Stick);
    app.undo();
    assert_eq!(plot(&app, 0).binding.series[0], original);
}
