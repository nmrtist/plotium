//! Output-space curve primitives shared by screen and vector backends.
use crate::Rect;
use plotium_figure::{Color, CurveAppearance, LineStyle, Marker, Series, SeriesKind};

/// How one marker is painted: open marks (plus, cross) stroke in the series
/// colour; closed marks take the appearance's fill and outline.
#[derive(Clone, Copy, Debug)]
pub(crate) struct MarkerPaint {
    pub color: Color,
    pub width: f32,
    pub fill: Option<Color>,
    pub edge: Option<(Color, f32)>,
}

impl MarkerPaint {
    pub(crate) fn new(appearance: CurveAppearance, color: Color, width: f32) -> Self {
        Self {
            color,
            width,
            fill: appearance.marker_fill(color),
            edge: appearance.marker_edge(color),
        }
    }

    /// Scale the output-space widths for a zoomed screen render.
    #[cfg(feature = "screen")]
    pub(crate) fn scaled(self, scale: f32) -> Self {
        Self {
            width: self.width * scale,
            edge: self.edge.map(|(color, width)| (color, width * scale)),
            ..self
        }
    }
}

pub(crate) fn dash_pattern(style: LineStyle, width: f32) -> Vec<f32> {
    let w = width.max(0.5);
    match style {
        LineStyle::Solid => vec![],
        LineStyle::Dashed => vec![6. * w, 3. * w],
        LineStyle::Dotted => vec![w, 2. * w],
        LineStyle::DashDot => vec![6. * w, 2. * w, w, 2. * w],
    }
}

/// Clip before enumerating dashes. An offscreen point may be arbitrarily far
/// away; it must not allocate millions of invisible dash primitives.
#[cfg(any(feature = "screen", all(windows, feature = "emf"), test))]
pub(crate) fn dashed_segments(
    points: &[(f32, f32)],
    style: LineStyle,
    width: f32,
    clip: Rect,
) -> Vec<[(f32, f32); 2]> {
    let pattern = dash_pattern(style, width);
    let period: f64 = pattern.iter().map(|v| f64::from(*v)).sum();
    let mut phase = 0.0_f64;
    let mut out = vec![];
    for pair in points.windows(2) {
        let a = pair[0];
        let b = pair[1];
        let dx = f64::from(b.0) - f64::from(a.0);
        let dy = f64::from(b.1) - f64::from(a.1);
        let length = dx.hypot(dy);
        if !length.is_finite() || length == 0. {
            continue;
        }
        if let Some((lo, hi)) = clip_segment(a, b, clip) {
            let start = (f64::from(a.0) + dx * lo, f64::from(a.1) + dy * lo);
            let finish = (f64::from(a.0) + dx * hi, f64::from(a.1) + dy * hi);
            let vx = finish.0 - start.0;
            let vy = finish.1 - start.1;
            let visible = vx.hypot(vy);
            if visible == 0. {
                phase = (phase + length).rem_euclid(period);
                continue;
            }
            let mut pos = 0.;
            let mut index = 0;
            let mut local = (phase + lo * length).rem_euclid(period);
            while local >= f64::from(pattern[index]) {
                local -= f64::from(pattern[index]);
                index = (index + 1) % pattern.len();
            }
            while pos < visible {
                let next = (pos + f64::from(pattern[index]) - local).min(visible);
                if index % 2 == 0 {
                    out.push([
                        (
                            (start.0 + vx * pos / visible) as f32,
                            (start.1 + vy * pos / visible) as f32,
                        ),
                        (
                            (start.0 + vx * next / visible) as f32,
                            (start.1 + vy * next / visible) as f32,
                        ),
                    ]);
                }
                pos = next;
                local = 0.;
                index = (index + 1) % pattern.len();
            }
        }
        phase = (phase + length).rem_euclid(period);
    }
    out
}
pub(crate) fn clip_segment(a: (f32, f32), b: (f32, f32), r: Rect) -> Option<(f64, f64)> {
    let dx = f64::from(b.0) - f64::from(a.0);
    let dy = f64::from(b.1) - f64::from(a.1);
    let mut lo = 0.0_f64;
    let mut hi = 1.0_f64;
    for (p, q) in [
        (-dx, f64::from(a.0 - r.left)),
        (dx, f64::from(r.right() - a.0)),
        (-dy, f64::from(a.1 - r.top)),
        (dy, f64::from(r.bottom() - a.1)),
    ] {
        if p == 0. {
            if q < 0. {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0. {
                lo = lo.max(t);
            } else {
                hi = hi.min(t);
            }
            if lo > hi {
                return None;
            }
        }
    }
    Some((lo, hi))
}
pub(crate) fn marker_polygon(marker: Marker, (x, y): (f32, f32), r: f32) -> Vec<(f32, f32)> {
    match marker {
        Marker::Square => vec![
            (x - r, y - r),
            (x + r, y - r),
            (x + r, y + r),
            (x - r, y + r),
        ],
        Marker::Diamond => vec![(x, y - r), (x + r, y), (x, y + r), (x - r, y)],
        Marker::TriangleUp => vec![(x, y - r), (x + r, y + r), (x - r, y + r)],
        Marker::TriangleDown => vec![(x - r, y - r), (x + r, y - r), (x, y + r)],
        _ => vec![],
    }
}
pub(crate) fn marker_segments(marker: Marker, (x, y): (f32, f32), r: f32) -> Vec<[(f32, f32); 2]> {
    match marker {
        Marker::Plus => vec![[(x - r, y), (x + r, y)], [(x, y - r), (x, y + r)]],
        Marker::Cross => vec![
            [(x - r, y - r), (x + r, y + r)],
            [(x - r, y + r), (x + r, y - r)],
        ],
        _ => vec![],
    }
}
pub(crate) fn legend_series(
    color: plotium_figure::Color,
    kind: SeriesKind,
    appearance: CurveAppearance,
    width: f32,
    x: f32,
    y: f32,
    sw: f32,
) -> Series {
    let mut s = Series::line(
        "",
        if kind == SeriesKind::Points {
            vec![[f64::from(x + sw * 0.5), f64::from(y)]]
        } else {
            vec![
                [f64::from(x), f64::from(y)],
                [f64::from(x + sw), f64::from(y)],
            ]
        },
    );
    s.color = color;
    s.kind = kind;
    s.appearance = appearance;
    s.width = width;
    s
}
