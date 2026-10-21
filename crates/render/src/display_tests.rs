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
