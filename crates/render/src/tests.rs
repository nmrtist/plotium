use super::*;
use crate::ticks::estimated_text_width;
use plotium_figure::{Axis, Figure, Series, TickDirection};

#[test]
fn ticks_are_nice_and_bounded() {
    let t = ticks(0.0, 10.0, 5);
    assert!(t.contains(&0.0));
    assert!(t.iter().all(|&v| (0.0..=10.0).contains(&v)));
    assert!(t.len() >= 3 && t.len() <= 12);
}

#[test]
fn ticks_never_exceed_range() {
    // Regression: the loop used to emit a tick up to half a step beyond max,
    // whose label spilled past the plot border (left side on reversed NMR axes).
    for &(lo, hi) in &[(0.5, 8.6), (-3.3, 4.1), (0.0, 10.0), (1.0, 100.0)] {
        let t = ticks(lo, hi, 8);
        assert!(
            t.iter().all(|&v| v >= lo - 1e-6 && v <= hi + 1e-6),
            "ticks({lo}, {hi}) produced out-of-range value: {t:?}"
        );
    }
}

#[test]
fn narrow_axis_labels_keep_the_precision_carried_by_the_ticks() {
    let ticks = axis_ticks(76.5, 78.1, 8);
    assert!(ticks.labels.iter().any(|label| label == "77.8"));
    assert!(ticks.labels.iter().any(|label| label == "77.0"));
    assert_eq!(ticks.scale_exponent, None);
}

#[test]
fn large_axis_uses_one_shared_scientific_multiplier() {
    let ticks = axis_ticks(-500.0, 17_000.0, 5);
    assert_eq!(ticks.scale_exponent, Some(4));
    assert_eq!(ticks.multiplier().as_deref(), Some("×10⁴"));
    assert!(ticks.labels.iter().any(|label| label == "0.0"));
    assert!(ticks.labels.iter().any(|label| label == "1.5"));
    assert!(ticks.labels.iter().all(|label| !label.contains('e')));
}

#[test]
fn margins_follow_tick_width_and_keep_a_compact_label_gap() {
    let compact = Figure::new("", Axis::new("x", 0.0, 1.0), Axis::new("y", 0.0, 1.0));
    let wide_ticks = Figure::new(
        "",
        Axis::new("x", 0.0, 1.0),
        Axis::new("y", -9_000.0, 9_000.0),
    );
    let compact_margins = Margins::for_figure(&compact);
    let wide_margins = Margins::for_figure(&wide_ticks);

    assert!(compact_margins.left < 36.0);
    assert!(wide_margins.left > compact_margins.left);
    assert!(wide_margins.left < 45.0);
    assert!(compact_margins.bottom < 30.0);

    let labels = axis_ticks(wide_ticks.y.min, wide_ticks.y.max, 5);
    let widest = labels
        .labels
        .iter()
        .map(|label| estimated_text_width(label, wide_ticks.typography.tick_pt))
        .fold(0.0, f32::max);
    let tick_text_left =
        wide_margins.left - wide_ticks.typography.tick_outset() - TICK_LABEL_PAD - widest;
    let y_title_right = OUTER_PAD + wide_ticks.typography.label_pt;
    assert!((tick_text_left - y_title_right - AXIS_LABEL_GAP).abs() < 1e-3);
}

#[test]
fn multipliers_get_their_own_text_rows() {
    let plain = Figure::new("t", Axis::new("x", 0.0, 1.0), Axis::new("y", 0.0, 1.0));
    let scientific = Figure::new(
        "t",
        Axis::new("x", 0.0, 90_000.0),
        Axis::new("y", 0.0, 90_000.0),
    );
    let plain_margins = Margins::for_figure(&plain);
    let sci_margins = Margins::for_figure(&scientific);
    let row = plain.typography.tick_pt + AXIS_LABEL_GAP;
    assert!((sci_margins.top - plain_margins.top - row).abs() < 1e-3);
    assert!((sci_margins.bottom - plain_margins.bottom - row).abs() < 1e-3);
}

