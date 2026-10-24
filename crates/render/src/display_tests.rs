use super::*;
use plotium_figure::{
    Axis, AxisDisplay, CurveAppearance, LineStyle, Marker, PositiveFiniteF32, Series,
};
#[test]
fn r6_display_ticks_convert_units_without_moving_projected_points() {
    // A valid scientific range remains meaningful below machine epsilon.
    let tiny = Axis::new("Signal", 0., 1e-20);
    assert_eq!(tiny.normalize(1e-20), 1.);
    assert_eq!(tiny.normalize(5e-21), 0.5);
    assert_eq!(axis_ticks_for(&tiny, 5).values.last(), Some(&1e-20));
    let mut axis = Axis::new("Current", 0., 10.);
    let base = axis_ticks_for(&axis, 5);
    axis.display = Some(AxisDisplay {
        scale: 0.001,
        offset: 0.,
        unit: "A".into(),
    });
    let shown = axis_ticks_for(&axis, 5);
    assert_eq!(base.values, shown.values);
    assert_ne!(base.labels, shown.labels);
    assert!(shown.labels.iter().any(|s| s.contains("0.01")));
    axis.min = 273.15;
    axis.max = 303.15;
    axis.display = Some(AxisDisplay {
        scale: 1.,
        offset: -273.15,
        unit: "°C".into(),
    });
    let ticks = axis_ticks_for(&axis, 4);
    assert!(ticks.labels.contains(&"0".to_owned()));
    assert!((ticks.values[0] - 273.15).abs() < 1e-10);
}
#[test]
fn r6_dash_segments_are_page_bounded_and_preserve_gaps() {
    let clip = Rect::new(0., 0., 100., 100.);
    for style in [LineStyle::Dashed, LineStyle::Dotted, LineStyle::DashDot] {
        let segs = curve_style::dashed_segments(&[(-100., 50.), (200., 50.)], style, 1., clip);
        assert!(!segs.is_empty());
        assert!(segs.len() < 100);
        assert!(segs.iter().flatten().all(|p| p.0 >= -0.01 && p.0 <= 100.01));
        let enormous = curve_style::dashed_segments(&[(-1e30, 50.), (1e30, 50.)], style, 1., clip);
        assert!(enormous.len() < 100);
    }
}
#[test]
fn r6_native_svg_styles_and_legend_use_the_same_appearance() {
    let mut figure = Figure::new("", Axis::new("x", 0., 3.), Axis::new("y", 0., 3.));
    let mut series = Series::line("Authored", vec![[0.5, 1.], [1.5, 2.], [2.5, 1.]]);
    series.appearance = CurveAppearance {
        line_style: LineStyle::DashDot,
        marker: Some(Marker::Diamond),
        marker_size: Some(PositiveFiniteF32::new(6.).unwrap()),
        ..CurveAppearance::default()
    };
    series.width = 1.5;
    figure.series.push(series);
    figure.guide_visibility = plotium_figure::GuideVisibility::Show;
    let svg = svg::export(&figure);
    assert_eq!(svg.matches("stroke-dasharray").count(), 2);
    assert!(svg.matches("<polygon").count() >= 4);
    assert!(svg.contains("Authored"));
    let entries = legend_entries(&figure);
    assert!(matches!(entries[0].2, LegendMark::Curve { .. }));
    let mut unstyled = Series::line("Authored", vec![[0., 0.], [1., 1.]]);
    unstyled.color = figure.series[0].color;
    figure.series.push(unstyled);
    assert_eq!(
        legend_entries(&figure).len(),
        2,
        "Distinct appearance must not merge in either insertion order"
    );
    figure.series.swap(0, 1);
    assert_eq!(legend_entries(&figure).len(), 2);
    for marker in [
        Marker::Square,
        Marker::Diamond,
        Marker::TriangleUp,
        Marker::TriangleDown,
        Marker::Star,
        Marker::Pentagon,
        Marker::Hexagon,
    ] {
        assert!(!curve_style::marker_polygon(marker, (0., 0.), 3.).is_empty());
    }
    for marker in [Marker::Plus, Marker::Cross] {
        assert_eq!(curve_style::marker_segments(marker, (0., 0.), 3.).len(), 2);
    }
}
#[test]
fn closed_markers_take_their_own_fill_and_edge_in_plot_and_legend() {
    let mut figure = Figure::new("", Axis::new("x", 0., 3.), Axis::new("y", 0., 3.));
    let mut series = Series::points("A", vec![[0.5, 1.], [1.5, 2.]]);
    series.color = plotium_figure::Color::rgb(0xe6, 0xaf, 0x2d);
    figure.series.push(series);
    figure.guide_visibility = plotium_figure::GuideVisibility::Show;
    let plain = svg::export(&figure);
    assert_eq!(plain.matches(r##"fill="#e6af2d"/>"##).count(), 3);

    let appearance = &mut figure.series[0].appearance;
    appearance.marker_fill = Some(plotium_figure::Color::rgb(0xf2, 0xd7, 0x97));
    appearance.marker_edge_width = PositiveFiniteF32::new(0.5);
    let edged = svg::export(&figure);
    // Two data points plus the legend swatch; the outline defaults to the curve colour.
    assert_eq!(
        edged
            .matches(r##"fill="#f2d797" stroke="#e6af2d" stroke-width="0.5"/>"##)
            .count(),
        3,
        "{edged}"
    );

    figure.series[0].appearance.marker_edge_color = Some(plotium_figure::Color::rgb(64, 64, 64));
    figure.series[0].appearance.marker = Some(Marker::Square);
    let square = svg::export(&figure);
    assert_eq!(
        square
            .matches(r##"fill="#f2d797" stroke="#404040" stroke-width="0.5"/>"##)
            .count(),
        3
    );

    // Open marks have no fill or outline and keep stroking in the curve colour.
    figure.series[0].appearance.marker = Some(Marker::Plus);
    let open = svg::export(&figure);
    assert!(!open.contains("#f2d797") && !open.contains("#404040"));
}
#[test]
fn hollow_markers_draw_only_their_outline_in_plot_and_legend() {
    let mut figure = Figure::new("", Axis::new("x", 0., 3.), Axis::new("y", 0., 3.));
    let mut series = Series::points("A", vec![[0.5, 1.], [1.5, 2.]]);
    series.color = plotium_figure::Color::rgb(0xe6, 0xaf, 0x2d);
    series.appearance.marker_fill = Some(plotium_figure::Color::rgb(0xf2, 0xd7, 0x97));
    series.appearance.marker_hollow = true;
    figure.series.push(series);
    figure.guide_visibility = plotium_figure::GuideVisibility::Show;
    for marker in [Marker::Circle, Marker::Square, Marker::TriangleUp] {
        figure.series[0].appearance.marker = Some(marker);
        let svg = svg::export(&figure);
        // Without an edge width the outline takes the curve colour at the data line width.
        assert_eq!(
            svg.matches(r##"fill="none" stroke="#e6af2d" stroke-width="0.5"/>"##)
                .count(),
            3,
            "{svg}"
        );
        assert!(!svg.contains("#f2d797"));
    }
    let appearance = &mut figure.series[0].appearance;
    appearance.marker_edge_color = Some(plotium_figure::Color::rgb(64, 64, 64));
    appearance.marker_edge_width = PositiveFiniteF32::new(0.8);
    let svg = svg::export(&figure);
    assert_eq!(
        svg.matches(r##"fill="none" stroke="#404040" stroke-width="0.8"/>"##)
            .count(),
        3
    );
    // Screen and EMF paint from the same MarkerPaint as SVG.
    let paint = curve_style::MarkerPaint::new(
        CurveAppearance {
            marker_hollow: true,
            ..CurveAppearance::default()
        },
        plotium_figure::Color::BLACK,
        1.,
    );
    assert_eq!(paint.fill, None);
    assert_eq!(paint.edge, Some((plotium_figure::Color::BLACK, 0.5)));
    figure.series[0].appearance.marker_hollow = false;
    assert!(svg::export(&figure).contains(r##"fill="#f2d797" stroke="#404040""##));
}
#[test]
fn star_pentagon_and_hexagon_point_up_inside_the_shared_envelope() {
    for (marker, vertices) in [
        (Marker::Star, 10),
        (Marker::Pentagon, 5),
        (Marker::Hexagon, 6),
    ] {
        let polygon = curve_style::marker_polygon(marker, (10., 20.), 4.);
        assert_eq!(polygon.len(), vertices, "{marker:?}");
        let (x, y) = polygon[0];
        assert!(
            (x - 10.).abs() < 1e-5 && (y - 16.).abs() < 1e-5,
            "{marker:?} points up"
        );
        for (px, py) in &polygon {
            assert!((px - 10.).hypot(py - 20.) <= 4. + 1e-4, "{marker:?}");
        }
    }
    let star = curve_style::marker_polygon(Marker::Star, (0., 0.), 1.);
    for (k, (x, y)) in star.iter().enumerate() {
        let expected = if k % 2 == 0 { 1. } else { 0.381_966 };
        assert!((x.hypot(*y) - expected).abs() < 1e-5);
    }
    let mut figure = Figure::new("", Axis::new("x", 0., 3.), Axis::new("y", 0., 3.));
    let mut series = Series::points("A", vec![[1., 1.]]);
    series.appearance.marker = Some(Marker::Star);
    figure.series.push(series);
    let svg = svg::export(&figure);
    let star = svg
        .split("<polygon points=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("a star marker exports as an SVG polygon");
    assert_eq!(star.split(' ').count(), 10, "{svg}");
}
#[test]
fn marker_every_n_draws_every_nth_point_from_the_first() {
    let mut figure = Figure::new("", Axis::new("x", 0., 10.), Axis::new("y", 0., 10.));
    figure.series.push(Series::points(
        "A",
        (0..10).map(|i| [f64::from(i), 1.]).collect(),
    ));
    let centers = |svg: &str| -> Vec<String> {
        svg.split("<circle cx=\"")
            .skip(1)
            .map(|rest| rest[..rest.find('"').unwrap()].to_owned())
            .collect()
    };
    let all = centers(&svg::export(&figure));
    assert_eq!(all.len(), 10);
    figure.series[0].appearance.marker_every = std::num::NonZeroU32::new(4);
    assert_eq!(
        centers(&svg::export(&figure)),
        [all[0].clone(), all[4].clone(), all[8].clone()]
    );
}
#[test]
fn a_set_major_step_places_ticks_on_its_multiples() {
    let mut axis = Axis::new("Capacity", 75., 135.);
    axis.major_step = Some(20.);
    let ticks = axis_ticks_for(&axis, 5);
    assert_eq!(ticks.values, [80., 100., 120.]);
    assert_eq!(ticks.labels, ["80", "100", "120"]);
    // Every tick budget gets the same set.
    assert_eq!(axis_ticks_for(&axis, 2).values, ticks.values);
    let mut figure = Figure::new("", Axis::new("x", 0., 500.), axis.clone());
    figure.x.major_step = Some(100.);
    let layout = crate::axis_layout(&figure, 120., 90.);
    assert_eq!(layout.x_ticks.values, [0., 100., 200., 300., 400., 500.]);
    assert_eq!(layout.y_ticks.values, ticks.values);

    // The step is read in the display unit.
    axis.display = Some(AxisDisplay {
        scale: 1000.,
        offset: 0.,
        unit: "mA".into(),
    });
    axis.min = 0.075;
    axis.max = 0.135;
    let shown = axis_ticks_for(&axis, 5);
    assert_eq!(shown.labels, ["80", "100", "120"]);
    assert!((shown.values[0] - 0.08).abs() < 1e-12);

    // A step giving one tick or too many, or a log scale, keeps automatic ticks.
    let mut single = Axis::new("x", 75., 135.);
    single.major_step = Some(50.);
    assert_eq!(
        axis_ticks_for(&single, 5).values,
        axis_ticks_for(&Axis::new("x", 75., 135.), 5).values
    );
    let mut dense = Axis::new("x", 0., 1000.);
    dense.major_step = Some(1.);
    assert_eq!(
        axis_ticks_for(&dense, 5).values,
        axis_ticks_for(&Axis::new("x", 0., 1000.), 5).values
    );
    let mut log = Axis::new("x", 1., 1000.);
    log.scale = plotium_figure::AxisScale::Log10;
    let auto = axis_ticks_for(&log, 5);
    log.major_step = Some(20.);
    assert_eq!(axis_ticks_for(&log, 5), auto);
}
