//! Output-space geometry shared by every backend for values that may have no
//! finite position: zero and negative values on a log axis, or NaN data.

use crate::{Projector, Rect};
use plotium_figure::ErrorBar;

pub(crate) fn is_finite_point(point: (f32, f32)) -> bool {
    point.0.is_finite() && point.1.is_finite()
}

/// Maximal runs of consecutive drawable points. A point without a finite
/// position breaks the stroke there instead of joining its neighbours across
/// the gap.
pub(crate) fn finite_runs(points: &[(f32, f32)]) -> impl Iterator<Item = &[(f32, f32)]> {
    points
        .split(|point| !is_finite_point(*point))
        .filter(|run| !run.is_empty())
}

/// Output y of a series baseline, on the plot edge when a log axis sends it to
/// infinity.
pub(crate) fn baseline_y(
    proj: &crate::Projector<'_>,
    series: &plotium_figure::Series,
) -> Option<f32> {
    let (x, y) = proj.project([proj.fig.x.min, series.display_baseline]);
    clip_to_plot((x, y), proj.plot).map(|point| point.1)
}

/// The closed outline between a projected curve run and its baseline.
pub(crate) fn baseline_fill_outline(run: &[(f32, f32)], baseline: f32) -> Vec<(f32, f32)> {
    let mut outline = run.to_vec();
    outline.extend(run.iter().rev().map(|&(x, _)| (x, baseline)));
    outline
}

/// Clip a coordinate that a log axis sent to infinity onto the plot edge it
/// lies beyond. NaN has no edge and stays unplaceable.
pub(crate) fn clip_to_plot(point: (f32, f32), plot: Rect) -> Option<(f32, f32)> {
    let clip = |value: f32, low: f32, high: f32| match value {
        v if v.is_nan() => None,
        f32::NEG_INFINITY => Some(low),
        f32::INFINITY => Some(high),
        v => Some(v),
    };
    Some((
        clip(point.0, plot.left, plot.right())?,
        clip(point.1, plot.top, plot.bottom())?,
    ))
}

impl Projector<'_> {
    /// Output `(px, py)` → data `[x, y]`; the inverse of [`Projector::project`].
    pub fn unproject(&self, point: (f32, f32)) -> [f64; 2] {
        let tx = f64::from((point.0 - self.plot.left) / self.plot.width);
        let ty = 1.0 - f64::from((point.1 - self.plot.top) / self.plot.height);
        [self.fig.x.value_at(tx), self.y_axis().value_at(ty)]
    }
}

/// One stick from the zero baseline to `point`. The baseline of a log y axis
/// lies below every decade, so it clips to the plot's bottom edge.
pub(crate) fn stick_segment(proj: &Projector<'_>, point: [f64; 2]) -> Option<[(f32, f32); 2]> {
    let top = proj.project(point);
    if !is_finite_point(top) {
        return None;
    }
    let baseline = clip_to_plot(proj.project([point[0], 0.0]), proj.plot)?;
    Some([baseline, top])
}

/// Output x of a range annotation's two ends. An end at or below zero on a
/// log x axis clips to the left or right plot edge it lies beyond.
pub(crate) fn range_annotation_xs(
    proj: &Projector<'_>,
    annotation: &plotium_figure::RangeAnnotation,
) -> Option<(f32, f32)> {
    let y = proj.fig.y.min;
    let (x0, _) = clip_to_plot(proj.project([annotation.x0, y]), proj.plot)?;
    let (x1, _) = clip_to_plot(proj.project([annotation.x1, y]), proj.plot)?;
    Some((x0, x1))
}