#[test]
fn hidden_axis_text_reduces_margins_but_keeps_tick_space() {
    let visible = Figure::new(
        "",
        Axis::new("Long horizontal title", 0.0, 90_000.0),
        Axis::new("Long vertical title", -90_000.0, 90_000.0),
    );
    let mut hidden = visible.clone();
    hidden.x.show_tick_labels = false;
    hidden.x.show_label = false;
    hidden.y.show_tick_labels = false;
    hidden.y.show_label = false;
    let tick_outset = visible.typography.tick_outset();
    let visible = Margins::for_figure(&visible);
    let hidden = Margins::for_figure(&hidden);
    assert!(hidden.left < visible.left && hidden.bottom < visible.bottom);
    assert!(hidden.left >= OUTER_PAD + tick_outset);
    assert!(hidden.bottom >= OUTER_PAD + tick_outset);
}

#[test]
fn empty_adaptive_ticks_do_not_reserve_tick_mark_space() {
    let mut fig = Figure::new("", Axis::new("", 0.0, 1.0), Axis::new("", 0.0, 1.0));
    fig.x.show_label = false;
    fig.y.show_label = false;
    let layout = axis_layout(&fig, 1.0, 1.0);
    assert!(layout.x_ticks.values.is_empty() && layout.y_ticks.values.is_empty());
    assert_eq!(layout.margins.left, OUTER_PAD);
    assert_eq!(layout.margins.bottom, OUTER_PAD);
}

#[test]
fn svg_hidden_axis_text_omits_labels_and_multiplier_but_keeps_ticks() {
    let mut fig = Figure::new(
        "",
        Axis::new("UNIQUE_X_TITLE", 0.0, 90_000.0),
        Axis::new("UNIQUE_Y_TITLE", -90_000.0, 90_000.0),
    );
    fig.x.show_tick_labels = false;
    fig.x.show_label = false;
    fig.y.show_tick_labels = false;
    fig.y.show_label = false;
    let out = crate::svg::export(&fig);
    assert!(!out.contains("UNIQUE_X_TITLE") && !out.contains("UNIQUE_Y_TITLE"));
    assert!(!out.contains("×10"));
    assert!(out.matches("stroke=\"#000000\"").count() > 1);
}

#[test]
fn projection_bands_shrink_plot_and_share_edges() {
    use plotium_figure::AxisTrace;
    let mut fig = Figure::new(
        "t",
        Axis::new("x", 0.0, 10.0).reversed(true),
        Axis::new("y", 0.0, 20.0).reversed(true),
    );
    let outer = Rect::new(0.0, 0.0, 400.0, 300.0);
    let m = Margins::default();
    let (bare_w, bare_h) = {
        let bare = Projector::new(&fig, outer, &m);
        assert!(bare.top_band.is_none() && bare.left_band.is_none());
        (bare.plot.width, bare.plot.height)
    };

    let trace = AxisTrace {
        points: vec![[0.0, 0.0], [5.0, 1.0], [10.0, 0.0]],
        color: Color::TRACE,
        width: 1.0,
    };
    fig.top_projection = Some(trace.clone());
    fig.left_projection = Some(trace);
    let proj = Projector::new(&fig, outer, &m);
    let top = proj.top_band.expect("top band reserved");
    let left = proj.left_band.expect("left band reserved");
    // Bands hug the (now smaller) plot: top sits directly above it, left flush
    // to its left, sharing the plot's along-axis extent.
    assert!(proj.plot.width < bare_w && proj.plot.height < bare_h);
    assert!((top.bottom() - proj.plot.top).abs() < 1e-3);
    assert!((left.right() - proj.plot.left).abs() < 1e-3);
    assert!((top.width - proj.plot.width).abs() < 1e-3);

    // The trace lands inside its band.
    let pts = projection_points(
        &fig,
        fig.top_projection.as_ref().unwrap(),
        proj.plot,
        top,
        true,
    );
    assert!(
        pts.iter()
            .all(|(_, y)| *y >= top.top - 1e-3 && *y <= top.bottom() + 1e-3)
    );
}

