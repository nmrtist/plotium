use super::*;
use crate::{
    Projector,
    curve_style::{clip_segment, dash_pattern, marker_polygon, marker_segments},
};
use plotium_figure::{LineStyle, Marker, Series, SeriesKind};

/// Point ordinals are positions in one immutable rendered run, never resource IDs.
#[derive(Clone, Debug, Default)]
pub struct CurveScan {
    pub next_point: usize,
    phase: f64,
    // Duplicate vertices do not change SVG tangents. Retain the start of the
    // last nonzero edge across batches instead of rescanning previous points.
    incoming_start: Option<(f32, f32)>,
}
#[derive(Default)]
pub struct CurveScanResult {
    pub visited: usize,
    pub visible: usize,
    pub clipped: usize,
    pub intersections: usize,
    pub first_intersection: Option<Rect>,
    pub first_clipped: Option<Rect>,
    pub invalid: usize,
    pub finished: bool,
    pub cancelled: bool,
}

/// Walk every source point, including the line entering it. Budgeting does not
/// sample or pool points; the next call resumes the same run, dash phase and tangent.
pub fn scan_curve(
    series: &Series,
    projector: &Projector<'_>,
    clip: Option<Rect>,
    legend: Option<Rect>,
    state: &mut CurveScan,
    budget: usize,
    cancelled: &impl Fn() -> bool,
) -> CurveScanResult {
    let mut out = CurveScanResult::default();
    while state.next_point < series.points.len() && out.visited < budget {
        if out.visited % 256 == 0 && cancelled() {
            out.cancelled = true;
            return out;
        }
        let i = state.next_point;
        let b = projector.project(series.points[i]);
        state.next_point += 1;
        out.visited += 1;
        if !b.0.is_finite() || !b.1.is_finite() {
            state.incoming_start = None;
            out.invalid += 1;
            continue;
        }
        let marker = series.appearance.marker(series.kind);
        if marker != Marker::None {
            let radius = series.appearance.marker_radius(series.width);
            if matches!(marker, Marker::Plus | Marker::Cross) && series.width <= 0. {
                continue;
            }
            let envelope = if marker == Marker::Cross {
                radius + series.width * 0.5 / 2f32.sqrt()
            } else if marker == Marker::Plus {
                radius.max(series.width * 0.5)
            } else {
                // A closed marker's outline is centred on its boundary.
                radius
                    + series
                        .appearance
                        .marker_edge(series.color)
                        .map_or(0., |(_, width)| width * 0.5)
            };
            let bounds = Rect::new(b.0 - envelope, b.1 - envelope, 2. * envelope, 2. * envelope);
            let visible = clip.is_some_and(|r| marker_hits(marker, b, radius, series.width, r));
            record(
                &mut out,
                bounds,
                visible,
                clip,
                !clip.is_some_and(|c| contains(c, bounds)),
                legend.filter(|l| {
                    clip.and_then(|c| intersection(c, *l))
                        .is_some_and(|r| marker_hits(marker, b, radius, series.width, r))
                }),
            );
        }
        if i == 0 || series.kind != SeriesKind::Line {
            continue;
        }
        let a = projector.project(series.points[i - 1]);
        let length = (f64::from(b.0) - f64::from(a.0)).hypot(f64::from(b.1) - f64::from(a.1));
        if !length.is_finite() {
            state.incoming_start = None;
            out.invalid += 1;
            continue;
        }
        if length == 0. || series.width <= 0. {
            continue;
        }
        if !series.width.is_finite() {
            out.invalid += 1;
            continue;
        }
        let radius = series.width * 0.5;
        let nx = (b.1 - a.1).abs() / length as f32 * radius;
        let ny = (b.0 - a.0).abs() / length as f32 * radius;
        let bounds = Rect::new(
            a.0.min(b.0) - nx,
            a.1.min(b.1) - ny,
            (a.0 - b.0).abs() + 2. * nx,
            (a.1 - b.1).abs() + 2. * ny,
        );
        let pattern = dash_pattern(series.appearance.line_style, series.width);
        // SVG's native polyline uses miter joins (default limit 4). A sharp
        // turn can extend beyond either segment's half-width envelope.
        if let Some(before) = state.incoming_start.replace(a)
            && painted_interval(&pattern, state.phase - 1e-6, state.phase)
            && painted_interval(&pattern, state.phase, state.phase + 1e-6)
            && let Some(poly) = join_polygon(before, a, b, radius)
        {
            let bounds = polygon_bounds(&poly);
            let visible = clip.is_some_and(|r| polygon_hits(&poly, r));
            let overlap = legend.filter(|l| {
                clip.and_then(|c| intersection(c, *l))
                    .is_some_and(|r| polygon_hits(&poly, r))
            });
            record(
                &mut out,
                bounds,
                visible,
                clip,
                !clip.is_some_and(|c| contains(c, bounds)),
                overlap,
            );
        }
        let hits = |r: Rect| {
            stroke_interval(a, b, series.width, r).is_some_and(|(lo, hi)| {
                painted_interval(
                    &pattern,
                    state.phase + lo * length,
                    state.phase + hi * length,
                )
            })
        };
        let visible = clip.is_some_and(hits);
        let overlap = legend.filter(|l| clip.and_then(|c| intersection(c, *l)).is_some_and(hits));
        // Detect painted intervals outside the fully contained stroke envelope,
        // not an unpainted dash gap outside the clip.
        let inside = clip.and_then(|c| {
            let inner = Rect::new(
                c.left + nx,
                c.top + ny,
                c.width - 2. * nx,
                c.height - 2. * ny,
            );
            (inner.width > 0. && inner.height > 0.)
                .then(|| clip_segment(a, b, inner))
                .flatten()
        });
        let paint = |lo: f64, hi: f64| {
            hi > lo
                && painted_interval(
                    &pattern,
                    state.phase + lo * length,
                    state.phase + hi * length,
                )
        };
        let clipped =
            inside.map_or_else(|| paint(0., 1.), |(lo, hi)| paint(0., lo) || paint(hi, 1.));
        record(&mut out, bounds, visible, clip, clipped, overlap);
        if series.appearance.line_style != LineStyle::Solid {
            let period: f64 = pattern.iter().map(|v| f64::from(*v)).sum();
            state.phase = (state.phase + length).rem_euclid(period);
        }
    }
    out.finished = state.next_point == series.points.len();
    out
}
fn record(
    out: &mut CurveScanResult,
    bounds: Rect,
    visible: bool,
    clip: Option<Rect>,
    clipped: bool,
    overlap: Option<Rect>,
) {
    if clipped {
        out.clipped += 1;
        out.first_clipped.get_or_insert(bounds);
    }
    if visible {
        out.visible += 1;
    }
    if visible
        && let Some(r) = overlap
            .and_then(|l| clip.and_then(|c| intersection(c, l)))
            .and_then(|r| intersection(r, bounds))
    {
        out.intersections += 1;
        out.first_intersection.get_or_insert(r);
    }
}
fn painted_interval(pattern: &[f32], begin: f64, end: f64) -> bool {
    if pattern.is_empty() {
        return true;
    }
    let period: f64 = pattern.iter().map(|v| f64::from(*v)).sum();
    if end - begin >= period {
        return true;
    }
    let start = begin.rem_euclid(period);
    let finish = start + (end - begin);
    let mut offset = 0.;
    for (i, d) in pattern.iter().enumerate() {
        let next = offset + f64::from(*d);
        if i % 2 == 0
            && ((start < next && finish > offset)
                || (finish > period + offset && start < period + next))
        {
            return true;
        }
        offset = next;
    }
    false
}
fn marker_hits(marker: Marker, p: (f32, f32), radius: f32, width: f32, r: Rect) -> bool {
    if marker == Marker::Circle {
        let x = p.0.clamp(r.left, r.right());
        let y = p.1.clamp(r.top, r.bottom());
        return (p.0 - x).hypot(p.1 - y) < radius;
    }
    let poly = marker_polygon(marker, p, radius);
    if !poly.is_empty() {
        return polygon_hits(&poly, r);
    }
    marker_segments(marker, p, radius)
        .iter()
        .any(|[a, b]| stroke_interval(*a, *b, width, r).is_some())
}