/// Project one vertical uncertainty whisker into its stem and caps.
/// `output_scale` converts the cap's logical-unit width for zoomed screen
/// output. An end at or below zero on a log axis is clipped to the plot edge
/// and loses its cap, which would otherwise mark a bound that is not there.
pub(crate) fn error_bar_segments(
    proj: &Projector<'_>,
    error_bar: &ErrorBar,
    output_scale: f32,
) -> Option<ErrorBarSegments> {
    let proj = &proj.on(error_bar.y_axis);
    if !error_bar.center[0].is_finite()
        || !error_bar.center[1].is_finite()
        || !error_bar.negative.is_finite()
        || error_bar.negative < 0.0
        || !error_bar.positive.is_finite()
        || error_bar.positive < 0.0
        || !error_bar.width.is_finite()
        || error_bar.width <= 0.0
        || !error_bar.cap_width.is_finite()
        || error_bar.cap_width < 0.0
        || !output_scale.is_finite()
        || output_scale <= 0.0
    {
        return None;
    }
    // A horizontal whisker runs along x; `along` is its measured axis and
    // `across` the one its caps span.
    let horizontal = error_bar.horizontal;
    let (along, across) = if horizontal {
        (error_bar.center[0], error_bar.center[1])
    } else {
        (error_bar.center[1], error_bar.center[0])
    };
    let at = |along: f64, across: f64| {
        if horizontal {
            [along, across]
        } else {
            [across, along]
        }
    };
    let low_value = along - error_bar.negative;
    let high_value = along + error_bar.positive;
    if !low_value.is_finite() || !high_value.is_finite() {
        return None;
    }
    let low_raw = proj.project(at(low_value, across));
    let high_raw = proj.project(at(high_value, across));
    if !is_finite_point(high_raw) {
        return None;
    }
    let low = clip_to_plot(low_raw, proj.plot)?;
    // A data-space cap is measured on the projected axis so it keeps its share
    // of the mark it belongs to.
    let half_cap = error_bar
        .cap_data_width
        .filter(|w| w.is_finite() && *w > 0.0)
        .map(|w| {
            let a = proj.project(at(high_value, across - 0.5 * w));
            let b = proj.project(at(high_value, across + 0.5 * w));
            0.5 * if horizontal { b.1 - a.1 } else { b.0 - a.0 }.abs()
        })
        .filter(|half| half.is_finite())
        .unwrap_or(error_bar.cap_width * output_scale * 0.5);
    let cap = |(x, y): (f32, f32)| {
        if horizontal {
            [(x, y - half_cap), (x, y + half_cap)]
        } else {
            [(x - half_cap, y), (x + half_cap, y)]
        }
    };
    let mut segments = ErrorBarSegments {
        segments: [[low, high_raw], cap(high_raw), cap(low)],
        len: 2,
    };
    if is_finite_point(low_raw) {
        segments.len = 3;
    }
    Some(segments)
}

/// Stem, upper cap and, unless clipped, lower cap of one error bar, without a
/// per-bar allocation on the screen path.
pub(crate) struct ErrorBarSegments {
    segments: [[(f32, f32); 2]; 3],
    len: usize,
}

