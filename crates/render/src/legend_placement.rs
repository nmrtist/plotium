//! Automatic legend placement: choose the inside anchor that covers the least
//! plotted data, so the default legend sits in empty plot space.
use crate::Rect;
use plotium_figure::{Figure, SeriesKind};

/// Gap between an automatic legend and the plot edges, in points at scale 1.
pub(crate) const LEGEND_INSET: f32 = 8.0;

/// Candidate anchors as fractions of the free travel inside the plot. The
/// order is the tie-break preference: upper right first, then the remaining
/// corners, then the edge midpoints.
pub(crate) const ANCHORS: [[f32; 2]; 8] = [
    [1.0, 0.0],
    [0.0, 0.0],
    [0.0, 1.0],
    [1.0, 1.0],
    [1.0, 0.5],
    [0.0, 0.5],
    [0.5, 0.0],
    [0.5, 1.0],
];

/// Top-left corner of an automatically placed legend of `width` × `height`.
/// Every anchor keeps the same `inset` from the plot edges; ties keep the
/// earlier anchor in [`ANCHORS`], so an empty plot gets the upper right.
pub(crate) fn auto_legend_origin(
    fig: &Figure,
    plot: Rect,
    width: f32,
    height: f32,
    inset: f32,
) -> (f32, f32) {
    let candidates = anchor_boxes(plot, width, height, inset);
    // Data that merely touches the text is still crowded, so test a slightly
    // grown box. Equally clear anchors then prefer more breathing room, judged
    // by a wider box; both margins scale with `inset` so zoom cannot change
    // the choice.
    let zones = [
        candidates.map(|r| grow(r, inset * 0.25)),
        candidates.map(|r| grow(r, inset * 2.0)),
    ]
    .concat();
    let scores = coverage(fig, plot, &zones);
    let best = (0..ANCHORS.len())
        .min_by_key(|&index| (scores[index], scores[index + ANCHORS.len()], index))
        .unwrap_or(0);
    (candidates[best].left, candidates[best].top)
}

/// Legend boxes at every anchor in [`ANCHORS`] order.
pub(crate) fn anchor_boxes(plot: Rect, width: f32, height: f32, inset: f32) -> [Rect; 8] {
    // An oversize legend keeps the upper-right fallback on that axis rather
    // than spilling past the plot's left or top edge.
    let travel_x = plot.width - width - 2.0 * inset;
    let travel_y = (plot.height - height - 2.0 * inset).max(0.0);
    let left = |fx: f32| {
        if travel_x >= 0.0 {
            plot.left + inset + fx * travel_x
        } else {
            (plot.right() - width - inset).max(plot.left + inset * 0.25)
        }
    };
    ANCHORS.map(|[fx, fy]| Rect::new(left(fx), plot.top + inset + fy * travel_y, width, height))
}

pub(crate) fn grow(r: Rect, by: f32) -> Rect {
    Rect::new(
        r.left - by,
        r.top - by,
        r.width + 2.0 * by,
        r.height + 2.0 * by,
    )
}

/// How many projected samples and drawn segments fall inside each zone.
pub(crate) fn coverage(fig: &Figure, plot: Rect, zones: &[Rect]) -> Vec<usize> {
    let mut scores = vec![0usize; zones.len()];
    for_each_segment(fig, plot, |a, b| {
        for (score, zone) in scores.iter_mut().zip(zones) {
            if segment_hits(a, b, *zone) {
                *score += 1;
            }
        }
    });
    scores
}

/// Visit every projected sample (as a zero-length segment) and drawn segment.
pub(crate) fn for_each_segment(
    fig: &Figure,
    plot: Rect,
    mut add_segment: impl FnMut((f32, f32), (f32, f32)),
) {
    let project = |p: [f64; 2]| {
        let tx = fig.x.normalize(p[0]) as f32;
        let ty = fig.y.normalize(p[1]) as f32;
        (
            plot.left + tx * plot.width,
            plot.top + (1.0 - ty) * plot.height,
        )
    };
    let finite = |p: [f64; 2]| p[0].is_finite() && p[1].is_finite();
    // Dense lines are pooled into one vertical span per output unit, so the
    // cost per call stays bounded by the plot width for long spectra.
    let columns = plot.width.max(1.0).ceil() as usize;
    for series in &fig.series {
        if matches!(series.kind, SeriesKind::Line) && series.points.len() > 2 * columns {
            let mut spans: Vec<Option<(f32, f32)>> = vec![None; columns];
            for &point in series.points.iter().filter(|p| finite(**p)) {
                let (x, y) = project(point);
                let column = (x - plot.left).floor();
                if !(0.0..columns as f32).contains(&column) {
                    continue;
                }
                let span = &mut spans[column as usize];
                *span = Some(span.map_or((y, y), |(lo, hi)| (lo.min(y), hi.max(y))));
            }
            for (column, span) in spans.iter().enumerate() {
                if let Some((lo, hi)) = span {
                    let x = plot.left + column as f32 + 0.5;
                    add_segment((x, *lo), (x, *hi));
                }
            }
            continue;
        }
        let mut previous: Option<(f32, f32)> = None;
        for &point in &series.points {
            if !finite(point) {
                previous = None;
                continue;
            }
            let p = project(point);
            match series.kind {
                SeriesKind::Line => add_segment(previous.unwrap_or(p), p),
                SeriesKind::Points => add_segment(p, p),
                SeriesKind::Stick => add_segment(project([point[0], 0.0]), p),
            }
            previous = Some(p);
        }
    }
    for bar in &fig.error_bars {
        let low = [bar.center[0], bar.center[1] - bar.negative];
        let high = [bar.center[0], bar.center[1] + bar.positive];
        if finite(low) && finite(high) {
            add_segment(project(low), project(high));
        }
    }
    for annotation in fig.text_annotations().filter(|a| finite(a.at)) {
        let p = project(annotation.at);
        add_segment(p, p);
    }
    for arrow in fig
        .author_arrows
        .iter()
        .filter(|a| finite(a.from) && finite(a.to))
    {
        add_segment(project(arrow.from), project(arrow.to));
    }
}

/// Whether segment `a`–`b` (a point when they coincide) touches `zone`.
fn segment_hits(a: (f32, f32), b: (f32, f32), zone: Rect) -> bool {
    clip(a, b, zone).is_some()
}

/// The parameter range of segment `a`–`b` inside `zone`, if any.
pub(crate) fn clip(a: (f32, f32), b: (f32, f32), zone: Rect) -> Option<(f32, f32)> {
    // Liang–Barsky clip of the segment's parameter range against the box.
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let mut t0 = 0.0_f32;
    let mut t1 = 1.0_f32;
    for (p, q) in [
        (-dx, a.0 - zone.left),
        (dx, zone.right() - a.0),
        (-dy, a.1 - zone.top),
        (dy, zone.bottom() - a.1),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
            if t0 > t1 {
                return None;
            }
        }
    }
    Some((t0, t1))
}

#[cfg(test)]
#[path = "legend_placement_tests.rs"]
mod tests;