fn polygon_bounds(points: &[(f32, f32)]) -> Rect {
    let (mut x, mut y, mut right, mut bottom) = (
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    );
    for &(px, py) in points {
        x = x.min(px);
        y = y.min(py);
        right = right.max(px);
        bottom = bottom.max(py);
    }
    Rect::new(x, y, right - x, bottom - y)
}
fn join_polygon(
    a: (f32, f32),
    b: (f32, f32),
    c: (f32, f32),
    radius: f32,
) -> Option<Vec<(f32, f32)>> {
    let unit = |a: (f32, f32), b: (f32, f32)| {
        let dx = b.0 - a.0;
        let dy = b.1 - a.1;
        let len = dx.hypot(dy);
        (len.is_finite() && len > 0.).then_some((dx / len, dy / len))
    };
    let u = unit(a, b)?;
    let v = unit(b, c)?;
    let turn = u.0 * v.1 - u.1 * v.0;
    if turn.abs() < 1e-6 {
        return None;
    }
    let side = -turn.signum();
    let n = (-u.1 * side, u.0 * side);
    let m = (-v.1 * side, v.0 * side);
    let first = (b.0 + radius * n.0, b.1 + radius * n.1);
    let last = (b.0 + radius * m.0, b.1 + radius * m.1);
    let denom = 1. + u.0 * v.0 + u.1 * v.1;
    let offset = ((n.0 + m.0) / denom, (n.1 + m.1) / denom);
    let tip = if offset.0.hypot(offset.1) <= 4. {
        (b.0 + radius * offset.0, b.1 + radius * offset.1)
    } else {
        b
    }; // Native miter-limit fallback is a bevel.
    Some(vec![first, tip, last])
}
fn polygon_hits(poly: &[(f32, f32)], r: Rect) -> bool {
    polygon_area(&clip_polygon(poly, r)) > 0.
}
fn polygon_area(points: &[(f32, f32)]) -> f64 {
    if points.len() < 3 {
        return 0.;
    }
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(a, b)| f64::from(a.0) * f64::from(b.1) - f64::from(b.0) * f64::from(a.1))
        .sum::<f64>()
        .abs()
        * 0.5
}
fn stroke_interval(a: (f32, f32), b: (f32, f32), width: f32, r: Rect) -> Option<(f64, f64)> {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let len = dx.hypot(dy);
    if !len.is_finite() || len == 0. || width <= 0. {
        return None;
    }
    let nx = -dy / len * width * 0.5;
    let ny = dx / len * width * 0.5;
    let poly = [
        (a.0 + nx, a.1 + ny),
        (a.0 - nx, a.1 - ny),
        (b.0 - nx, b.1 - ny),
        (b.0 + nx, b.1 + ny),
    ];
    let points = clip_polygon(&poly, r);
    if polygon_area(&points) == 0. {
        return None;
    }
    let (mut lo, mut hi) = (1f64, 0f64);
    for p in points {
        let t = (f64::from(p.0 - a.0) * f64::from(dx) + f64::from(p.1 - a.1) * f64::from(dy))
            / f64::from(len).powi(2);
        lo = lo.min(t);
        hi = hi.max(t);
    }
    Some((lo.max(0.), hi.min(1.)))
}
fn clip_polygon(poly: &[(f32, f32)], r: Rect) -> Vec<(f32, f32)> {
    // Sutherland-Hodgman clipping of the painted polygon, not its bounding box.
    let mut points = poly.to_vec();
    for edge in 0..4 {
        let distance = |p: (f32, f32)| match edge {
            0 => p.0 - r.left,
            1 => r.right() - p.0,
            2 => p.1 - r.top,
            _ => r.bottom() - p.1,
        };
        let mut next = Vec::new();
        if let Some(&last) = points.last() {
            let mut a = last;
            for &b in &points {
                let da = distance(a);
                let db = distance(b);
                if (da >= 0.) != (db >= 0.) {
                    let t = da / (da - db);
                    next.push((a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1)));
                }
                if db >= 0. {
                    next.push(b);
                }
                a = b;
            }
        }
        points = next;
    }
    points
}
