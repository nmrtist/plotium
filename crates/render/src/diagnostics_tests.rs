use crate::{diagnostics::*, *};
use plotium_figure::{Axis, LineStyle, Marker, Series};
fn figure() -> Figure {
    Figure::new("", Axis::new("x", 0., 100.), Axis::new("y", 0., 100.))
}
fn scan(s: &Series, clip: Option<Rect>, legend: Rect) -> CurveScanResult {
    let f = figure();
    let p = Projector {
        fig: &f,
        plot: Rect::new(0., 0., 100., 100.),
        top_band: None,
        left_band: None,
    };
    scan_curve(
        s,
        &p,
        clip,
        Some(legend),
        &mut CurveScan::default(),
        usize::MAX,
        &|| false,
    )
}
#[test]
fn sparse_points_and_sloping_lines_are_not_bbox_occlusion() {
    let s = Series::points("same", vec![[0., 0.], [100., 100.]]);
    assert_eq!(
        scan(
            &s,
            Some(Rect::new(0., 0., 100., 100.)),
            Rect::new(45., 45., 10., 10.)
        )
        .intersections,
        0
    );
    let s = Series::line("same", vec![[0., 0.], [100., 100.]]);
    assert_eq!(
        scan(
            &s,
            Some(Rect::new(0., 0., 100., 100.)),
            Rect::new(1., 1., 10., 10.)
        )
        .intersections,
        0
    );
    assert_eq!(
        scan(
            &s,
            Some(Rect::new(0., 0., 100., 100.)),
            Rect::new(45., 45., 10., 10.)
        )
        .intersections,
        1
    );
}
#[test]
fn effective_clip_excludes_primitives_before_legend_test() {
    let s = Series::line("same", vec![[0., 95.], [100., 95.]]);
    let r = scan(
        &s,
        Some(Rect::new(20., 20., 60., 60.)),
        Rect::new(30., 0., 10., 10.),
    );
    assert_eq!(r.visible, 0);
    assert_eq!(r.intersections, 0);
    assert_eq!(r.clipped, 1);
    assert_eq!(
        scan(&s, None, Rect::new(0., 0., 100., 100.)).intersections,
        0
    );
}
#[test]
fn dash_gaps_and_continuation_keep_native_phase() {
    let mut s = Series::line("dash", vec![[0., 50.], [7., 50.], [14., 50.], [21., 50.]]);
    s.width = 1.;
    s.appearance.line_style = LineStyle::Dashed;
    let clip = Rect::new(0., 0., 100., 100.);
    let legend = Rect::new(7., 49., 0.5, 2.);
    assert_eq!(scan(&s, Some(clip), legend).intersections, 0);
    let f = figure();
    let p = Projector {
        fig: &f,
        plot: clip,
        top_band: None,
        left_band: None,
    };
    let mut cursor = CurveScan::default();
    let a = scan_curve(&s, &p, Some(clip), Some(legend), &mut cursor, 2, &|| false);
    assert!(!a.finished);
    assert_eq!(cursor.next_point, 2);
    let b = scan_curve(&s, &p, Some(clip), Some(legend), &mut cursor, 2, &|| false);
    assert!(b.finished);
    assert_eq!(a.intersections + b.intersections, 0);
    assert!(
        scan_curve(
            &s,
            &p,
            Some(clip),
            None,
            &mut CurveScan::default(),
            2,
            &|| true
        )
        .cancelled
    );
}
#[test]
fn marker_shape_not_bounding_box_and_zero_stroke() {
    let mut s = Series::points("diamond", vec![[50., 50.]]);
    s.width = 5.;
    s.appearance.marker = Some(Marker::Diamond);
    let r = Rect::new(53.5, 53.5, 1., 1.);
    assert_eq!(
        scan(&s, Some(Rect::new(0., 0., 100., 100.)), r).intersections,
        0
    );
    s.kind = plotium_figure::SeriesKind::Line;
    s.appearance.marker = Some(Marker::None);
    s.width = 0.;
    s.points = vec![[0., 0.], [100., 100.]];
    assert_eq!(
        scan(
            &s,
            Some(Rect::new(0., 0., 100., 100.)),
            Rect::new(45., 45., 10., 10.)
        )
        .intersections,
        0
    );
}
#[test]
fn star_marker_hits_follow_its_concave_outline() {
    let mut s = Series::points("star", vec![[50., 50.]]);
    s.width = 5.;
    s.appearance.marker = Some(Marker::Pentagon);
    let page = Some(Rect::new(0., 0., 100., 100.));
    // Between the top and upper-right points: inside the pentagon, outside the star.
    let notch = Rect::new(51.8, 46.9, 0.4, 0.4);
    assert_eq!(scan(&s, page, notch).intersections, 1);
    s.appearance.marker = Some(Marker::Star);
    assert_eq!(scan(&s, page, notch).intersections, 0);
    assert_eq!(
        scan(&s, page, Rect::new(49.9, 45.2, 0.2, 0.2)).intersections,
        1
    );
}
#[test]
fn raster_integer_crop_rotation_and_fit_match_native_viewport() {
    let frame = Rect::new(10., 20., 100., 100.);
    let r = raster_bounds(frame, [200, 100], [0., 0., 1., 1.], 0, RasterFit::Contain).unwrap();
    assert_eq!((r.left, r.top, r.width, r.height), (10., 45., 100., 50.));
    let r = raster_bounds(frame, [200, 100], [0., 0., 1., 1.], 0, RasterFit::Cover).unwrap();
    assert_eq!((r.left, r.top, r.width, r.height), (-40., 20., 200., 100.));
    let r = raster_bounds(frame, [200, 100], [0., 0., 1., 1.], 1, RasterFit::Contain).unwrap();
    assert_eq!((r.left, r.top, r.width, r.height), (35., 20., 50., 100.));
    assert!(raster_bounds(frame, [0, 0], [0., 0., 1., 1.], 0, RasterFit::Contain).is_none());
}
#[test]
fn provenance_is_runtime_only() {
    let mut s = Series::line("same", vec![[0., 0.], [1., 1.]]);
    s.origin = Some(plotium_figure::SeriesOrigin(17));
    // Figure crate serialization is exercised in core tests (render has no JSON dependency).
    assert_eq!(s.clone().origin, s.origin);
}

