//! Placement of the rotated y-axis title around a panel letter.

use crate::{OUTER_PAD, Rect, ticks};
use plotium_figure::{AxisFrame, Figure};

/// Vertical centre of the rotated y-axis title: the middle of the plot, moved
/// down just far enough that its top end clears `clearance` below the frame
/// top. The title may run into the free bottom-left corner but stops above
/// the x-axis title row.
pub(crate) fn y_title_center(
    fig: &Figure,
    plot: Rect,
    outer: Rect,
    clearance: f32,
    scale: f32,
) -> f32 {
    let centre = (plot.top + plot.bottom()) * 0.5;
    if clearance <= 0.0 {
        return centre;
    }
    let ty = fig.typography;
    let half = ticks::estimated_text_width(&fig.y.label, ty.label_pt) * 0.5 * scale;
    let clear = outer.top + clearance + half;
    let lowest = outer.bottom() - (OUTER_PAD + ty.label_pt * 1.2) * scale - half;
    centre.max(clear.min(lowest))
}

/// Room a panel letter needs around a plot's rotated y-axis title, measured
/// from the frame's top-left corner. Zero when nothing occupies the corner.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct YTitleClearance {
    /// Distance below the frame top that the title must start.
    pub below: f32,
    /// Distance from the frame left to the letter's right edge.
    pub beside: f32,
}

impl YTitleClearance {
    pub fn scaled(self, scale: f32) -> Self {
        Self {
            below: self.below * scale,
            beside: self.beside * scale,
        }
    }
}

/// The part of a plot frame its axes lay out in, and the clearance left for
/// [`y_title_center`] there. A title longer than the room between the letter
/// and the x-title row cannot move down past the letter, so the whole plot
/// moves right of it instead; `OUTER_PAD` then separates letter and title.
/// The renderers and the editor's plot-box geometry both go through this so
/// the editor hit-tests the plot where it is drawn. Size-only geometry such as
/// guide diagnostics and visual spacing does not see the shift.
pub fn y_title_area(
    fig: &Figure,
    outer: Rect,
    clearance: YTitleClearance,
    scale: f32,
) -> (Rect, f32) {
    let shows_title = fig.y.show_label && fig.axis_frame != AxisFrame::Hidden;
    if !shows_title || clearance.below <= 0.0 || clearance.beside <= 0.0 {
        return (outer, clearance.below);
    }
    let ty = fig.typography;
    let length = ticks::estimated_text_width(&fig.y.label, ty.label_pt) * scale;
    let room = outer.height - clearance.below - (OUTER_PAD + ty.label_pt * 1.2) * scale;
    if length <= room {
        return (outer, clearance.below);
    }
    let inset = clearance.beside.min(outer.width);
    let area = Rect::new(
        outer.left + inset,
        outer.top,
        outer.width - inset,
        outer.height,
    );
    (area, 0.0)
}
