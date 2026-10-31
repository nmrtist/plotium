//! Hatch geometry shared by every backend, so exports carry real vector lines
//! instead of format-specific pattern fills.
use crate::Rect;
use plotium_figure::{HATCH_LINE_PT, HATCH_SPACING_PT, Hatch, HatchPattern};
use std::fmt::Write as _;

/// Hatch lines across a convex `outline`, `scale` output units per point.
///
/// Lines are anchored to the outline so they travel with it on pan, and only
/// those crossing `clip` are built: a zoomed bar can be far larger than the
/// plot.
pub(crate) fn hatch_lines(
    outline: &[(f32, f32)],
    pattern: HatchPattern,
    clip: Rect,
    scale: f32,
) -> Vec<[(f32, f32); 2]> {
    let spacing = HATCH_SPACING_PT * scale;
    let mut lines = Vec::new();
    // Normals in y-down page space: a `/` line runs along (1, -1).
    if matches!(pattern, HatchPattern::Forward | HatchPattern::Cross) {
        family((1.0, 1.0), outline, clip, spacing, &mut lines);
    }
    if matches!(pattern, HatchPattern::Backward | HatchPattern::Cross) {
        family((1.0, -1.0), outline, clip, spacing, &mut lines);
    }
    lines
}

/// Append the hatch of a convex `outline` as one SVG path.
pub(crate) fn write_svg(s: &mut String, outline: &[(f32, f32)], hatch: Hatch, clip: Rect) {
    let lines = hatch_lines(outline, hatch.pattern, clip, 1.0);
    if lines.is_empty() {
        return;
    }
    let mut d = String::new();
    for [(x0, y0), (x1, y1)] in lines {
        let _ = write!(d, "M{x0:.2} {y0:.2}L{x1:.2} {y1:.2}");
    }
    let _ = write!(
        s,
        r#"<path d="{d}" fill="none" stroke="{col}" stroke-width="{HATCH_LINE_PT}"/>"#,
        col = hatch.color.to_hex(),
    );
}

/// Paint the hatch of a convex `outline` given in screen pixels.
#[cfg(feature = "screen")]
pub(crate) fn paint(
    painter: &egui::Painter,
    outline: &[(f32, f32)],
    hatch: Hatch,
    clip: Rect,
    scale: f32,
) {
    let color = egui::Color32::from_rgb(hatch.color.r, hatch.color.g, hatch.color.b);
    let stroke = egui::Stroke::new(HATCH_LINE_PT * scale, color);
    for [(x0, y0), (x1, y1)] in hatch_lines(outline, hatch.pattern, clip, scale) {
        painter.line_segment([egui::pos2(x0, y0), egui::pos2(x1, y1)], stroke);
    }
}

fn family(
    (nx, ny): (f32, f32),
    outline: &[(f32, f32)],
    clip: Rect,
    spacing: f32,
    lines: &mut Vec<[(f32, f32); 2]>,
) {
    let normal = std::f32::consts::FRAC_1_SQRT_2;
    let along = |(x, y): (f32, f32)| (x * nx + y * ny) * normal;
    let range = |points: &[(f32, f32)]| {
        points.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &p| {
            (lo.min(along(p)), hi.max(along(p)))
        })
    };
    let (first, last) = range(outline);
    let (clip_first, clip_last) = range(&clip.corners());
    if !first.is_finite() || !last.is_finite() || spacing <= 0.0 {
        return;
    }
    let (lo, hi) = (first.max(clip_first), last.min(clip_last));
    let mut step = ((lo - first) / spacing).floor() + 1.0;
    while first + step * spacing < hi {
        if let Some(line) = chord(outline, &along, first + step * spacing) {
            lines.push(line);
        }
        step += 1.0;
    }
}

/// Where the line at `offset` enters and leaves a convex outline.
fn chord(
    outline: &[(f32, f32)],
    along: &impl Fn((f32, f32)) -> f32,
    offset: f32,
) -> Option<[(f32, f32); 2]> {
    let mut hits = outline
        .iter()
        .zip(outline.iter().cycle().skip(1))
        .filter_map(|(&a, &b)| {
            let (da, db) = (along(a) - offset, along(b) - offset);
            ((da < 0.0) != (db < 0.0)).then(|| {
                let t = da / (da - db);
                (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
            })
        });
    Some([hits.next()?, hits.next()?])
}

#[cfg(test)]
#[path = "hatch_tests.rs"]
mod tests;