#[test]
fn shape_extrema_include_stroke_and_native_arrow_heads() {
    let r = shape_bounds(Rect::new(10., 20., 100., 50.), OverlayShapeKind::Rect, 2.);
    assert_eq!((r.left, r.top, r.width, r.height), (9., 19., 102., 52.));
    let r = shape_bounds(Rect::new(10., 20., 100., 50.), OverlayShapeKind::Line, 2.);
    assert!((r.left - (10. - 1. / 5f32.sqrt())).abs() < 0.001);
    let arrow = shape_bounds(Rect::new(0., 0., 2., 2.), OverlayShapeKind::Arrow, 1.);
    assert!(arrow.left < 0. && arrow.top < 0.);
}

#[test]
fn stroke_markers_and_native_miter_join_extend_past_centerline_envelopes() {
    let clip = Some(Rect::new(0., 0., 100., 100.));
    let mut s = Series::points("cross", vec![[50., 50.]]);
    s.width = 4.;
    s.appearance.marker = Some(Marker::Cross);
    s.appearance.marker_size = Some(plotium_figure::PositiveFiniteF32::new(6.).unwrap());
    assert!(scan(&s, clip, Rect::new(53.2, 51.8, 0.3, 0.3)).intersections > 0);
    let mut s = Series::line("turn", vec![[20., 20.], [50., 80.], [80., 20.]]);
    s.width = 4.;
    assert!(scan(&s, clip, Rect::new(49.8, 15.9, 0.4, 0.5)).intersections > 0);
}

#[test]
fn butt_caps_and_clip_corner_do_not_invent_visible_strokes() {
    let mut s = Series::line("butt", vec![[20., 50.], [80., 50.]]);
    s.width = 4.;
    let r = scan(
        &s,
        Some(Rect::new(20., 40., 60., 20.)),
        Rect::new(18.5, 49., 1., 1.),
    );
    assert_eq!(r.clipped, 0);
    assert_eq!(r.intersections, 0);
    // Only the expanded centerline rectangle touches this corner, not the stroke.
    s.points = vec![[0., 0.], [100., 100.]];
    let r = scan(
        &s,
        Some(Rect::new(40., 40., 1., 1.)),
        Rect::new(40., 40., 1., 1.),
    );
    assert_eq!(r.visible, 0);
    assert_eq!(r.intersections, 0);
}

#[test]
fn plus_marker_butt_caps_are_not_clipped_at_their_native_extrema() {
    let mut s = Series::points("plus", vec![[50., 50.]]);
    s.width = 2.;
    s.appearance.marker = Some(Marker::Plus);
    s.appearance.marker_size = plotium_figure::PositiveFiniteF32::new(6.);
    let r = scan(
        &s,
        Some(Rect::new(47., 47., 6., 6.)),
        Rect::new(53.1, 49., 1., 1.),
    );
    assert_eq!(r.clipped, 0);
    assert_eq!(r.intersections, 0);
}

#[test]
fn duplicate_vertices_preserve_native_miter_conflicts() {
    let clip = Some(Rect::new(0., 0., 100., 100.));
    let legend = Rect::new(49.8, 15.9, 0.4, 0.5);
    for duplicates in [0, 1, 4] {
        let mut points = vec![[20., 20.]; duplicates + 1];
        points.extend(std::iter::repeat_n([50., 80.], duplicates + 1));
        points.extend(std::iter::repeat_n([80., 20.], duplicates + 1));
        let mut s = Series::line("same painted polyline", points);
        s.width = 4.;
        let result = scan(&s, clip, legend);
        assert_eq!(result.intersections, 1, "{duplicates} duplicate vertices");
        assert_eq!(result.visible, 3);
        assert_eq!(result.clipped, 0);
        assert_eq!(result.visited, s.points.len());
        assert!(result.finished);
    }
}

