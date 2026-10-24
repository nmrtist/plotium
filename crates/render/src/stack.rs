//! Geometry of a plot that is one member of a shared-x stack.
//!
//! A stack is a runtime arrangement derived from its panel on every render; it
//! never edits the figure. Each renderer, the editor's hit testing and the
//! diagnostics read the same [`StackPlacement`], so the canvas, vector exports
//! and agent observations agree on where the plot area is.
use crate::ticks::{AxisLayout, adaptive_x_ticks_for_plot, margins_for_ticks};
use crate::{Margins, Projector, Rect, axis_layout};
use plotium_figure::{AxisFrame, Figure};

/// Where a plot sits inside a shared-x stack. All lengths are unscaled points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StackPlacement {
    /// Left margin shared by every member so the plot areas line up.
    pub left: f32,
    /// Right margin shared by every member.
    pub right: f32,
    /// The widest y tick clearance among the members, from
    /// [`AxisLayout::y_tick_clearance`]. Every member's y title sits this far
    /// plus the title gap left of the shared plot edge, so the titles align
    /// beside the widest tick labels.
    pub y_tick_clearance: f32,
    /// The widest right-axis tick clearance among the members, from
    /// [`AxisLayout::y2_tick_clearance`], so right titles align the same way.
    pub y2_tick_clearance: f32,
    /// The widest right-axis band (ticks, labels and title) among the members,
    /// from [`AxisLayout::right_axis_width`]. Outside-right legends stand this
    /// far off every member's plot so they line up down the column.
    pub right_axis_width: f32,
    /// Another member sits directly above: no top margin and no figure title.
    pub joined_above: bool,
    /// Another member sits directly below: no bottom margin, no x tick labels
    /// or x title, and x ticks point into the plot so they do not cross into
    /// the member below.
    pub joined_below: bool,
}

/// Axis layout for a plot that may sit in a stack. Without a placement this is
/// exactly [`axis_layout`].
///
/// The x ticks are chosen for the forced plot width so every member of one
/// stack, which shares that width and the x window, draws the same tick set.
pub fn axis_layout_placed(
    fig: &Figure,
    outer_width: f32,
    outer_height: f32,
    placement: Option<&StackPlacement>,
) -> AxisLayout {
    let natural = axis_layout(fig, outer_width, outer_height);
    let Some(placement) = placement.filter(|_| fig.axis_frame != AxisFrame::Hidden) else {
        return natural;
    };
    let outer = Rect::new(0.0, 0.0, outer_width.max(0.0), outer_height.max(0.0));
    let first = placed_margins(natural.margins, placement);
    let plot = Projector::new(fig, outer, &first).plot;
    let x_ticks = adaptive_x_ticks_for_plot(fig, plot.width);
    let margins = placed_margins(
        margins_for_ticks(
            fig,
            &x_ticks,
            &natural.y_ticks,
            &natural.y2_ticks,
            [outer_width, outer_height],
        ),
        placement,
    );
    AxisLayout {
        margins,
        x_ticks,
        y_ticks: natural.y_ticks,
        y2_ticks: natural.y2_ticks,
    }
}

fn placed_margins(mut margins: Margins, placement: &StackPlacement) -> Margins {
    margins.left = placement.left;
    margins.right = placement.right;
    if placement.joined_above {
        margins.top = 0.0;
    }
    if placement.joined_below {
        margins.bottom = 0.0;
    }
    margins
}

/// Which x-axis furniture a renderer draws for a possibly stacked plot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XFurniture {
    pub show_tick_labels: bool,
    pub show_label: bool,
    pub show_title: bool,
    /// Signed major x tick extent away from the plot area.
    pub tick_reach: f32,
    /// Signed minor x tick extent away from the plot area.
    pub minor_tick_reach: f32,
    /// Distance below the outer top edge where the background starts. A lower
    /// member must not paint over the half of the shared boundary line that
    /// belongs to the member above it.
    pub background_inset: f32,
    /// A joined top edge has no margin for the y "×10ⁿ" multiplier, so it is
    /// drawn just inside the plot's top-left corner instead of above it.
    pub y_multiplier_inside: bool,
    /// Y tick labels centred closer than this (unscaled pt) to the plot's top
    /// edge are skipped: at a joined boundary they would sit on the bottom
    /// label of the member above.
    pub y_label_top_clearance: f32,
}

impl XFurniture {
    pub fn of(fig: &Figure, placement: Option<&StackPlacement>) -> Self {
        let ty = fig.typography;
        let stacked = placement.filter(|_| fig.axis_frame != AxisFrame::Hidden);
        let joined_below = stacked.is_some_and(|p| p.joined_below);
        let joined_above = stacked.is_some_and(|p| p.joined_above);
        let inward = -ty.tick_length_pt;
        Self {
            show_tick_labels: fig.x.show_tick_labels && !joined_below,
            show_label: fig.x.show_label && !joined_below,
            show_title: !joined_above,
            tick_reach: if joined_below {
                inward
            } else {
                ty.tick_reach()
            },
            minor_tick_reach: if joined_below {
                inward * 0.5
            } else {
                ty.minor_tick_reach()
            },
            background_inset: if joined_above {
                ty.axis_line_pt * 0.5
            } else {
                0.0
            },
            y_multiplier_inside: joined_above,
            y_label_top_clearance: if joined_above { ty.tick_pt * 0.5 } else { 0.0 },
        }
    }

    /// Whether a y tick label centred `from_top` unscaled pt below the plot's
    /// top edge is drawn.
    pub fn shows_y_label_at(&self, from_top: f32) -> bool {
        from_top >= self.y_label_top_clearance
    }
}

#[cfg(test)]
#[path = "stack_tests.rs"]
mod tests;
