//! Plot margins: the room the axis furniture needs around the plot area.

use super::{AxisTicks, estimated_text_width, widest_tick_label};
use crate::{AXIS_LABEL_GAP, Margins, TICK_LABEL_PAD};
use plotium_figure::{AxisFrame, Figure};

pub(crate) fn margins_for_ticks(
    fig: &Figure,
    x_ticks: &AxisTicks,
    y_ticks: &AxisTicks,
    y2_ticks: &AxisTicks,
    outer: [f32; 2],
) -> Margins {
    let widths = TickLabelWidths {
        x: widest_tick_label(x_ticks, fig.typography.tick_pt),
        y: widest_tick_label(y_ticks, fig.typography.tick_pt),
        y2: widest_tick_label(y2_ticks, fig.typography.tick_pt),
    };
    margins_for_ticks_with_widths(
        fig,
        x_ticks,
        [y_ticks, y2_ticks],
        widths,
        outer,
        Overhang::Exact,
    )
}

/// Room between the y axis line and the left edge of its widest tick label,
/// for tick labels `widest_label` wide.
pub(crate) fn y_tick_clearance(fig: &Figure, y_ticks: &AxisTicks, widest_label: f32) -> f32 {
    let ty = fig.typography;
    if y_ticks.values.is_empty() {
        0.0
    } else if fig.y.show_tick_labels {
        widest_label + ty.tick_outset() + TICK_LABEL_PAD
    } else {
        ty.tick_outset()
    }
}

/// Whether tick labels near a plot edge are measured where they are drawn, or
/// as if the widest label sat on the edge. The first layout pass, whose ticks
/// are not final, takes the worst case so the final margins cannot grow past
/// it.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Overhang {
    Exact,
    WorstCase,
}

/// How far tick labels reach past one plot edge when `clearance`, this
/// overhang itself or more, is reserved beyond it. Each label is centred at
/// fraction `t` of the plot extent away from the edge and is `half` its extent
/// across the edge.
///
/// A label reaches `half - t * plot` past the edge, and the plot shrinks as
/// the clearance grows. Solving `c = half - t * (available - c)` for each
/// label gives the least clearance that holds every label, so labels well
/// inside the plot, such as the first category slot, need none.
fn edge_overhang(labels: impl Iterator<Item = (f32, f32)>, available: f32) -> f32 {
    let mut widest_half = 0.0f32;
    let overhang = labels
        .filter_map(|(t, half)| {
            widest_half = widest_half.max(half);
            (t.is_finite() && t < 1.0).then(|| (half - t.max(0.0) * available) / (1.0 - t.max(0.0)))
        })
        .fold(0.0f32, f32::max);
    // A degenerate plot extent must not reserve more than a label edge-on.
    overhang.min(widest_half)
}

/// Fractions of the plot width and half widths of the drawn x tick labels.
fn x_label_spans<'a>(
    fig: &'a Figure,
    x_ticks: &'a AxisTicks,
) -> impl Iterator<Item = (f32, f32)> + 'a {
    let tick_pt = fig.typography.tick_pt;
    x_ticks
        .values
        .iter()
        .zip(&x_ticks.labels)
        .filter(move |_| fig.x.show_tick_labels)
        .map(move |(&value, label)| {
            (
                fig.x.normalize(value) as f32,
                estimated_text_width(label, tick_pt) * 0.5,
            )
        })
}

/// Fractions of the plot height, from the bottom, and half heights of the
/// drawn tick labels of both y axes.
fn y_label_spans<'a>(
    fig: &'a Figure,
    y_ticks: &'a AxisTicks,
    y2_ticks: &'a AxisTicks,
) -> impl Iterator<Item = (f32, f32)> + 'a {
    let half = fig.typography.tick_pt * 0.5;
    let left = fig
        .y
        .show_tick_labels
        .then_some((&fig.y, y_ticks))
        .into_iter();
    let right = fig
        .y2
        .as_ref()
        .filter(|axis| axis.show_tick_labels)
        .map(|axis| (axis, y2_ticks))
        .into_iter();
    left.chain(right).flat_map(move |(axis, ticks)| {
        ticks
            .values
            .iter()
            .map(move |&value| (axis.normalize(value) as f32, half))
    })
}