#[test]
fn duplicate_vertices_retain_tangent_across_batches_and_cancellation() {
    let mut s = Series::line(
        "same painted polyline",
        vec![[20., 20.], [50., 80.], [50., 80.], [50., 80.], [80., 20.]],
    );
    s.width = 4.;
    let f = figure();
    let clip = Rect::new(0., 0., 100., 100.);
    let p = Projector {
        fig: &f,
        plot: clip,
        top_band: None,
        left_band: None,
    };
    let legend = Some(Rect::new(49.8, 15.9, 0.4, 0.5));
    for budget in [1, 2, 3] {
        let mut cursor = CurveScan::default();
        let (mut visited, mut visible, mut intersections) = (0, 0, 0);
        loop {
            let result = scan_curve(&s, &p, Some(clip), legend, &mut cursor, budget, &|| false);
            assert!(result.visited <= budget);
            visited += result.visited;
            visible += result.visible;
            intersections += result.intersections;
            if result.finished {
                break;
            }
            let next = cursor.next_point;
            let cancelled = scan_curve(&s, &p, Some(clip), legend, &mut cursor, budget, &|| true);
            assert!(cancelled.cancelled);
            assert_eq!(cancelled.visited, 0);
            assert_eq!(cursor.next_point, next);
        }
        assert_eq!(visited, s.points.len());
        assert_eq!(visible, 3);
        assert_eq!(intersections, 1, "point budget {budget}");
    }
}

#[test]
fn duplicate_vertices_keep_native_miter_only_clipping() {
    // Both straight stroke envelopes fit below y=18; only the miter tip crosses it.
    let clip = Some(Rect::new(0., 18., 100., 82.));
    for points in [
        vec![[20., 20.], [50., 80.], [80., 20.]],
        vec![[20., 20.], [50., 80.], [50., 80.], [80., 20.]],
    ] {
        let mut s = Series::line("clipped miter", points);
        s.width = 4.;
        let result = scan(&s, clip, Rect::new(49.8, 15.9, 0.4, 0.5));
        assert_eq!(result.clipped, 1);
        assert!(result.first_clipped.unwrap().top < 18.);
        // The same tip is outside the effective clip, so it cannot occlude a legend.
        assert_eq!(result.intersections, 0);
    }
}

fn square_data_figure(lock_aspect: bool) -> Figure {
    let mut fig = Figure::new(
        "",
        Axis::new("x title", 0.0, 10.0),
        Axis::new("y", 0.0, 10.0),
    );
    fig.lock_aspect = lock_aspect;
    fig.width = 120.0;
    fig.height = 220.0;
    fig
}

fn x_title_gap(fig: &Figure) -> f32 {
    let boxes = figure_text_boxes(fig, [fig.width, fig.height]);
    let ticks_bottom = boxes
        .iter()
        .filter(|b| b.component.starts_with("axis.x.tick:"))
        .map(|b| b.bounds.bottom())
        .fold(f32::MIN, f32::max);
    let title = boxes
        .iter()
        .find(|b| b.component == "axis.x.label")
        .expect("x title box");
    title.bounds.top - ticks_bottom
}

#[test]
fn aspect_locked_x_title_keeps_its_gap_from_the_tick_labels() {
    let locked = square_data_figure(true);
    let layout = axis_layout(&locked, locked.width, locked.height);
    let outer = Rect::new(0.0, 0.0, locked.width, locked.height);
    let proj = Projector::new(&locked, outer, &layout.margins);
    assert!(proj.bottom_letterbox(outer, &layout.margins) > 10.0);

    let free = x_title_gap(&square_data_figure(false));
    assert!((x_title_gap(&locked) - free).abs() < 0.01);
}

#[test]
fn aspect_locked_svg_x_title_sits_under_the_plot() {
    let fig = square_data_figure(true);
    let layout = axis_layout(&fig, fig.width, fig.height);
    let outer = Rect::new(0.0, 0.0, fig.width, fig.height);
    let proj = Projector::new(&fig, outer, &layout.margins);
    let svg = crate::svg::export(&fig);
    let end = svg.find(">x title</text>").expect("x title text");
    let start = svg[..end].rfind("<text").expect("x title element");
    let y: f32 = svg[start..end]
        .split("y=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .and_then(|value| value.parse().ok())
        .expect("x title y");
    let expected = outer.bottom() - proj.bottom_letterbox(outer, &layout.margins) - OUTER_PAD;
    assert!((y - expected).abs() < 0.01);
    assert!(y < outer.bottom() - 10.0);
}
