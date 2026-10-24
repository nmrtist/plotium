use crate::{AXIS_LABEL_GAP, Margins, OUTER_PAD, Projector, Rect, TICK_LABEL_PAD};
use plotium_figure::{Axis, AxisFrame, AxisScale, Figure};
use unicode_width::UnicodeWidthChar;

pub(crate) const MAX_X_TARGET: usize = 8;
pub(crate) const MAX_Y_TARGET: usize = 5;
const LABEL_GAP_FACTOR: f32 = 0.8;
const Y_TICK_SPACING_FACTOR: f32 = 1.8;
/// Closest label centres a short panel accepts: one label height plus a
/// small gap, so the labels never touch.
const Y_TICK_MIN_SPACING_FACTOR: f32 = 1.2;
/// f64 represents every integer below 2⁵³ ≈ 9×10¹⁵ exactly.
const GROUPED_FULL_EXPONENT_LIMIT: i32 = 15;

/// Margins and tick sets computed together for a particular output size.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisLayout {
    pub margins: Margins,
    pub x_ticks: AxisTicks,
    pub y_ticks: AxisTicks,
    /// Ticks of the right y axis; empty when the figure has none.
    pub y2_ticks: AxisTicks,
}

impl AxisLayout {
    /// Room between the y axis line and the left edge of the widest y tick
    /// label this layout draws.
    pub fn y_tick_clearance(&self, fig: &Figure) -> f32 {
        y_tick_clearance(
            fig,
            &self.y_ticks,
            widest_tick_label(&self.y_ticks, fig.typography.tick_pt),
        )
    }

    /// Room between the right axis line and the right edge of the widest
    /// right-axis tick label; zero without a right axis.
    pub fn y2_tick_clearance(&self, fig: &Figure) -> f32 {
        crate::y2_axis::tick_clearance(
            fig,
            &self.y2_ticks,
            widest_tick_label(&self.y2_ticks, fig.typography.tick_pt),
        )
    }

    /// Width the right axis reserves beside the plot: ticks, labels and
    /// title; zero without a right axis.
    pub fn right_axis_width(&self, fig: &Figure) -> f32 {
        crate::y2_axis::reserved_width(
            fig,
            &self.y2_ticks,
            widest_tick_label(&self.y2_ticks, fig.typography.tick_pt),
        )
    }
}

/// Compute axes in page units, adapting tick density to the final plot rect.
///
/// The first pass uses the normal maximum tick budgets to establish margins.
/// The second and final pass chooses ticks from the resulting plot size and
/// recomputes margins. Avoiding another geometry pass keeps the result stable;
/// the second-pass margins can only make the available plot area less
/// constrained than the conservative first pass.
pub fn axis_layout(fig: &Figure, outer_width: f32, outer_height: f32) -> AxisLayout {
    if fig.axis_frame == AxisFrame::Hidden {
        let x_ticks = AxisTicks::empty();
        let y_ticks = AxisTicks::empty();
        let y2_ticks = AxisTicks::empty();
        return AxisLayout {
            margins: margins_for_ticks(fig, &x_ticks, &y_ticks, &y2_ticks, outer_width),
            x_ticks,
            y_ticks,
            y2_ticks,
        };
    }

    let initial_x = axis_ticks_for(&fig.x, MAX_X_TARGET);
    let initial_y = axis_ticks_for(&fig.y, MAX_Y_TARGET);
    let initial_y2 = fig
        .y2
        .as_ref()
        .map_or_else(AxisTicks::empty, |axis| axis_ticks_for(axis, MAX_Y_TARGET));
    // Categorical thinning can select a different subset at a lower target.
    // Size the first pass for every visible category so no second-pass label
    // can make the final margins grow and invalidate the fit checks below.
    // This deliberately reserves names that may not ultimately be drawn; on a
    // narrow figure that conservative cost can reduce the x tick budget.
    let initial_widths = TickLabelWidths {
        x: widest_layout_label(&fig.x, &initial_x, fig.typography.tick_pt),
        y: widest_layout_label(&fig.y, &initial_y, fig.typography.tick_pt),
        y2: widest_tick_label(&initial_y2, fig.typography.tick_pt),
    };
    let initial_margins = margins_for_ticks_with_widths(
        fig,
        &initial_x,
        [&initial_y, &initial_y2],
        initial_widths,
        outer_width,
    );
    let outer = Rect::new(0.0, 0.0, outer_width.max(0.0), outer_height.max(0.0));
    let plot = Projector::new(fig, outer, &initial_margins).plot;
    let gap = fig.typography.tick_pt * LABEL_GAP_FACTOR;

    let x_ticks = adaptive_x_ticks(fig, plot.width, gap, initial_widths.x);
    let y_ticks = adaptive_y_ticks(&fig.y, fig.typography.tick_pt, plot.height);
    let y2_ticks = fig.y2.as_ref().map_or_else(AxisTicks::empty, |axis| {
        adaptive_y_ticks(axis, fig.typography.tick_pt, plot.height)
    });
    let margins = margins_for_ticks(fig, &x_ticks, &y_ticks, &y2_ticks, outer_width);

    AxisLayout {
        margins,
        x_ticks,
        y_ticks,
        y2_ticks,
    }
}