#[test]
fn reversed_x_axis_flips_projection() {
    let fig = Figure::new(
        "t",
        Axis::new("ppm", 0.0, 10.0).reversed(true),
        Axis::new("i", 0.0, 1.0),
    );
    let outer = Rect::new(0.0, 0.0, 200.0, 100.0);
    let m = Margins::default();
    let proj = Projector::new(&fig, outer, &m);
    let (x_at_max, _) = proj.project([10.0, 0.5]);
    let (x_at_min, _) = proj.project([0.0, 0.5]);
    assert!(
        x_at_max < x_at_min,
        "reversed axis should put max on the left"
    );
}

#[test]
fn categorical_axis_labels_slots_and_thins_when_dense() {
    let axis = Axis::categorical("group", vec!["a".into(), "b".into(), "c".into()]);
    let t = axis_ticks_for(&axis, 8);
    assert_eq!(t.values, vec![0.0, 1.0, 2.0]);
    assert_eq!(t.labels, vec!["a", "b", "c"]);
    assert_eq!(t.scale_exponent, None);

    let many: Vec<String> = (0..40).map(|i| format!("c{i}")).collect();
    let dense = axis_ticks_for(&Axis::categorical("group", many), 8);
    assert!(dense.values.len() <= 8, "dense: {:?}", dense.labels);
    assert_eq!(dense.labels[0], "c0");

    // A zoomed window only labels visible slots.
    let mut axis = Axis::categorical("group", vec!["a".into(), "b".into(), "c".into()]);
    axis.min = 0.5;
    axis.max = 2.5;
    let zoomed = axis_ticks_for(&axis, 8);
    assert_eq!(zoomed.labels, vec!["b", "c"]);
}

#[test]
fn adaptive_x_labels_do_not_overlap_at_any_supported_width() {
    let fig = Figure::new(
        "",
        Axis::new("chemical shift", -12_345.0, 98_765.0).reversed(true),
        Axis::new("intensity", -1.0, 1.0),
    );
    for width in [24.0, 48.0, 72.0, 100.0, 160.0, 240.0, 400.0, 800.0] {
        let layout = axis_layout(&fig, width, 300.0);
        let proj = Projector::new(&fig, Rect::new(0.0, 0.0, width, 300.0), &layout.margins);
        let mut intervals: Vec<(f32, f32)> = layout
            .x_ticks
            .values
            .iter()
            .zip(&layout.x_ticks.labels)
            .map(|(&value, label)| {
                let (center, _) = proj.project([value, fig.y.min]);
                let half = estimated_text_width(label, fig.typography.tick_pt) * 0.5;
                (center - half, center + half)
            })
            .collect();
        intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
        assert!(
            intervals.windows(2).all(|pair| pair[0].1 <= pair[1].0),
            "overlapping labels at width {width}: {intervals:?}"
        );
    }
}

#[test]
fn left_x_endpoint_stays_clear_when_y_ticks_are_dropped() {
    let fig = Figure::new(
        "",
        Axis::new("x", -8_000.0, 0.0),
        Axis::new("intensity", 0.0, 10.0),
    );
    let layout = axis_layout(&fig, 400.0, 55.0);
    assert!(layout.y_ticks.labels.is_empty());

    let proj = Projector::new(&fig, Rect::new(0.0, 0.0, 400.0, 55.0), &layout.margins);
    let leftmost_label_edge = layout
        .x_ticks
        .values
        .iter()
        .zip(&layout.x_ticks.labels)
        .map(|(&value, label)| {
            let (center, _) = proj.project([value, fig.y.min]);
            center - estimated_text_width(label, fig.typography.tick_pt) * 0.5
        })
        .fold(f32::INFINITY, f32::min);
    let y_title_lane_right = OUTER_PAD + fig.typography.label_pt;
    assert!(
        leftmost_label_edge >= y_title_lane_right + AXIS_LABEL_GAP - 1e-3,
        "x label edge {leftmost_label_edge} entered y-title lane ending at {y_title_lane_right}"
    );
}