impl ErrorBarSegments {
    pub(crate) fn as_slice(&self) -> &[[(f32, f32); 2]] {
        &self.segments[..self.len]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Margins;
    use plotium_figure::{Axis, AxisScale, Color, Figure};

    #[test]
    fn strokes_break_at_points_without_a_position() {
        let nan = f32::NAN;
        let points = [
            (0.0, 1.0),
            (1.0, 2.0),
            (2.0, f32::INFINITY),
            (3.0, 1.0),
            (nan, 0.0),
            (4.0, 4.0),
            (5.0, 5.0),
        ];
        let runs: Vec<_> = finite_runs(&points).collect();
        assert_eq!(runs.len(), 3);
        assert_eq!(runs[0], &points[0..2]);
        assert_eq!(runs[1], &points[3..4]);
        assert_eq!(runs[2], &points[5..7]);
        assert_eq!(finite_runs(&[]).count(), 0);
    }

    fn log_figure() -> Figure {
        let mut y = Axis::new("y", 1.0, 100.0);
        y.scale = AxisScale::Log10;
        Figure::new("", Axis::new("x", 0.0, 10.0), y)
    }

    #[test]
    fn unproject_inverts_project_on_log_axes() {
        let fig = log_figure();
        let proj = Projector::new(&fig, Rect::new(0.0, 0.0, 200.0, 100.0), &Margins::default());
        let point = proj.project([2.5, 10.0]);
        let [x, y] = proj.unproject(point);
        assert!((x - 2.5).abs() < 1e-4);
        assert!((y - 10.0).abs() < 1e-4);
    }

    #[test]
    fn non_positive_error_bar_ends_clip_to_the_plot_edge_without_a_cap() {
        let fig = log_figure();
        let proj = Projector::new(&fig, Rect::new(0.0, 0.0, 200.0, 100.0), &Margins::default());
        let bar = |center: f64, negative: f64| ErrorBar {
            display_baseline: 0.0,
            center: [5.0, center],
            negative,
            positive: 5.0,
            color: Color::BLACK,
            width: 1.0,
            cap_width: 4.0,
            draw_over_data: false,
            cap_data_width: None,
            y_axis: plotium_figure::YAxisSide::Left,
            horizontal: false,
        };
        let segments = error_bar_segments(&proj, &bar(10.0, 20.0), 1.0).expect("visible top");
        assert_eq!(segments.as_slice().len(), 2, "stem and upper cap only");
        assert_eq!(segments.as_slice()[0][0].1, proj.plot.bottom());
        let segments = error_bar_segments(&proj, &bar(10.0, 5.0), 1.0).expect("inside");
        assert_eq!(segments.as_slice().len(), 3);
        assert!(error_bar_segments(&proj, &bar(-10.0, 1.0), 1.0).is_none());
    }

    #[test]
    fn a_horizontal_whisker_runs_along_x_with_upright_caps() {
        let fig = Figure::new("", Axis::new("x", 0.0, 10.0), Axis::new("y", 0.0, 10.0));
        let proj = Projector::new(&fig, Rect::new(0.0, 0.0, 200.0, 100.0), &Margins::default());
        let mut bar = ErrorBar::symmetric([5.0, 5.0], 1.0).with_cap_data_width(0.5);
        bar.horizontal = true;
        let segments = error_bar_segments(&proj, &bar, 1.0).expect("visible");
        let [stem, cap, _] = [
            segments.as_slice()[0],
            segments.as_slice()[1],
            segments.as_slice()[2],
        ];
        assert!((stem[0].1 - stem[1].1).abs() < 1e-3, "stem is horizontal");
        assert!((proj.project([6.0, 5.0]).0 - stem[1].0).abs() < 1e-3);
        assert!((cap[0].0 - cap[1].0).abs() < 1e-3, "cap is upright");
        let unit = (proj.project([5.0, 0.0]).1 - proj.project([5.0, 1.0]).1).abs();
        assert!(((cap[1].1 - cap[0].1).abs() - 0.5 * unit).abs() < 1e-3);
    }

    #[test]
    fn a_data_width_cap_spans_its_projected_share_of_the_x_axis() {
        let fig = Figure::new("", Axis::new("x", 0.0, 10.0), Axis::new("y", 0.0, 10.0));
        let proj = Projector::new(&fig, Rect::new(0.0, 0.0, 200.0, 100.0), &Margins::default());
        let unit = proj.project([1.0, 5.0]).0 - proj.project([0.0, 5.0]).0;
        let bar = ErrorBar::symmetric([5.0, 5.0], 1.0).with_cap_data_width(0.5);
        let segments = error_bar_segments(&proj, &bar, 3.0).expect("visible");
        let [(x0, _), (x1, _)] = segments.as_slice()[1];
        // Half a data unit wide whatever the output scale; a point cap ignores the axis.
        assert!(((x1 - x0) - 0.5 * unit).abs() < 1e-3);
        let fixed = error_bar_segments(&proj, &ErrorBar::symmetric([5.0, 5.0], 1.0), 1.0).unwrap();
        let [(x0, _), (x1, _)] = fixed.as_slice()[1];
        assert!(((x1 - x0) - 8.0).abs() < 1e-3);
    }

    #[test]
    fn svg_export_of_a_log_plot_breaks_lines_and_labels_decades() {
        let mut fig = log_figure();
        fig.y.min = 0.1;
        fig.y.max = 1000.0;
        fig.series.push(plotium_figure::Series::line(
            "s",
            vec![
                [0.0, 1.0],
                [1.0, 10.0],
                [2.0, 0.0],
                [3.0, 100.0],
                [4.0, 500.0],
            ],
        ));
        let svg = crate::svg::export(&fig);
        assert!(!svg.contains("NaN") && !svg.contains("inf"), "{svg}");
        assert_eq!(svg.matches("<polyline").count(), 2);
        assert!(svg.contains(">10<"), "decade labels draw a base of ten");
        assert!(
            !svg.contains("10^{"),
            "rich-text markup is rendered, not printed"
        );
        assert!(!svg.contains("×10"), "log axes have no multiplier label");
    }

    #[test]
    fn decade_labels_use_the_figure_typeface_and_weight() {
        let mut fig = log_figure();
        fig.typography.typeface = plotium_figure::Typeface::TimesNewRoman;
        fig.typography.font_weight = plotium_figure::FontWeight::Bold;
        fig.series.push(plotium_figure::Series::line(
            "s",
            vec![[0.0, 1.0], [4.0, 100.0]],
        ));
        let svg = crate::svg::export(&fig);
        let group = svg
            .find(r#"font-family="'Times New Roman', Times, serif" font-weight="bold""#)
            .expect("plot text group carries the chosen face and weight");
        let label = svg.find(">10<").expect("a decade label is drawn");
        assert!(
            label > group,
            "decade labels sit inside the plot text group"
        );
    }
}