/// The final x tick set for a plot area of `plot_width`, sized with the same
/// conservative label widths as the first pass of [`axis_layout`].
pub(crate) fn adaptive_x_ticks_for_plot(fig: &Figure, plot_width: f32) -> AxisTicks {
    let initial = axis_ticks_for(&fig.x, MAX_X_TARGET);
    let widest = widest_layout_label(&fig.x, &initial, fig.typography.tick_pt);
    let gap = fig.typography.tick_pt * LABEL_GAP_FACTOR;
    adaptive_x_ticks(fig, plot_width, gap, widest)
}

fn adaptive_x_ticks(fig: &Figure, plot_width: f32, gap: f32, widest: f32) -> AxisTicks {
    let slot_width = widest + gap;
    if !plot_width.is_finite() || slot_width <= 0.0 || plot_width < 2.0 * slot_width {
        return AxisTicks::empty();
    }

    let target = ((plot_width / slot_width).floor() as usize).clamp(2, MAX_X_TARGET);
    for candidate in (2..=target).rev() {
        let ticks = axis_ticks_for(&fig.x, candidate);
        if horizontal_labels_fit(&fig.x, &ticks, plot_width, fig.typography.tick_pt, gap) {
            return ticks;
        }
    }
    AxisTicks::empty()
}

pub(crate) fn adaptive_y_ticks(axis: &Axis, tick_pt: f32, plot_height: f32) -> AxisTicks {
    let spacing = tick_pt * Y_TICK_SPACING_FACTOR;
    if !plot_height.is_finite() || plot_height <= 0.0 || spacing <= 0.0 {
        return AxisTicks::empty();
    }
    if plot_height < 2.0 * spacing {
        return sparse_y_ticks(axis, tick_pt, plot_height);
    }

    // A category axis names every slot that fits; the numeric cap only keeps
    // value ticks sparse.
    let cap = match &axis.categories {
        Some(_) => visible_categories(axis).count().max(2),
        None => MAX_Y_TARGET,
    };
    let target = ((plot_height / spacing).floor() as usize).clamp(2, cap);
    for candidate in (2..=target).rev() {
        let ticks = axis_ticks_for(axis, candidate);
        // A coarse step can leave a single tick in range; one label shows no
        // scale, so it is only the last resort below.
        if ticks.values.len() >= 2 && vertical_labels_fit(axis, &ticks, plot_height, tick_pt) {
            return ticks;
        }
    }
    sparse_y_ticks(axis, tick_pt, plot_height)
}

/// Ticks for a panel too short for evenly spaced labels: the lowest and
/// highest ticks of the coarsest set whose labels do not touch, otherwise
/// the single tick nearest the middle, so the axis keeps a readable value. A
/// panel shorter than one label keeps none.
///
/// Ticks within half a label of the top edge are passed over: a stacked
/// member joined above skips those labels (`XFurniture::shows_y_label_at`),
/// which would leave a chosen pair with one visible label.
fn sparse_y_ticks(axis: &Axis, tick_pt: f32, plot_height: f32) -> AxisTicks {
    if plot_height < tick_pt {
        return AxisTicks::empty();
    }
    let position = |value: f64| axis.normalize(value) as f32 * plot_height;
    let highest = plot_height - tick_pt * 0.5;
    let mut middle = None;
    for candidate in 2..=MAX_Y_TARGET {
        let ticks = axis_ticks_for(axis, candidate);
        let mut order: Vec<usize> = (0..ticks.values.len())
            .filter(|&index| position(ticks.values[index]) <= highest)
            .collect();
        order.sort_by(|&a, &b| position(ticks.values[a]).total_cmp(&position(ticks.values[b])));
        let (Some(&low), Some(&high)) = (order.first(), order.last()) else {
            continue;
        };
        if position(ticks.values[high]) - position(ticks.values[low])
            >= tick_pt * Y_TICK_MIN_SPACING_FACTOR
        {
            // Index order keeps the values ascending, as every tick set is.
            return ticks_subset(&ticks, &[low.min(high), low.max(high)]);
        }
        if middle.is_none() {
            let offset = |index: usize| (position(ticks.values[index]) - plot_height * 0.5).abs();
            let nearest = order
                .iter()
                .copied()
                .min_by(|&a, &b| offset(a).total_cmp(&offset(b)));
            middle = nearest.map(|index| ticks_subset(&ticks, &[index]));
        }
    }
    middle.unwrap_or_else(AxisTicks::empty)
}

