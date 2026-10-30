use super::*;
use crate::curve_style::{MarkerPaint, dashed_segments, marker_polygon, marker_segments};
use plotium_figure::{LineStyle, Marker, Series};
pub(super) fn write(dc: &mut Dc, proj: &Projector<'_>, series: &Series) {
    if series.points.is_empty() {
        return;
    }
    let proj = &proj.on(series.appearance.y_axis);
    let points: Vec<_> = series.points.iter().map(|p| proj.project(*p)).collect();
    match series.kind {
        SeriesKind::Line => {
            for run in crate::finite_runs(&points).filter(|run| run.len() >= 2) {
                if let Some(baseline) = crate::baseline_y(proj, series).filter(|_| series.occludes)
                {
                    let outline = crate::baseline_fill_outline(run, baseline);
                    dc.polygon(&outline, Some(proj.fig.background), None);
                }
                stroke(dc, run, series, proj.plot);
            }
        }
        SeriesKind::Stick => {
            let segments = series
                .points
                .iter()
                .filter_map(|p| crate::stick_segment(proj, *p))
                .collect::<Vec<_>>();
            dc.segments(&segments, series.color, series.width);
            return;
        }
        _ => {}
    }
    let marker = series.appearance.marker(series.kind);
    if marker != Marker::None {
        let paint = MarkerPaint::new(series.appearance, series.color, series.width);
        let r = series.appearance.marker_radius(series.width);
        for p in points
            .into_iter()
            .step_by(series.appearance.marker_stride())
            .filter(|p| crate::is_finite_point(*p))
        {
            marker_at(dc, p, marker, r, &paint);
        }
    }
}
pub(super) fn stroke(dc: &mut Dc, points: &[(f32, f32)], series: &Series, clip: Rect) {
    if series.appearance.line_style == LineStyle::Solid {
        dc.polyline(points, series.color, series.width);
    } else {
        dc.segments(
            &dashed_segments(points, series.appearance.line_style, series.width, clip),
            series.color,
            series.width,
        );
    }
}
pub(super) fn marker_at(dc: &mut Dc, p: (f32, f32), marker: Marker, r: f32, paint: &MarkerPaint) {
    if marker == Marker::Circle {
        dc.ellipse(
            Rect::new(p.0 - r, p.1 - r, r * 2., r * 2.),
            paint.fill,
            paint.edge,
        );
    }
    let polygon = marker_polygon(marker, p, r);
    if !polygon.is_empty() {
        dc.polygon(&polygon, paint.fill, paint.edge);
    }
    dc.segments(&marker_segments(marker, p, r), paint.color, paint.width);
}
