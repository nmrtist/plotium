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
    let proj = &proj.on(series.appearance.y_axis);
    if series.kind == SeriesKind::Stick {
        super::sticks::paint(painter, proj, series, scale);
        return;
    }
    if series.kind == SeriesKind::Line && series.points.len() >= 2 {
        let columns = line_columns(detail, proj.plot.width, painter.ctx().pixels_per_point());
        let visible = scaled_line_points(
            &series.points,
            &proj.fig.x,
            !proj.y_axis().effective_scale().is_linear(),
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
        for run in crate::finite_runs(&pts).filter(|run| run.len() >= 2) {
            if series.occludes {
                fill_to_baseline(painter, proj, series, run);
            }
            stroke(painter, run, series, scale, proj.plot);
        }
    }
    let marker = series.appearance.marker(series.kind);
    if marker != Marker::None {
        let r = series.appearance.marker_radius(series.width) * scale;
        let paint = MarkerPaint::new(series.appearance, series.color, series.width).scaled(scale);
        for p in series
            .points
            .iter()
            .step_by(series.appearance.marker_stride())
        {
            let p = proj.project(*p);
            if crate::is_finite_point(p) {
                marker_at(painter, p, marker, r, &paint);
            }
        }
    }
}
/// A triangle strip, since the area under a curve is not convex and one mesh
/// has no seams between its cells.
fn fill_to_baseline(
    painter: &egui::Painter,
    proj: &Projector<'_>,
    series: &Series,
    run: &[(f32, f32)],
) {
    let Some(baseline) = crate::baseline_y(proj, series) else {
        return;
    };
    let color = col(proj.fig.background);
    let mut mesh = egui::Mesh::default();
    for (index, &(x, y)) in run.iter().enumerate() {
        mesh.colored_vertex(Pos2::new(x, y), color);
        mesh.colored_vertex(Pos2::new(x, baseline), color);
        if index > 0 {
            let right = 2 * index as u32;
            mesh.add_triangle(right - 2, right - 1, right);
            mesh.add_triangle(right - 1, right, right + 1);
        }
    }
    painter.add(mesh);
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
    let fill = paint.fill.map_or(Color32::TRANSPARENT, col);
    if marker == Marker::Circle {
        painter.circle(Pos2::new(p.0, p.1), r, fill, edge);
    }
    let polygon: Vec<_> = marker_polygon(marker, p, r)
        .into_iter()
        .map(|(x, y)| Pos2::new(x, y))
        .collect();
    if marker == Marker::Star {
        concave_marker(painter, Pos2::new(p.0, p.1), polygon, fill, edge);
    } else if !polygon.is_empty() {
        painter.add(Shape::convex_polygon(polygon, fill, edge));
    }
    for [a, b] in marker_segments(marker, p, r) {
        painter.line_segment(
            [Pos2::new(a.0, a.1), Pos2::new(b.0, b.1)],
            Stroke::new(paint.width, col(paint.color)),
        );
    }
}
/// egui fills only convex paths, so a star-shaped marker is fanned from its
/// centre into an unfeathered mesh. Without an edge, a one-pixel outline in the
/// fill colour restores the anti-aliased boundary the mesh lacks.
fn concave_marker(
    painter: &egui::Painter,
    centre: Pos2,
    polygon: Vec<Pos2>,
    fill: Color32,
    edge: Stroke,
) {
    if fill != Color32::TRANSPARENT {
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(centre, fill);
        for &point in &polygon {
            mesh.colored_vertex(point, fill);
        }
        let n = polygon.len() as u32;
        for k in 0..n {
            mesh.add_triangle(0, k + 1, (k + 1) % n + 1);
        }
        painter.add(Shape::mesh(mesh));
    }
    let outline = if edge.is_empty() && fill != Color32::TRANSPARENT {
        Stroke::new(1. / painter.pixels_per_point(), fill)
    } else {
        edge
    };
    if !outline.is_empty() {
        painter.add(Shape::closed_line(polygon, outline));
    }
}
