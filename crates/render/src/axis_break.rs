//! Ticks and furniture for broken axes. The data-to-position mapping itself
//! lives in [`plotium_figure::Axis::normalize`], so data, ticks and hit testing
//! already agree; this module adds what only a renderer needs: a tick set per
//! visible piece, and the gap with its break marks.
//!
//! Every backend paints the figure background over the gap after the data and
//! spines. Figure backgrounds are opaque, so this is equivalent to clipping the
//! data to the two pieces, and it also opens the spine at the break.
use crate::Rect;
use crate::ticks::{
    AxisTicks, label_tick_values, minor_tick_values, nice_step, ticks as nice_ticks,
};
use plotium_figure::{Axis, AxisFrame, Figure};

/// Half the long and short extents of one break mark, in points.
pub(crate) const MARK_HALF_LONG: f32 = 2.5;
const MARK_HALF_SHORT: f32 = 1.25;

/// Major ticks of a broken axis: each piece gets a share of `target` by its
/// drawn length, and all labels share one multiplier and precision. A tick on
/// the edge next to the gap is left out unless it is its piece's only tick:
/// its label would collide with the one across the narrow gap.
pub(crate) fn ticks(axis: &Axis, pieces: [(f64, f64); 2], target: usize) -> AxisTicks {
    let to_display = |v: f64| axis.display.as_ref().map_or(v, |d| d.to_display(v));
    let to_coordinate = |v: f64| axis.display.as_ref().map_or(v, |d| d.to_coordinate(v));
    let mut values = Vec::new();
    let mut step = f64::INFINITY;
    let mut max_abs: f64 = 0.0;
    for (index, (lo, hi)) in pieces.into_iter().enumerate() {
        let share = (axis.normalize(hi) - axis.normalize(lo)).abs();
        let piece_target = ((target as f64 * share).round() as usize).max(1);
        let (a, b) = (to_display(lo), to_display(hi));
        max_abs = max_abs.max(a.abs()).max(b.abs());
        let mut piece = nice_ticks(a, b, piece_target);
        let inner = to_display(if index == 0 { hi } else { lo });
        let eps = (b - a).abs() * 1e-9;
        if piece.len() > 1 {
            piece.retain(|v| (v - inner).abs() > eps);
        }
        let piece_step = nice_step((b - a).abs(), piece_target);
        if piece_step.is_finite() && piece_step > 0.0 {
            step = step.min(piece_step);
        }
        values.extend(piece);
    }
    let mut result = label_tick_values(values, step, max_abs, axis.thousands_separator);
    for value in &mut result.values {
        *value = to_coordinate(*value);
    }
    result
}

/// Minor ticks of a broken axis, spaced within each piece from that piece's
/// own majors.
pub(crate) fn minor_ticks(
    axis: &Axis,
    pieces: [(f64, f64); 2],
    majors: &AxisTicks,
    count: u8,
) -> Vec<f64> {
    pieces
        .into_iter()
        .flat_map(|(lo, hi)| {
            let mut piece = axis.clone();
            piece.axis_break = None;
            (piece.min, piece.max) = if axis.min <= axis.max {
                (lo, hi)
            } else {
                (hi, lo)
            };
            let eps = (hi - lo).abs() * 1e-9;
            let values: Vec<f64> = majors
                .values
                .iter()
                .copied()
                .filter(|v| *v >= lo - eps && *v <= hi + eps)
                .collect();
            let piece_majors = AxisTicks {
                values,
                labels: Vec::new(),
                ..majors.clone()
            };
            minor_tick_values(&piece, &piece_majors, count)
        })
        .collect()
}

/// What a backend paints over a plot with a broken axis: `erase` rectangles in
/// the figure background, then `marks` as axis-coloured line segments of
/// `mark_width`. All lengths are in output units.
#[derive(Debug, Default)]
pub(crate) struct BreakOverlay {
    pub erase: Vec<Rect>,
    pub marks: Vec<[(f32, f32); 2]>,
    pub mark_width: f32,
}

/// The overlay for `fig` drawn in `plot`, with the y spine at `y_axis_x`.
pub(crate) fn overlay(fig: &Figure, plot: Rect, y_axis_x: f32, scale: f32) -> BreakOverlay {
    let mut out = BreakOverlay {
        mark_width: fig.typography.axis_line_pt * scale,
        ..BreakOverlay::default()
    };
    // Reach past the plot edge far enough to open a spine drawn on it.
    let pad = fig.typography.axis_line_pt * scale;
    let long = MARK_HALF_LONG * scale;
    let short = MARK_HALF_SHORT * scale;
    let marked = fig.axis_frame != AxisFrame::Hidden;
    let boxed = fig.axis_frame == AxisFrame::Box;
    if let Some((a, b)) = fig.x.break_gap() {
        let (x0, x1) = (
            plot.left + a as f32 * plot.width,
            plot.left + b as f32 * plot.width,
        );
        // Open frames have no top spine, so the erase stops at the plot top
        // and leaves a plot stacked directly above untouched.
        let top = if boxed { plot.top - pad } else { plot.top };
        out.erase
            .push(Rect::new(x0, top, x1 - x0, plot.bottom() + pad - top));
        if marked {
            let spines = [Some(plot.bottom()), boxed.then_some(plot.top)];
            for y in spines.into_iter().flatten() {
                for x in [x0, x1] {
                    out.marks
                        .push([(x - short, y + long), (x + short, y - long)]);
                }
            }
        }
    }
    if let Some((a, b)) = fig.y.break_gap() {
        let (y0, y1) = (
            plot.top + (1.0 - b as f32) * plot.height,
            plot.top + (1.0 - a as f32) * plot.height,
        );
        let left = y_axis_x.min(plot.left) - pad;
        let right = if boxed {
            plot.right() + pad
        } else {
            plot.right()
        };
        out.erase.push(Rect::new(left, y0, right - left, y1 - y0));
        if marked {
            let spines = [Some(y_axis_x), boxed.then_some(plot.right())];
            for x in spines.into_iter().flatten() {
                for y in [y0, y1] {
                    out.marks
                        .push([(x - long, y + short), (x + long, y - short)]);
                }
            }
        }
    }
    out
}

/// Write the overlay into an SVG document drawn at unit scale.
pub(crate) fn write_svg(s: &mut String, fig: &Figure, plot: Rect, y_axis_x: f32) {
    use std::fmt::Write;
    let gap = overlay(fig, plot, y_axis_x, 1.0);
    let background = fig.background.to_hex();
    let axis = plotium_figure::Color::AXIS.to_hex();
    // Writing into a String cannot fail.
    for r in &gap.erase {
        let _ = write!(
            s,
            r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{background}"/>"#,
            r.left, r.top, r.width, r.height,
        );
    }
    for &[a, b] in &gap.marks {
        let _ = write!(
            s,
            r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{axis}" stroke-width="{}"/>"#,
            a.0, a.1, b.0, b.1, gap.mark_width,
        );
    }
}

#[cfg(test)]
#[path = "axis_break_tests.rs"]
mod tests;