fn ticks_subset(ticks: &AxisTicks, indices: &[usize]) -> AxisTicks {
    AxisTicks {
        values: indices.iter().map(|&index| ticks.values[index]).collect(),
        labels: indices
            .iter()
            .map(|&index| ticks.labels[index].clone())
            .collect(),
        ..ticks.clone()
    }
}

fn widest_layout_label(axis: &Axis, ticks: &AxisTicks, tick_pt: f32) -> f32 {
    if axis.categories.is_some() {
        visible_categories(axis)
            .map(|(_, label)| estimated_text_width(label, tick_pt))
            .fold(0.0, f32::max)
    } else {
        widest_tick_label(ticks, tick_pt)
    }
}

pub(crate) fn widest_tick_label(ticks: &AxisTicks, tick_pt: f32) -> f32 {
    ticks
        .labels
        .iter()
        .map(|label| estimated_text_width(label, tick_pt))
        .fold(0.0, f32::max)
}

fn horizontal_labels_fit(
    axis: &Axis,
    ticks: &AxisTicks,
    width: f32,
    tick_pt: f32,
    gap: f32,
) -> bool {
    let mut intervals: Vec<(f32, f32)> = ticks
        .values
        .iter()
        .zip(&ticks.labels)
        .map(|(&value, label)| {
            let center = axis.normalize(value) as f32 * width;
            let half = estimated_text_width(label, tick_pt) * 0.5;
            (center - half, center + half)
        })
        .collect();
    intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
    intervals
        .windows(2)
        .all(|pair| pair[0].1 + gap <= pair[1].0)
}

fn vertical_labels_fit(axis: &Axis, ticks: &AxisTicks, height: f32, tick_pt: f32) -> bool {
    let mut centers: Vec<f32> = ticks
        .values
        .iter()
        .map(|&value| axis.normalize(value) as f32 * height)
        .collect();
    centers.sort_by(f32::total_cmp);
    centers
        .windows(2)
        .all(|pair| pair[1] - pair[0] >= tick_pt * Y_TICK_SPACING_FACTOR)
}