#[test]
fn categorical_y_first_pass_covers_labels_selected_by_later_strides() {
    let longest = "a very long category selected only by the second pass";
    let fig = Figure::new(
        "",
        Axis::new("x", -8_000.0, 0.0),
        Axis::categorical(
            "group",
            vec![
                "a".into(),
                "b".into(),
                "c".into(),
                longest.into(),
                "e".into(),
                "f".into(),
            ],
        ),
    );
    let layout = axis_layout(&fig, 160.0, 65.0);

    assert!(layout.y_ticks.labels.iter().any(|label| label == longest));
    assert!(
        layout.x_ticks.labels.is_empty(),
        "the conservative first pass must reject x labels before the later y stride widens margins"
    );
}

#[test]
fn adaptive_tick_counts_grow_monotonically_and_reach_the_old_wide_budget() {
    let fig = Figure::new("", Axis::new("x", 0.0, 10.0), Axis::new("y", 0.0, 10.0));
    let counts: Vec<usize> = [24.0, 60.0, 100.0, 160.0, 240.0, 400.0, 800.0]
        .into_iter()
        .map(|width| axis_layout(&fig, width, 600.0).x_ticks.values.len())
        .collect();
    assert!(
        counts.windows(2).all(|pair| pair[0] <= pair[1]),
        "tick counts were not monotonic: {counts:?}"
    );

    let wide = axis_layout(&fig, 800.0, 600.0);
    assert_eq!(wide.x_ticks, axis_ticks_for(&fig.x, 8));
    assert_eq!(wide.y_ticks, axis_ticks_for(&fig.y, 5));
}

#[test]
fn tiny_figures_drop_ticks_but_keep_a_finite_axis_rect() {
    let fig = Figure::new("", Axis::new("x", 0.0, 10.0), Axis::new("y", 0.0, 10.0));
    let layout = axis_layout(&fig, 24.0, 24.0);
    assert!(layout.x_ticks.values.is_empty());
    assert!(layout.y_ticks.values.is_empty());

    let plot = Projector::new(&fig, Rect::new(0.0, 0.0, 24.0, 24.0), &layout.margins).plot;
    assert!(plot.left.is_finite() && plot.top.is_finite());
    assert!(plot.width > 0.0 && plot.height > 0.0);
}

#[test]
fn categorical_layout_accounts_for_long_names() {
    let short = Figure::new(
        "",
        Axis::categorical("group", (0..8).map(|i| format!("c{i}")).collect()),
        Axis::new("y", 0.0, 1.0),
    );
    let long = Figure::new(
        "",
        Axis::categorical(
            "group",
            (0..8).map(|i| format!("long category name {i}")).collect(),
        ),
        Axis::new("y", 0.0, 1.0),
    );
    let short_ticks = axis_layout(&short, 420.0, 300.0).x_ticks;
    let long_ticks = axis_layout(&long, 420.0, 300.0).x_ticks;
    assert_eq!(short_ticks.values.len(), 8);
    assert!(long_ticks.values.len() < short_ticks.values.len());
}

