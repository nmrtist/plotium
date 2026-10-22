use super::*;
use crate::y2_axis::RightAxis;

/// The right y axis, mirroring [`crate::svg`]'s right-axis output.
pub(super) fn write(dc: &mut Dc, fig: &Figure, geometry: &RightAxis<'_>, plot: Rect) {
    let ty = fig.typography;
    let x = geometry.x;
    if geometry.spine {
        dc.line(
            (x, plot.top),
            (x, plot.bottom()),
            Color::AXIS,
            ty.axis_line_pt,
        );
    }
    for &y in &geometry.minor {
        let end = (x + geometry.minor_reach, y);
        dc.line((x, y), end, Color::AXIS, ty.axis_line_pt);
    }
    for &(_, y, label) in &geometry.ticks {
        let end = (x + geometry.tick_reach, y);
        dc.line((x, y), end, Color::AXIS, ty.axis_line_pt);
        if let Some(label) = label {
            dc.text(
                label,
                (geometry.label_x, y),
                TextStyle::new(ty.tick_pt, Color::AXIS, TA_LEFT).middle(),
            );
        }
    }
    if let Some((text, mx, inside)) = &geometry.multiplier {
        let y = if *inside {
            plot.top + TICK_LABEL_PAD + ty.tick_pt
        } else {
            plot.top - TICK_LABEL_PAD
        };
        dc.text(
            text,
            (*mx, y),
            TextStyle::new(ty.tick_pt, Color::AXIS, TA_RIGHT),
        );
    }
    if let Some((text, lane, centre)) = geometry.title {
        dc.text(
            text,
            (
                lane + ty.label_pt * crate::rich_text::TOP_TO_BASELINE,
                centre,
            ),
            TextStyle::new(ty.label_pt, Color::AXIS, TA_CENTER).rotated(),
        );
    }
}