pub(crate) fn margins_for_ticks(
    fig: &Figure,
    x_ticks: &AxisTicks,
    y_ticks: &AxisTicks,
    y2_ticks: &AxisTicks,
    outer_width: f32,
) -> Margins {
    let widths = TickLabelWidths {
        x: widest_tick_label(x_ticks, fig.typography.tick_pt),
        y: widest_tick_label(y_ticks, fig.typography.tick_pt),
        y2: widest_tick_label(y2_ticks, fig.typography.tick_pt),
    };
    margins_for_ticks_with_widths(fig, x_ticks, [y_ticks, y2_ticks], widths, outer_width)
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

/// How far the x tick labels reach left of the plot's left edge when
/// `clearance`, this overhang itself or more, is reserved beside the plot.
///
/// A label at fraction `t` of the plot width reaches `half - t * plot_width`
/// past the edge, and the plot width shrinks as the clearance grows. Solving
/// `c = half - t * (available - c)` for each label gives the least clearance
/// that holds every label, so labels centred inside the plot, such as the
/// first category slot, need none.
fn x_label_overhang(fig: &Figure, x_ticks: &AxisTicks, available: f32) -> f32 {
    let tick_pt = fig.typography.tick_pt;
    let mut widest_half = 0.0f32;
    let overhang = x_ticks
        .values
        .iter()
        .zip(&x_ticks.labels)
        .filter_map(|(&value, label)| {
            let half = estimated_text_width(label, tick_pt) * 0.5;
            widest_half = widest_half.max(half);
            let t = fig.x.normalize(value) as f32;
            (t.is_finite() && t < 1.0).then(|| (half - t.max(0.0) * available) / (1.0 - t.max(0.0)))
        })
        .fold(0.0f32, f32::max);
    // A degenerate plot width must not reserve more than a label edge-on.
    overhang.min(widest_half)
}

#[derive(Clone, Copy)]
struct TickLabelWidths {
    x: f32,
    y: f32,
    y2: f32,
}

fn margins_for_ticks_with_widths(
    fig: &Figure,
    x_ticks: &AxisTicks,
    [y_ticks, y2_ticks]: [&AxisTicks; 2],
    widths: TickLabelWidths,
    outer_width: f32,
) -> Margins {
    let ty = fig.typography;
    let (guide_right, guide_bottom) = crate::guide_extra_margins(fig);
    if fig.axis_frame == AxisFrame::Hidden {
        let title_clearance = if fig.title.trim().is_empty() {
            0.0
        } else {
            ty.title_pt + AXIS_LABEL_GAP
        };
        return Margins {
            left: OUTER_PAD,
            right: OUTER_PAD + guide_right,
            top: OUTER_PAD + title_clearance,
            bottom: OUTER_PAD + guide_bottom,
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
    let x_width = if fig.x.show_tick_labels {
        widths.x
    } else {
        0.0
    };
    // A right axis needs its ticks, labels and title beside the plot.
    let right_axis = crate::y2_axis::reserved_width(fig, y2_ticks, widths.y2);
    let right = (OUTER_PAD + x_width * 0.5)
        .max(8.0)
        .max(OUTER_PAD + right_axis)
        + guide_right;
    // Keep a left-end x label out of the rotated y-title lane even when the
    // y ticks themselves have been dropped on a short panel.
    let x_endpoint_clearance = if !fig.x.show_tick_labels {
        0.0
    } else {
        let available = outer_width - OUTER_PAD - y_title_clearance - right;
        x_label_overhang(fig, x_ticks, available)
    };
    let left = OUTER_PAD + y_title_clearance + y_tick_clearance.max(x_endpoint_clearance);
    let title_clearance = if fig.title.trim().is_empty() {
        0.0
    } else {
        ty.title_pt + AXIS_LABEL_GAP
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
    let top = OUTER_PAD + title_clearance + y_multiplier;
    let bottom = OUTER_PAD + x_multiplier + x_title_clearance + x_tick_clearance + guide_bottom;

    Margins {
        left,
        right,
        top,
        bottom,
    }
}

/// The 1/2/5×10ⁿ spacing [`ticks`] uses for `target` ticks over `span`.
pub(crate) fn nice_step(span: f64, target: usize) -> f64 {
    let raw_step = span / target as f64;
    let mag = 10f64.powf(raw_step.log10().floor());
    let norm = raw_step / mag;
    let nice = if norm < 1.5 {
        1.0
    } else if norm < 3.0 {
        2.0
    } else if norm < 7.0 {
        5.0
    } else {
        10.0
    };
    nice * mag
}

/// Up to `target` "nice" tick values covering `[min, max]`, using 1/2/5×10ⁿ
/// rounding so ticks land on human-friendly numbers.
pub fn ticks(min: f64, max: f64, target: usize) -> Vec<f64> {
    let (lo, hi) = if min <= max { (min, max) } else { (max, min) };
    let span = hi - lo;
    if !span.is_finite() || span <= 0.0 || target == 0 {
        return vec![lo];
    }
    let step = nice_step(span, target);

    let eps = step * 1e-9;
    let first = ((lo - eps) / step).ceil() * step;
    let mut out = Vec::new();
    let mut value = first;
    let mut guard = 0;
    while value <= hi + eps && guard < 1000 {
        out.push(if value.abs() < step * 1e-9 {
            0.0
        } else {
            value
        });
        value += step;
        guard += 1;
    }
    out
}

/// Tick positions plus labels formatted as one axis-wide system.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisTicks {
    pub values: Vec<f64>,
    pub labels: Vec<String>,
    pub scale_exponent: Option<i32>,
    /// Majors sit on powers of ten of a log axis.
    pub decades: bool,
}

impl AxisTicks {
    pub fn empty() -> Self {
        Self {
            values: Vec::new(),
            labels: Vec::new(),
            scale_exponent: None,
            decades: false,
        }
    }

    pub fn multiplier(&self) -> Option<String> {
        self.scale_exponent
            .map(|exponent| format!("×10{}", superscript(exponent)))
    }

    pub fn multiplier_clearance(&self, tick_pt: f32) -> f32 {
        if self.scale_exponent.is_some() {
            tick_pt + AXIS_LABEL_GAP
        } else {
            0.0
        }
    }
}

pub(crate) fn estimated_text_width(text: &str, font_size: f32) -> f32 {
    if crate::rich_text::has_scripts(text) {
        return crate::rich_text::runs(text)
            .iter()
            .map(|run| estimated_plain_width(&run.text, font_size * run.script.scale()))
            .sum();
    }
    estimated_plain_width(text, font_size)
}

fn estimated_plain_width(text: &str, font_size: f32) -> f32 {
    text.chars()
        .map(|ch| match ch {
            '0'..='9' => 0.56,
            '.' | ',' => 0.28,
            '-' | '−' => 0.36,
            _ => match ch.width() {
                None | Some(0) => 0.0,
                Some(1) => 0.58,
                Some(_) => 1.0,
            },
        })
        .sum::<f32>()
        * font_size
}

/// Ticks for an axis honoring its ordinal mode.
pub fn axis_ticks_for(axis: &Axis, target: usize) -> AxisTicks {
    if axis.categories.is_none()
        && let Some(ticks) = decade_ticks_for(axis, target)
    {
        return ticks;
    }
    if axis.categories.is_none()
        && let Some(pieces) = axis.break_pieces()
    {
        return crate::axis_break::ticks(axis, pieces, target);
    }
    if axis.categories.is_none() {
        if let Some(display) = &axis.display {
            let mut result = format_axis_ticks(
                display.to_display(axis.min),
                display.to_display(axis.max),
                target,
                axis.thousands_separator,
            );
            for value in &mut result.values {
                *value = display.to_coordinate(*value);
            }
            return result;
        }
        return format_axis_ticks(axis.min, axis.max, target, axis.thousands_separator);
    }
    let visible: Vec<(f64, &str)> = visible_categories(axis).collect();
    let stride = visible.len().div_ceil(target.max(1)).max(1);
    let mut values = Vec::new();
    let mut labels = Vec::new();
    for (value, name) in visible.iter().step_by(stride) {
        values.push(*value);
        labels.push((*name).to_owned());
    }
    AxisTicks {
        values,
        labels,
        scale_exponent: None,
        decades: false,
    }
}

/// Minor tick positions: `count` evenly spaced values in every major interval,
/// including the partial intervals before the first and after the last major
/// tick. Categorical axes and axes with fewer than two major ticks have none.
pub fn minor_tick_values(axis: &Axis, ticks: &AxisTicks, count: u8) -> Vec<f64> {
    if ticks.decades {
        return if count == 0 {
            Vec::new()
        } else {
            decade_minors_for(axis, ticks)
        };
    }
    if let Some(pieces) = axis.break_pieces() {
        return crate::axis_break::minor_ticks(axis, pieces, ticks, count);
    }
    let (Some(&first), Some(&second)) = (ticks.values.first(), ticks.values.get(1)) else {
        return Vec::new();
    };
    let step = second - first;
    if count == 0 || axis.categories.is_some() || !step.is_finite() || step == 0.0 {
        return Vec::new();
    }
    let (lo, hi) = (axis.min.min(axis.max), axis.min.max(axis.max));
    let eps = step.abs() * 1e-9;
    let sub = step / (f64::from(count) + 1.0);
    (-1..ticks.values.len() as i64)
        .flat_map(|interval| {
            let start = first + interval as f64 * step;
            (1..=count).map(move |k| start + f64::from(k) * sub)
        })
        .filter(|value| *value >= lo - eps && *value <= hi + eps)
        .collect()
}

/// Log axes label powers of ten of the displayed unit. A window with fewer
/// than two visible powers, or a display unit that is not positive there,
/// uses the linear ticks instead.
fn decade_ticks_for(axis: &Axis, target: usize) -> Option<AxisTicks> {
    if axis.effective_scale() != AxisScale::Log10 {
        return None;
    }
    let Some(display) = &axis.display else {
        return crate::log_ticks::decade_ticks(axis.min, axis.max, target);
    };
    let mut ticks = crate::log_ticks::decade_ticks(
        display.to_display(axis.min),
        display.to_display(axis.max),
        target,
    )?;
    for value in &mut ticks.values {
        *value = display.to_coordinate(*value);
    }
    Some(ticks)
}

fn decade_minors_for(axis: &Axis, ticks: &AxisTicks) -> Vec<f64> {
    let Some(display) = &axis.display else {
        return crate::log_ticks::decade_minor_values(axis.min, axis.max, ticks);
    };
    let majors = AxisTicks {
        values: ticks
            .values
            .iter()
            .map(|v| display.to_display(*v))
            .collect(),
        ..ticks.clone()
    };
    crate::log_ticks::decade_minor_values(
        display.to_display(axis.min),
        display.to_display(axis.max),
        &majors,
    )
    .into_iter()
    .map(|value| display.to_coordinate(value))
    .collect()
}

fn visible_categories(axis: &Axis) -> impl Iterator<Item = (f64, &str)> {
    let (lo, hi) = (axis.min.min(axis.max), axis.min.max(axis.max));
    axis.categories
        .iter()
        .flat_map(|names| names.iter().enumerate())
        .filter_map(move |(index, name)| {
            let value = index as f64;
            (value >= lo - 1e-9 && value <= hi + 1e-9).then_some((value, name.as_str()))
        })
}

pub fn axis_ticks(min: f64, max: f64, target: usize) -> AxisTicks {
    format_axis_ticks(min, max, target, false)
}

fn format_axis_ticks(min: f64, max: f64, target: usize, group_thousands: bool) -> AxisTicks {
    let values = ticks(min, max, target);
    let step = values
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).abs())
        .find(|step| step.is_finite() && *step > 0.0)
        .unwrap_or(f64::NAN);
    label_tick_values(values, step, min.abs().max(max.abs()), group_thousands)
}

/// Label `values` as one system: a shared ×10ⁿ multiplier chosen from
/// `max_abs` and a precision that resolves `step`.
pub(crate) fn label_tick_values(
    values: Vec<f64>,
    step: f64,
    max_abs: f64,
    group_thousands: bool,
) -> AxisTicks {
    let exponent = if max_abs.is_finite() && max_abs > 0.0 {
        max_abs.log10().floor() as i32
    } else {
        0
    };
    // Grouped labels are written in full up to the largest magnitude whose
    // integers f64 still prints exactly; beyond that, and for tiny values, the
    // ×10ⁿ multiplier stays.
    let large_exponent = if group_thousands {
        GROUPED_FULL_EXPONENT_LIMIT
    } else {
        4
    };
    let scale_exponent = (exponent >= large_exponent || exponent <= -4).then_some(exponent);
    let scale = scale_exponent.map_or(1.0, |value| 10f64.powi(value));
    let scaled_step = Some(step / scale)
        .filter(|step| step.is_finite() && *step > 0.0)
        .unwrap_or(1.0);
    let precision = decimal_places(scaled_step);
    let zero_threshold = 0.5 * 10f64.powi(-(precision as i32));
    let labels = values
        .iter()
        .map(|value| {
            let scaled = value / scale;
            let clean = if scaled.abs() < zero_threshold {
                0.0
            } else {
                scaled
            };
            let label = format!("{clean:.precision$}");
            if group_thousands {
                group_integer_digits(&label)
            } else {
                label
            }
        })
        .collect();

    AxisTicks {
        values,
        labels,
        scale_exponent,
        decades: false,
    }
}

/// Insert commas every three integer digits of a plain decimal label, leaving
/// the sign and fractional part untouched: `-12345.5` → `-12,345.5`.
fn group_integer_digits(label: &str) -> String {
    let (sign, unsigned) = match label.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", label),
    };
    let (integer, fraction) = match unsigned.find('.') {
        Some(dot) => unsigned.split_at(dot),
        None => (unsigned, ""),
    };
    let mut grouped = String::with_capacity(label.len() + integer.len() / 3);
    grouped.push_str(sign);
    for (index, digit) in integer.chars().enumerate() {
        if index > 0 && (integer.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped.push_str(fraction);
    grouped
}

fn decimal_places(step: f64) -> usize {
    if !step.is_finite() || step <= 0.0 {
        return 0;
    }
    (-(step.log10().floor() as i32)).clamp(0, 8) as usize
}

fn superscript(value: i32) -> String {
    value
        .to_string()
        .chars()
        .map(|ch| match ch {
            '-' => '⁻',
            '0' => '⁰',
            '1' => '¹',
            '2' => '²',
            '3' => '³',
            '4' => '⁴',
            '5' => '⁵',
            '6' => '⁶',
            '7' => '⁷',
            '8' => '⁸',
            '9' => '⁹',
            _ => ch,
        })
        .collect()
}

#[cfg(test)]
#[path = "ticks_tests.rs"]
mod tests;
