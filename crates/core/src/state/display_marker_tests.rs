//! Marker fill and outline are per-curve overrides of the curve colour.
use super::*;
use crate::properties::{Availability, line_display};

fn appearance(app: &PlotiumApp, series: usize) -> plotium_figure::CurveAppearance {
    let SeriesEncoding::Line(line) = &plot(app, 0).binding.series[series].encoding else {
        panic!()
    };
    line.appearance
}
fn value(app: &PlotiumApp, target: &TargetRef, key: properties::PropertyId) -> PropertyValue {
    let resolved = app
        .resolve_property(&PropertyAddress::new(target.clone(), key))
        .unwrap();
    let properties::AggregateValue::Uniform(value) = resolved.value else {
        panic!()
    };
    value
}

#[test]
fn marker_fill_and_edge_are_set_per_curve_and_survive_save() {
    let mut app = fixture();
    let a = target(&app, 0, 0);
    let b = target(&app, 0, 1);
    let curve = Color::rgb(0, 114, 178);
    write(
        &mut app,
        line_display::COLOR,
        PropertyValue::Color(curve),
        std::slice::from_ref(&a),
    );
    assert_eq!(
        value(&app, &a, line_display::MARKER_FILL),
        PropertyValue::Color(curve)
    );
    assert_eq!(
        value(&app, &a, line_display::MARKER_EDGE_COLOR),
        PropertyValue::Color(curve)
    );
    assert_eq!(
        value(&app, &a, line_display::MARKER_EDGE_WIDTH),
        PropertyValue::Float(0.)
    );
    let untouched = plot(&app, 0).binding.series[1].clone();
    for (key, value) in [
        ("series.line.marker_fill", json!("#F2D797")),
        ("series.line.marker_edge_color", json!("#404040")),
        ("series.line.marker_edge_width", json!(0.5)),
    ] {
        let p = plan_tool(&app, request(&app, key, value, vec![a.clone()])).unwrap();
        execute_tool(&mut app, p, ExecutionAuthority::ReversibleModify).unwrap();
    }
    let set = appearance(&app, 0);
    assert_eq!(set.marker_fill, Some(Color::rgb(0xf2, 0xd7, 0x97)));
    assert_eq!(
        set.marker_edge(curve),
        Some((Color::rgb(0x40, 0x40, 0x40), 0.5))
    );
    assert_eq!(plot(&app, 0).binding.series[1], untouched);
    assert_eq!(plot(&app, 0).figure().series[0].appearance, set);
    assert_eq!(target(&app, 0, 1), b);

    let path =
        std::env::temp_dir().join(format!("plotium-marker-{}.plotium", uuid::Uuid::new_v4()));
    crate::project::save_project(&app, &path, false).unwrap();
    let loaded = crate::project::load_project(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(appearance(&loaded, 0), set);

    assert!(matches!(
        plan_tool(
            &app,
            request(
                &app,
                "series.line.marker_edge_width",
                json!(6.),
                vec![a.clone()]
            )
        ),
        Err(AutomationError::InvalidParameters { .. })
    ));
    write(
        &mut app,
        line_display::MARKER_EDGE_WIDTH,
        PropertyValue::Float(0.),
        std::slice::from_ref(&a),
    );
    assert_eq!(appearance(&app, 0).marker_edge_width, None);
    let reset = app
        .plan_property_reset(line_display::MARKER_FILL, std::slice::from_ref(&a))
        .unwrap();
    app.commit_property(reset);
    assert_eq!(appearance(&app, 0).marker_fill, None);
}

#[test]
fn semantic_colours_disable_marker_colour_overrides_but_not_edge_width() {
    let mut app = fixture();
    let a = target(&app, 0, 0);
    let object = app.doc.canvases[0].objects[0].id;
    let mut figure = plot(&app, 0).figure().clone();
    figure.series_colors_are_semantic = true;
    app.doc.canvases[0]
        .object_mut(object)
        .unwrap()
        .plot_mut()
        .unwrap()
        .adopt_rebuilt_figure(figure);
    for key in [line_display::MARKER_FILL, line_display::MARKER_EDGE_COLOR] {
        let resolved = app
            .resolve_property(&PropertyAddress::new(a.clone(), key))
            .unwrap();
        assert!(matches!(resolved.availability, Availability::Disabled(_)));
    }
    write(
        &mut app,
        line_display::MARKER_EDGE_WIDTH,
        PropertyValue::Float(0.4),
        std::slice::from_ref(&a),
    );
    assert!(appearance(&app, 0).marker_edge_width.is_some());

    // Overrides set before the chart turned semantic must not paint over it.
    let mut binding = plot(&app, 0).binding.series[0].clone();
    let SeriesEncoding::Line(line) = &mut binding.encoding else {
        panic!()
    };
    line.appearance.marker_fill = Some(Color::BLACK);
    line.appearance.marker_edge_color = Some(Color::BLACK);
    line.appearance.marker_hollow = true;
    let mut semantic = plot(&app, 0).figure().clone();
    semantic.series_colors_are_semantic = true;
    app.apply_series_binding_style(&mut semantic, &binding);
    for series in &semantic.series {
        assert_eq!(series.appearance.marker_fill, None);
        assert_eq!(series.appearance.marker_edge_color, None);
        assert!(series.appearance.marker_edge_width.is_some());
        assert!(series.appearance.marker_hollow);
    }
}

#[test]
fn hollow_markers_keep_an_outline_and_their_fill_setting() {
    let mut app = fixture();
    let a = target(&app, 0, 0);
    write(
        &mut app,
        line_display::MARKER_FILL,
        PropertyValue::Color(Color::rgb(0xf2, 0xd7, 0x97)),
        std::slice::from_ref(&a),
    );
    let p = plan_tool(
        &app,
        request(
            &app,
            "series.line.marker_hollow",
            json!(true),
            vec![a.clone()],
        ),
    )
    .unwrap();
    execute_tool(&mut app, p, ExecutionAuthority::ReversibleModify).unwrap();
    let hollow = appearance(&app, 0);
    assert!(hollow.marker_hollow);
    assert_eq!(hollow.marker_fill(Color::BLACK), None);
    assert_eq!(
        value(&app, &a, line_display::MARKER_EDGE_WIDTH),
        PropertyValue::Float(f64::from(plotium_figure::DEFAULT_DATA_LINE_WIDTH_PT))
    );
    assert_eq!(plot(&app, 0).figure().series[0].appearance, hollow);

    let path =
        std::env::temp_dir().join(format!("plotium-hollow-{}.plotium", uuid::Uuid::new_v4()));
    crate::project::save_project(&app, &path, false).unwrap();
    let loaded = crate::project::load_project(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(appearance(&loaded, 0), hollow);

    // Resetting the edge width removes the override instead of storing the hollow default.
    write(
        &mut app,
        line_display::MARKER_EDGE_WIDTH,
        PropertyValue::Float(0.8),
        std::slice::from_ref(&a),
    );
    let reset = app
        .plan_property_reset(line_display::MARKER_EDGE_WIDTH, std::slice::from_ref(&a))
        .unwrap();
    app.commit_property(reset);
    assert_eq!(appearance(&app, 0).marker_edge_width, None);

    let reset = app
        .plan_property_reset(line_display::MARKER_HOLLOW, std::slice::from_ref(&a))
        .unwrap();
    app.commit_property(reset);
    let filled = appearance(&app, 0);
    assert!(!filled.marker_hollow);
    assert_eq!(
        filled.marker_fill(Color::BLACK),
        Some(Color::rgb(0xf2, 0xd7, 0x97))
    );
    assert_eq!(filled.marker_edge(Color::BLACK), None);
}