#[test]
fn east_asian_wide_categories_use_full_em_width_for_thinning() {
    let fig = Figure::new(
        "",
        Axis::categorical(
            "group",
            ["分类一", "分类二", "分类三", "分类四", "分类五", "分类六"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        ),
        Axis::new("y", 0.0, 1.0),
    );
    let layout = axis_layout(&fig, 165.0, 150.0);
    assert!(
        layout.x_ticks.labels.len() < 6,
        "full-width category labels must be thinned: {:?}",
        layout.x_ticks.labels
    );

    let tick_pt = fig.typography.tick_pt;
    assert!((estimated_text_width("分类一", tick_pt) - 3.0 * tick_pt).abs() < 1e-3);
    assert!((estimated_text_width("𠀀", tick_pt) - tick_pt).abs() < 1e-3);
    assert!((estimated_text_width("e\u{301}", tick_pt) - 0.58 * tick_pt).abs() < 1e-3);
}

#[test]
fn aspect_lock_and_projection_bands_constrain_the_adaptive_budget() {
    use plotium_figure::AxisTrace;

    let base = Figure::new("", Axis::new("x", 0.0, 10.0), Axis::new("y", 0.0, 10.0));
    let base_count = axis_layout(&base, 600.0, 140.0).x_ticks.values.len();

    let mut constrained = base.clone();
    constrained.lock_aspect = true;
    constrained.left_projection = Some(AxisTrace {
        points: vec![[0.0, 0.0], [10.0, 1.0]],
        color: Color::TRACE,
        width: 1.0,
    });
    let constrained_count = axis_layout(&constrained, 600.0, 140.0).x_ticks.values.len();
    assert!(constrained_count < base_count);
}

#[test]
fn hidden_frame_collapses_margins_to_outer_pad() {
    let mut fig = Figure::new("", Axis::new("x", 0.0, 1.0), Axis::new("y", 0.0, 1.0));
    fig.axis_frame = plotium_figure::AxisFrame::Hidden;
    let m = Margins::for_figure(&fig);
    assert_eq!(m.left, OUTER_PAD);
    assert_eq!(m.right, OUTER_PAD);
    assert_eq!(m.top, OUTER_PAD);
    assert_eq!(m.bottom, OUTER_PAD);
}

#[test]
fn legend_merges_series_and_named_polygons_once() {
    let mut fig = Figure::new("", Axis::new("x", 0.0, 1.0), Axis::new("y", 0.0, 1.0));
    fig.series.push(plotium_figure::Series::line(
        "trace",
        vec![[0.0, 0.0], [1.0, 1.0]],
    ));
    for _ in 0..3 {
        fig.polygons.push(plotium_figure::Polygon::rect(
            "bars",
            0.0,
            0.5,
            0.0,
            1.0,
            Color::TRACE,
        ));
    }
    fig.polygons.push(plotium_figure::Polygon::rect(
        "",
        0.0,
        0.1,
        0.0,
        0.1,
        Color::TRACE,
    ));
    let entries = legend_entries(&fig);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].0, "trace");
    assert_eq!(entries[1].0, "bars");
    assert!(matches!(entries[1].2, LegendMark::Rect { .. }));
}