#[derive(Clone, Copy)]
pub(super) struct TickLabelWidths {
    pub x: f32,
    pub y: f32,
    pub y2: f32,
}

/// Margins that hold the axis furniture and nothing more: the outermost
/// text, tick or axis stroke reaches the frame edge, so a frame on the canvas
/// edge leaves no blank border there.
pub(super) fn margins_for_ticks_with_widths(
    fig: &Figure,
    x_ticks: &AxisTicks,
    [y_ticks, y2_ticks]: [&AxisTicks; 2],
    widths: TickLabelWidths,
    [outer_width, outer_height]: [f32; 2],
    overhang: Overhang,
) -> Margins {
    let ty = fig.typography;
    let (guide_right, guide_bottom) = guide_extra_margins(fig);
    let title_clearance = if fig.title.trim().is_empty() {
        0.0
    } else {
        ty.title_pt + AXIS_LABEL_GAP
    };
    if fig.axis_frame == AxisFrame::Hidden {
        return Margins {
            left: 0.0,
            right: guide_right,
            top: title_clearance,
            bottom: guide_bottom,
        };
    }

    let y_tick_clearance = y_tick_clearance(fig, y_ticks, widths.y);
    let x_tick_clearance = if x_ticks.values.is_empty() {
        0.0
    } else if fig.x.show_tick_labels {
        ty.tick_pt + TICK_LABEL_PAD + ty.tick_outset()
    } else {
        ty.tick_outset()
    };
    let y_title_clearance = if fig.y.show_label {
        ty.label_pt + AXIS_LABEL_GAP
    } else {
        0.0
    };
    let y_multiplier = if fig.y.show_tick_labels {
        y_ticks.multiplier_clearance(ty.tick_pt)
    } else {
        0.0
    }
    .max(crate::y2_axis::multiplier_clearance(fig, y2_ticks));
    let x_multiplier = if fig.x.show_tick_labels {
        x_ticks.multiplier_clearance(ty.tick_pt)
    } else {
        0.0
    };
    let x_title_clearance = if fig.x.show_label {
        ty.label_pt + AXIS_LABEL_GAP
    } else {
        0.0
    };

    // Half the axis stroke lies outside the plot rectangle on every side, and
    // break marks cross their spines further.
    let stroke = ty.axis_line_pt * 0.5;
    let mark = crate::axis_break::MARK_HALF_LONG + stroke;
    let boxed = fig.axis_frame == AxisFrame::Box;
    let x_break = fig.x.break_gap().is_some();
    let y_break = fig.y.break_gap().is_some();
    let edge_floor = |broken: bool| if broken { mark } else { stroke };

    let x_worst = if fig.x.show_tick_labels && !x_ticks.values.is_empty() {
        widths.x * 0.5
    } else {
        0.0
    };
    let y_worst = if y_label_spans(fig, y_ticks, y2_ticks).next().is_some() {
        ty.tick_pt * 0.5
    } else {
        0.0
    };

    let top_furniture = (title_clearance + y_multiplier).max(edge_floor(x_break && boxed));
    let bottom_furniture = (x_multiplier + x_title_clearance + x_tick_clearance + guide_bottom)
        .max(edge_floor(x_break));
    // The end y tick labels are centred on the plot edges and stick out by
    // half their height where nothing else is reserved beyond the edge.
    let top = top_furniture.max(match overhang {
        Overhang::Exact => edge_overhang(
            y_label_spans(fig, y_ticks, y2_ticks).map(|(t, half)| (1.0 - t, half)),
            outer_height - bottom_furniture,
        ),
        Overhang::WorstCase => y_worst,
    });
    let bottom = bottom_furniture.max(match overhang {
        Overhang::Exact => edge_overhang(y_label_spans(fig, y_ticks, y2_ticks), outer_height - top),
        Overhang::WorstCase => y_worst,
    });

    // A right axis needs its ticks, labels and title beside the plot. An
    // outside-right legend stands off that axis as measured at the drawn plot
    // height, which can pick wider right tick labels than this layout's.
    let mut right_axis = crate::y2_axis::reserved_width(fig, y2_ticks, widths.y2);
    if outside_right_legend(fig) {
        right_axis = right_axis.max(crate::y2_axis::guide_offset(
            fig,
            outer_height - top - bottom,
        ));
    }
    let right_furniture = (right_axis + guide_right).max(edge_floor(y_break && boxed));
    let right_overhang = |left: f32| match overhang {
        Overhang::Exact => edge_overhang(
            x_label_spans(fig, x_ticks).map(|(t, half)| (1.0 - t, half)),
            outer_width - left,
        ),
        Overhang::WorstCase => x_worst,
    };
    // The two x label overhangs share one width. The right one is sized for
    // the narrowest left margin, then the left one for that right margin, then
    // the right one again for the final left margin.
    let first_right = right_overhang(y_title_clearance + y_tick_clearance).max(right_furniture);
    // Keep a left-end x label out of the rotated y-title lane even when the
    // y ticks themselves have been dropped on a short panel.
    let x_endpoint_clearance = match overhang {
        Overhang::Exact => edge_overhang(
            x_label_spans(fig, x_ticks),
            outer_width - y_title_clearance - first_right,
        ),
        Overhang::WorstCase => x_worst,
    };
    let left =
        (y_title_clearance + y_tick_clearance.max(x_endpoint_clearance)).max(edge_floor(y_break));
    let right = right_overhang(left).max(right_furniture);

    Margins {
        left,
        right,
        top,
        bottom,
    }
}

