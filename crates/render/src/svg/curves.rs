use super::*;
use crate::curve_style::{MarkerPaint, dash_pattern, marker_polygon, marker_segments};
use plotium_figure::{LineStyle, Marker, Series, SeriesKind};

pub(super) fn write(
    s: &mut String,
    proj: &Projector<'_>,
    series: &Series,
    index: usize,
    lod: Option<&mut super::preview::PreviewLod>,
) {
    if series.points.is_empty() {
        return;
    }
    let proj = &proj.on(series.appearance.y_axis);
    if series.kind == SeriesKind::Stick {
        super::sticks::write(s, proj, series);
        return;
    }
    let mut drawn = 0;
    let mut thinned = false;
    if series.kind == SeriesKind::Line {
        let reduced = lod
            .as_deref()
            .and_then(|lod| lod.line(proj, &series.points));
        thinned = reduced.as_ref().is_some_and(|(_, pooled)| *pooled);
        let line = reduced
            .as_ref()
            .map_or(series.points.as_slice(), |(points, _)| points);
        drawn = line.len();
        let points: Vec<_> = line.iter().map(|p| proj.project(*p)).collect();
        for run in crate::finite_runs(&points).filter(|run| run.len() >= 2) {
            if series.occludes {
                fill_to_baseline(s, proj, series, run);
            }
            stroke(s, run, series);
        }
    }
    let marker = series.appearance.marker(series.kind);
    if marker != Marker::None {
        let paint = MarkerPaint::new(series.appearance, series.color, series.width);
        let r = series.appearance.marker_radius(series.width);
        let mut points: Vec<_> = series
            .points
            .iter()
            .step_by(series.appearance.marker_stride())
            .map(|p| proj.project(*p))
            .filter(|p| crate::is_finite_point(*p))
            .collect();
        let deduped = lod
            .as_deref()
            .is_some_and(|lod| lod.markers(proj, &mut points));
        if series.kind != SeriesKind::Line {
            thinned = deduped;
            drawn = points.len();
        }
        for p in points {
            marker_at(s, p, marker, r, &paint);
        }
    }
    if let Some(lod) = lod
        && thinned
    {
        lod.record(index, series, drawn);
    }
}
pub(super) fn stroke(s: &mut String, points: &[(f32, f32)], series: &Series) {
    let pts = points
        .iter()
        .map(|(x, y)| format!("{x},{y}"))
        .collect::<Vec<_>>()
        .join(" ");
    let dash = if series.appearance.line_style == LineStyle::Solid {
        String::new()
    } else {
        format!(
            " stroke-dasharray=\"{}\"",
            dash_pattern(series.appearance.line_style, series.width)
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    let _ = write!(
        s,
        r#"<polyline points="{pts}" fill="none" stroke="{col}" stroke-width="{w}"{dash}/>"#,
        col = series.color.to_hex(),
        w = series.width
    );
}
pub(super) fn marker_at(
    s: &mut String,
    p: (f32, f32),
    marker: Marker,
    r: f32,
    paint: &MarkerPaint,
) {
    let (col, width) = (paint.color.to_hex(), paint.width);
    let fill = paint.fill.map_or("none".into(), |fill| fill.to_hex());
    let edge = paint.edge.map_or(String::new(), |(color, width)| {
        format!(r#" stroke="{}" stroke-width="{width}""#, color.to_hex())
    });
    if marker == Marker::Circle {
        let _ = write!(
            s,
            r#"<circle cx="{x:.2}" cy="{y:.2}" r="{r:.2}" fill="{fill}"{edge}/>"#,
            x = p.0,
            y = p.1
        );
    }
    let polygon = marker_polygon(marker, p, r);
    if !polygon.is_empty() {
        let pts = polygon
            .iter()
            .map(|(x, y)| format!("{x},{y}"))
            .collect::<Vec<_>>()
            .join(" ");
        let _ = write!(s, r#"<polygon points="{pts}" fill="{fill}"{edge}/>"#);
    }
    for [a, b] in marker_segments(marker, p, r) {
        let _ = write!(
            s,
            r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{col}" stroke-width="{width}"/>"#,
            x1 = a.0,
            y1 = a.1,
            x2 = b.0,
            y2 = b.1
        );
    }
}

fn fill_to_baseline(s: &mut String, proj: &Projector<'_>, series: &Series, run: &[(f32, f32)]) {
    let Some(baseline) = crate::baseline_y(proj, series) else {
        return;
    };
    let mut points = String::new();
    for (x, y) in crate::baseline_fill_outline(run, baseline) {
        let _ = write!(points, "{x:.2},{y:.2} ");
    }
    let _ = write!(
        s,
        r#"<polygon points="{points}" fill="{fill}"/>"#,
        fill = proj.fig.background.to_hex(),
    );
}