#[test]
fn bar_swatches_carry_the_bar_outline() {
    let mut fig = Figure::new("", Axis::new("x", 0.0, 2.0), Axis::new("y", 0.0, 2.0));
    let fill = Color::rgb(0xf5, 0xd7, 0x8e);
    let rect = |name: &str, x: f64| plotium_figure::Polygon::rect(name, x, x + 0.5, 0.0, 1.0, fill);
    fig.polygons
        .push(rect("outlined", 0.0).with_stroke(Color::BLACK, 0.5));
    fig.polygons.push(rect("plain", 0.5));
    // Stacked segments are separated in the background color, not outlined.
    fig.polygons
        .push(rect("stacked", 1.0).with_stroke(fig.background, 0.5));
    let edges: Vec<_> = legend_entries(&fig)
        .iter()
        .map(|(_, _, mark)| match mark {
            LegendMark::Rect { edge } => *edge,
            _ => panic!("polygons legend as rectangles"),
        })
        .collect();
    assert_eq!(edges, vec![Some((Color::BLACK, 0.5)), None, None]);
    let svg = svg::export(&fig);
    assert_eq!(
        svg.matches(r##"rx="1" fill="#f5d78e" stroke="#000000" stroke-width="0.5""##)
            .count(),
        1
    );
    assert_eq!(svg.matches(r##"rx="1" fill="#f5d78e"/>"##).count(), 2);
}

#[test]
fn axis_spines_are_drawn_over_data_resting_on_them() {
    let mut fig = Figure::new("", Axis::new("x", -0.5, 1.5), Axis::new("y", 0.0, 2.0));
    fig.polygons.push(
        plotium_figure::Polygon::rect("bar", 0.0, 1.0, 0.0, 1.0, Color::TRACE)
            .with_stroke(Color::BLACK, 0.5),
    );
    let svg = svg::export(&fig);
    let bar = svg.find("<polygon").expect("bar");
    // The open frame is the one path with absolute H and V segments; ticks
    // use relative h/v.
    let frame = svg
        .match_indices(r#"<path d="M"#)
        .map(|(i, _)| i)
        .find(|&i| {
            let element = &svg[i..i + svg[i..].find("/>").unwrap_or(0)];
            element.contains('H') && element.contains('V')
        })
        .expect("open axis frame");
    assert!(bar < frame, "spines must paint after the bar");
}

#[test]
fn legend_merges_same_name_points_and_line_into_one_semantic_entry() {
    let color = Color::rgb(20, 80, 160);
    let figure = Figure::new("", Axis::new("x", 0.0, 1.0), Axis::new("y", 0.0, 1.0))
        .with_series(Series::points("Region A", vec![[0.0, 0.0]]).colored(color))
        .with_series(Series::line("Region A", vec![[0.0, 0.0], [1.0, 1.0]]).colored(color))
        .with_series(Series::points("Region B", vec![[0.0, 1.0]]).colored(Color::rgb(160, 80, 20)));

    let entries = legend_entries(&figure);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].0, "Region A");
    assert!(matches!(entries[0].2, LegendMark::LinePoints));
}

#[test]
fn manual_legend_position_maps_to_the_available_plot_area() {
    let mut figure = Figure::new("", Axis::new("x", 0.0, 1.0), Axis::new("y", 0.0, 1.0))
        .with_series(Series::line("A", vec![[0.0, 0.0], [1.0, 1.0]]))
        .with_series(Series::line("B", vec![[0.0, 1.0], [1.0, 0.0]]));
    figure.guide_visibility = plotium_figure::GuideVisibility::Show;
    figure.legend_position = Some([0.0, 1.0]);
    let plot = Rect::new(10.0, 20.0, 300.0, 200.0);
    let legend = legend_rect(&figure, plot, 1.0).unwrap();
    assert_eq!(legend.left, plot.left);
    assert_eq!(legend.bottom(), plot.bottom());

    let position =
        legend_position_for_origin(&figure, plot, 1.0, [plot.right(), plot.top]).unwrap();
    assert_eq!(position, [1.0, 0.0]);
}

#[test]
fn guide_visibility_auto_show_and_hide_have_distinct_semantics() {
    let mut figure = Figure::new("", Axis::new("x", 0.0, 1.0), Axis::new("y", 0.0, 1.0))
        .with_series(Series::line("A", vec![[0.0, 0.0], [1.0, 1.0]]));
    assert!(
        !renders_legend(&figure),
        "Auto omits a redundant one-item key"
    );
    figure.guide_visibility = plotium_figure::GuideVisibility::Show;
    assert!(
        renders_legend(&figure),
        "Show permits a useful one-item key"
    );
    figure = figure.with_series(Series::line("B", vec![[0.0, 1.0], [1.0, 0.0]]));
    figure.guide_visibility = plotium_figure::GuideVisibility::Auto;
    assert!(renders_legend(&figure));
    figure.guide_visibility = plotium_figure::GuideVisibility::Hide;
    assert!(!renders_legend(&figure));
}

#[test]
fn horizontal_legend_and_title_change_the_shared_layout() {
    let mut figure = Figure::new("", Axis::new("x", 0.0, 1.0), Axis::new("y", 0.0, 1.0))
        .with_series(Series::line("A", vec![[0.0, 0.0], [1.0, 1.0]]))
        .with_series(Series::line("B", vec![[0.0, 1.0], [1.0, 0.0]]));
    let entries = legend_entries(&figure);
    let vertical = legend_layout(&figure, &entries);
    drop(entries);
    figure.guide_layout = plotium_figure::GuideLayout::Horizontal;
    let entries = legend_entries(&figure);
    let horizontal = legend_layout(&figure, &entries);
    assert!(horizontal.width > vertical.width);
    assert!(horizontal.height < vertical.height);
    drop(entries);
    figure.guide_title = "Channels".to_owned();
    let entries = legend_entries(&figure);
    assert!(legend_layout(&figure, &entries).height > horizontal.height);
}

#[test]
fn heatmap_auto_reserves_a_color_scale_and_hide_removes_it() {
    let mut figure = Figure::new("", Axis::new("x", 0.0, 2.0), Axis::new("y", 0.0, 2.0));
    figure.heatmap = Some(plotium_figure::HeatmapGrid {
        rows: 2,
        cols: 2,
        values: vec![0.0, 1.0, 2.0, 3.0],
        x_bounds: [0.0, 2.0],
        y_bounds: [0.0, 2.0],
        colormap: plotium_figure::ColormapId::Viridis,
        value_range: [0.0, 3.0],
    });
    assert!(renders_color_scale(&figure));
    let visible = Margins::for_figure(&figure);
    figure.guide_visibility = plotium_figure::GuideVisibility::Hide;
    assert!(!renders_color_scale(&figure));
    assert!(visible.right > Margins::for_figure(&figure).right);
}

#[test]
fn heatmap_cells_project_every_finite_cell() {
    let fig = Figure::new("", Axis::new("x", 0.0, 2.0), Axis::new("y", 0.0, 2.0));
    let grid = plotium_figure::HeatmapGrid {
        rows: 2,
        cols: 2,
        values: vec![0.0, 1.0, f32::NAN, 3.0],
        x_bounds: [0.0, 2.0],
        y_bounds: [0.0, 2.0],
        colormap: plotium_figure::ColormapId::Viridis,
        value_range: [0.0, 3.0],
    };
    let outer = Rect::new(0.0, 0.0, 200.0, 200.0);
    let proj = Projector::new(&fig, outer, &Margins::default());
    let cells = heatmap_cells(&proj, &grid);
    assert_eq!(cells.len(), 3, "NaN cell must be skipped");
    assert!(cells.iter().all(|(r, _)| r.width > 0.0 && r.height > 0.0));
}

#[test]
fn legends_draw_no_frame_or_backing() {
    let mut figure = Figure::new("", Axis::new("x", 0.0, 1.0), Axis::new("y", 0.0, 1.0))
        .with_series(Series::line("A", vec![[0.0, 0.0], [1.0, 1.0]]))
        .with_series(Series::line("B", vec![[0.0, 1.0], [1.0, 0.0]]));
    let with_legend = svg::export(&figure);
    assert!(with_legend.contains(">A</text>"));
    figure.guide_visibility = plotium_figure::GuideVisibility::Hide;
    let without_legend = svg::export(&figure);
    assert_eq!(
        with_legend.matches("<rect").count(),
        without_legend.matches("<rect").count(),
        "the legend adds entries only, never a box"
    );
}

#[test]
fn inward_ticks_point_into_the_plot_and_free_their_margin() {
    let outward = Figure::new("", Axis::new("x", 0.0, 1.0), Axis::new("y", 0.0, 1.0));
    let mut inward = outward.clone();
    inward.typography.tick_direction = TickDirection::In;

    let out_margins = Margins::for_figure(&outward);
    let in_margins = Margins::for_figure(&inward);
    let length = outward.typography.tick_length_pt;
    assert!((out_margins.left - in_margins.left - length).abs() < 1e-3);
    assert!((out_margins.bottom - in_margins.bottom - length).abs() < 1e-3);

    let svg = svg::export(&inward);
    assert!(svg.contains(&format!("v{}\"", -length)));
    assert!(!svg.contains(&format!("v{length}\"")));
}

#[test]
fn axis_strokes_and_tick_length_follow_the_figure_typography() {
    let mut fig = Figure::new("", Axis::new("x", 0.0, 1.0), Axis::new("y", 0.0, 1.0));
    fig.typography.axis_line_pt = 0.6;
    fig.typography.tick_length_pt = 2.5;
    let svg = svg::export(&fig);
    assert!(svg.contains(r#"stroke-width="0.6""#));
    assert!(!svg.contains(r#"stroke-width="1""#));
    assert!(svg.contains("v2.5\""));
}

#[test]
fn truncated_range_labels_never_leave_broken_script_markup() {
    let plot = Rect::new(0.0, 0.0, 70.0, 50.0);
    let label = range_label_layout(plot, 10.0, 40.0, 7.0, "Δδ / cm^{-2} region", None).unwrap();
    assert!(label.text.ends_with('…'));
    assert!(!label.text.contains('^') && !label.text.contains('{'));
}

#[test]
fn minor_ticks_split_every_major_interval_and_stay_inside_the_axis() {
    let axis = Axis::new("x", -0.3, 2.1);
    let majors = axis_ticks(axis.min, axis.max, 3);
    assert_eq!(majors.values, vec![0.0, 1.0, 2.0]);
    let minors = minor_tick_values(&axis, &majors, 1);
    assert_eq!(minors.len(), 2, "only the in-range half steps: {minors:?}");
    assert!((minors[0] - 0.5).abs() < 1e-9 && (minors[1] - 1.5).abs() < 1e-9);
    assert_eq!(minor_tick_values(&axis, &majors, 4).len(), 9);
    assert!(minor_tick_values(&axis, &majors, 0).is_empty());

    // A negative display scale lists majors in descending coordinate order.
    let mut shown = Axis::new("x", 0.0, 2.0).reversed(true);
    shown.display = Some(plotium_figure::AxisDisplay {
        scale: -1.0,
        offset: 0.0,
        unit: String::new(),
    });
    let majors = axis_ticks_for(&shown, 3);
    let mut minors = minor_tick_values(&shown, &majors, 1);
    minors.sort_by(f64::total_cmp);
    let expected = [0.25, 0.75, 1.25, 1.75];
    assert_eq!(minors.len(), expected.len(), "{minors:?}");
    assert!(
        minors
            .iter()
            .zip(expected)
            .all(|(a, b)| (a - b).abs() < 1e-9)
    );

    let categories = Axis::categorical("c", vec!["a".into(), "b".into(), "c".into()]);
    let ticks = axis_ticks_for(&categories, 3);
    assert!(minor_tick_values(&categories, &ticks, 1).is_empty());
}

#[test]
fn minor_ticks_are_half_length_and_follow_the_tick_direction() {
    let mut fig = Figure::new("", Axis::new("x", 0.0, 1.0), Axis::new("y", 0.0, 1.0));
    let plain = svg::export(&fig);
    let plain_margins = Margins::for_figure(&fig);
    fig.typography.minor_ticks = 1;
    let out = svg::export(&fig);
    assert!(!plain.contains("v1.5") && out.contains("v1.5"));
    let x_minors = minor_tick_values(&fig.x, &axis_layout(&fig, fig.width, fig.height).x_ticks, 1);
    assert_eq!(out.matches("v1.5").count(), x_minors.len());
    assert!(out.contains("h1.5"));
    assert_eq!(Margins::for_figure(&fig), plain_margins);

    fig.typography.tick_direction = TickDirection::In;
    let inward = svg::export(&fig);
    assert!(inward.contains("v-1.5"));
}

#[test]
fn y_title_moves_down_only_as_far_as_the_corner_letter_needs() {
    let mut fig = Figure::new(
        "",
        Axis::new("x", 0.0, 1.0),
        Axis::new("Voltage (V)", 0.0, 1.0),
    );
    let outer = Rect::new(0.0, 0.0, 150.0, 160.0);
    let plot = Rect::new(30.0, 6.0, 110.0, 130.0);
    let centre = plot.top + plot.height * 0.5;
    assert_eq!(y_title_center(&fig, plot, outer, 0.0, 1.0), centre);
    // A short title already clears the corner.
    assert_eq!(y_title_center(&fig, plot, outer, 30.0, 1.0), centre);

    fig.y.label = "Specific capacity (mAh g)".to_owned();
    let half = estimated_text_width(&fig.y.label, fig.typography.label_pt) * 0.5;
    let moved = y_title_center(&fig, plot, outer, 30.0, 1.0);
    assert!(
        (moved - half - 30.0).abs() < 1e-3,
        "top end sits just under the letter"
    );
    // Never pushed into the x-axis title row.
    let squeezed = y_title_center(&fig, plot, outer, 80.0, 1.0);
    assert!(squeezed + half <= outer.bottom() - OUTER_PAD - fig.typography.label_pt + 1e-3);
}
