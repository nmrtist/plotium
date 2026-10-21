use super::*;
use crate::curve_style::{MarkerPaint, dashed_segments, marker_polygon, marker_segments};
use plotium_figure::{LineStyle, Marker, Series, SeriesKind};

pub(super) fn paint(
    painter: &egui::Painter,
    proj: &Projector<'_>,
    series: &Series,
    scale: f32,
    detail: ScreenRenderDetail,
    stats: Option<&mut RenderStats>,
) {
    if series.points.is_empty() {
        return;
    }
    if series.kind == SeriesKind::Stick {
        super::sticks::paint(painter, proj, series, scale);
        return;
    }
    if series.kind == SeriesKind::Line && series.points.len() >= 2 {
        let columns = line_columns(detail, proj.plot.width, painter.ctx().pixels_per_point());
        let visible = screen_line_points(
            &series.points,
            proj.fig.x.min.min(proj.fig.x.max),
            proj.fig.x.min.max(proj.fig.x.max),
            columns,
        );
        if let Some(stats) = stats {
            stats.line_series_visited += 1;
            stats.record_line(
                visible.source_points_visited,
                visible.points.len(),
                visible.pooled,
            );
        }
        let pts: Vec<_> = visible.points.iter().map(|p| proj.project(*p)).collect();
        stroke(painter, &pts, series, scale, proj.plot);
    }
    let marker = series.appearance.marker(series.kind);
    if marker != Marker::None {
        let r = series.appearance.marker_radius(series.width) * scale;
        let paint = MarkerPaint::new(series.appearance, series.color, series.width).scaled(scale);
        for p in &series.points {
            marker_at(painter, proj.project(*p), marker, r, &paint);
        }
    }
}
pub(super) fn stroke(
    painter: &egui::Painter,
    pts: &[(f32, f32)],
    series: &Series,
    scale: f32,
    clip: Rect,
) {
    let stroke = Stroke::new(series.width * scale, col(series.color));
    if series.appearance.line_style == LineStyle::Solid {
        painter.add(Shape::line(
            pts.iter().map(|&(x, y)| Pos2::new(x, y)).collect(),
            stroke,
        ));
    } else {
        for [a, b] in dashed_segments(
            pts,
            series.appearance.line_style,
            series.width * scale,
            clip,
        ) {
            painter.line_segment([Pos2::new(a.0, a.1), Pos2::new(b.0, b.1)], stroke);
        }
    }
}
pub(super) fn marker_at(
    painter: &egui::Painter,
    p: (f32, f32),
    marker: Marker,
    r: f32,
    paint: &MarkerPaint,
) {
    let edge = paint.edge.map_or(Stroke::NONE, |(color, width)| {
        Stroke::new(width, col(color))
    });
    if marker == Marker::Circle {
        painter.circle(Pos2::new(p.0, p.1), r, col(paint.fill), edge);
    }
    let polygon = marker_polygon(marker, p, r);
    if !polygon.is_empty() {
        painter.add(Shape::convex_polygon(
            polygon.into_iter().map(|(x, y)| Pos2::new(x, y)).collect(),
            col(paint.fill),
            edge,
        ));
    }
    for [a, b] in marker_segments(marker, p, r) {
        painter.line_segment(
            [Pos2::new(a.0, a.1), Pos2::new(b.0, b.1)],
            Stroke::new(paint.width, col(paint.color)),
        );
    }
}