pub(crate) fn guide_extra_margins(fig: &Figure) -> (f32, f32) {
    let mut right: f32 = 0.0;
    let mut bottom: f32 = 0.0;
    if let Some(heatmap) = fig
        .heatmap
        .as_ref()
        .filter(|_| crate::renders_color_scale(fig))
    {
        // Mirrors `color_scale_rect`: the bar stands 10 pt off the plot, is
        // 8 pt thick and has its end values 3 pt beside it (2 pt below a
        // horizontal bar).
        let font = fig.typography.legend_pt;
        match fig.guide_placement {
            plotium_figure::GuidePlacement::OutsideBottom => bottom = 20.0 + font,
            plotium_figure::GuidePlacement::Auto | plotium_figure::GuidePlacement::OutsideRight => {
                let label = heatmap
                    .value_range
                    .iter()
                    .map(|&value| {
                        estimated_text_width(&crate::svg::color_scale::format_value(value), font)
                    })
                    .fold(0.0, f32::max);
                right = 21.0 + label;
            }
            plotium_figure::GuidePlacement::Inside => {}
        }
    }
    if crate::renders_legend(fig) && fig.legend_position.is_none() {
        let entries = crate::legend_entries(fig);
        let layout = crate::legend_layout(fig, &entries);
        match fig.guide_placement {
            plotium_figure::GuidePlacement::OutsideRight => right = right.max(layout.width + 8.0),
            plotium_figure::GuidePlacement::OutsideBottom => bottom += layout.height + 8.0,
            plotium_figure::GuidePlacement::Auto | plotium_figure::GuidePlacement::Inside => {}
        }
    }
    (right, bottom)
}

/// Whether the legend stands outside the plot on its right.
fn outside_right_legend(fig: &Figure) -> bool {
    crate::renders_legend(fig)
        && fig.legend_position.is_none()
        && fig.guide_placement == plotium_figure::GuidePlacement::OutsideRight
}
