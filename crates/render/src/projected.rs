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
        [self.fig.x.value_at(tx), self.fig.y.value_at(ty)]
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
    let x = error_bar.center[0];
    let low_y = error_bar.center[1] - error_bar.negative;
    let high_y = error_bar.center[1] + error_bar.positive;
    if !low_y.is_finite() || !high_y.is_finite() {
        return None;
    }
    let low_raw = proj.project([x, low_y]);
    let high_raw = proj.project([x, high_y]);
    if !is_finite_point(high_raw) {
        return None;
    }
    let low = clip_to_plot(low_raw, proj.plot)?;
    let half_cap = error_bar.cap_width * output_scale * 0.5;
    let cap = |(x, y): (f32, f32)| [(x - half_cap, y), (x + half_cap, y)];
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
            center: [5.0, center],
            negative,
            positive: 5.0,
            color: Color::BLACK,
            width: 1.0,
            cap_width: 4.0,
            draw_over_data: false,
        };
        let segments = error_bar_segments(&proj, &bar(10.0, 20.0), 1.0).expect("visible top");
        assert_eq!(segments.as_slice().len(), 2, "stem and upper cap only");
        assert_eq!(segments.as_slice()[0][0].1, proj.plot.bottom());
        let segments = error_bar_segments(&proj, &bar(10.0, 5.0), 1.0).expect("inside");
        assert_eq!(segments.as_slice().len(), 3);
        assert!(error_bar_segments(&proj, &bar(-10.0, 1.0), 1.0).is_none());
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
