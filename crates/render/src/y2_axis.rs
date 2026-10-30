//! The right y axis, or the sequence scale in its place: its reserved width and
//! the geometry every backend draws.
//!
//! Layout, screen, SVG, EMF and diagnostics all read [`RightAxis`], so the
//! right spine, ticks, labels and title sit in the same place in every output.

use crate::ticks::{AxisLayout, AxisTicks, widest_tick_label};
use crate::{
    AXIS_LABEL_GAP, Projector, StackPlacement, TICK_LABEL_PAD, XFurniture, minor_tick_values,
};
use plotium_figure::{Axis, AxisFrame, Figure, YAxisSide};

/// The right axis, when the figure draws one.
fn axis(fig: &Figure) -> Option<&Axis> {
    fig.y2
        .as_ref()
        .or(fig.sequence_scale.as_ref().map(|scale| &scale.axis))
        .filter(|_| fig.axis_frame != AxisFrame::Hidden)
}

/// Room between the right axis line and the right edge of its widest tick
/// label, for tick labels `widest_label` wide. Mirrors the left axis.
pub(crate) fn tick_clearance(fig: &Figure, ticks: &AxisTicks, widest_label: f32) -> f32 {
    let ty = fig.typography;
    match axis(fig) {
        None => 0.0,
        Some(_) if ticks.values.is_empty() => 0.0,
        Some(axis) if axis.show_tick_labels => widest_label + ty.tick_outset() + TICK_LABEL_PAD,
        Some(_) => ty.tick_outset(),
    }
}

/// Whether the right axis draws its title. A blank title reserves no lane.
pub(crate) fn shows_title(fig: &Figure) -> bool {
    axis(fig).is_some_and(|axis| axis.show_label && !axis.label.trim().is_empty())
}

/// Width the right axis needs beside the plot: ticks, labels and title.
pub(crate) fn reserved_width(fig: &Figure, ticks: &AxisTicks, widest_label: f32) -> f32 {
    let title = if shows_title(fig) {
        AXIS_LABEL_GAP + fig.typography.label_pt
    } else {
        0.0
    };
    tick_clearance(fig, ticks, widest_label) + title
}

/// Room above the plot for the right axis "×10ⁿ" multiplier.
pub(crate) fn multiplier_clearance(fig: &Figure, ticks: &AxisTicks) -> f32 {
    match axis(fig) {
        Some(axis) if axis.show_tick_labels => ticks.multiplier_clearance(fig.typography.tick_pt),
        _ => 0.0,
    }
}

/// How far an outside-right guide must stand off the plot edge, in unscaled
/// points, to clear the right axis of a plot `plot_height` points tall.
pub(crate) fn guide_offset(fig: &Figure, plot_height: f32) -> f32 {
    if axis(fig).is_none() {
        return 0.0;
    }
    let ticks = crate::ticks::right_ticks(fig, plot_height);
    reserved_width(
        fig,
        &ticks,
        widest_tick_label(&ticks, fig.typography.tick_pt),
    )
}

/// Output-space right axis furniture of one plot, in drawing order.
pub(crate) struct RightAxis<'a> {
    /// Output x of the right axis line.
    pub x: f32,
    /// The open frame draws the right spine; a box frame already has one.
    pub spine: bool,
    /// Value and output y of each major tick, with its label when one is drawn.
    pub ticks: Vec<(f64, f32, Option<&'a str>)>,
    pub minor: Vec<f32>,
    /// Signed tick extents to the right, already scaled.
    pub tick_reach: f32,
    pub minor_reach: f32,
    /// Left edge of the tick labels, which are vertically centred on a tick.
    pub label_x: f32,
    /// The "×10ⁿ" multiplier, right-aligned at the axis. `inside` puts it in
    /// the plot's top-right corner, where a joined stack boundary leaves it.
    pub multiplier: Option<(String, f32, bool)>,
    /// Title text, the left edge of its `label_pt`-wide lane, and its centre y.
    /// The title reads bottom to top like the left one.
    pub title: Option<(&'a str, f32, f32)>,
}

/// Right axis geometry for a laid-out plot, or `None` without a right axis.
pub(crate) fn right_axis<'a>(
    fig: &'a Figure,
    layout: &'a AxisLayout,
    proj: &Projector<'_>,
    stack: Option<&StackPlacement>,
    scale: f32,
) -> Option<RightAxis<'a>> {
    let axis = axis(fig)?;
    let ty = fig.typography;
    let xf = XFurniture::of(fig, stack);
    let plot = proj.plot;
    let right = proj.on(YAxisSide::Right);
    let at = |value: f64| right.project([fig.x.min, value]).1;
    let ticks = layout
        .y2_ticks
        .values
        .iter()
        .zip(&layout.y2_ticks.labels)
        .map(|(&value, label)| {
            let y = at(value);
            let shown = axis.show_tick_labels && xf.shows_y_label_at((y - plot.top) / scale);
            (value, y, shown.then_some(label.as_str()))
        })
        .collect();
    let minor = if fig.sequence_scale.is_some() {
        Vec::new()
    } else {
        minor_tick_values(axis, &layout.y2_ticks, ty.minor_ticks)
            .into_iter()
            .map(at)
            .collect()
    };
    let multiplier = layout
        .y2_ticks
        .multiplier()
        .filter(|_| axis.show_tick_labels)
        .map(|text| {
            if xf.y_multiplier_inside {
                let inset = (ty.tick_length_pt + TICK_LABEL_PAD) * scale;
                (text, plot.right() - inset, true)
            } else {
                (text, plot.right(), false)
            }
        });
    let clearance = stack.map_or_else(|| layout.y2_tick_clearance(fig), |p| p.y2_tick_clearance);
    let title = shows_title(fig).then(|| {
        (
            axis.label.as_str(),
            plot.right() + (clearance + AXIS_LABEL_GAP) * scale,
            (plot.top + plot.bottom()) * 0.5,
        )
    });
    Some(RightAxis {
        x: plot.right(),
        spine: fig.axis_frame == AxisFrame::Open,
        ticks,
        minor,
        tick_reach: ty.tick_reach() * scale,
        minor_reach: ty.minor_tick_reach() * scale,
        label_x: plot.right() + (ty.tick_outset() + TICK_LABEL_PAD) * scale,
        multiplier,
        title,
    })
}

#[cfg(feature = "screen")]
pub(crate) fn paint(
    painter: &egui::Painter,
    fig: &Figure,
    geometry: &RightAxis<'_>,
    plot: crate::Rect,
    scale: f32,
) {
    use egui::{Align2, Color32, Pos2, Stroke, Vec2};
    let ty = fig.typography;
    let ink = Color32::from_rgb(
        plotium_figure::Color::AXIS.r,
        plotium_figure::Color::AXIS.g,
        plotium_figure::Color::AXIS.b,
    );
    let stroke = Stroke::new(ty.axis_line_pt * scale, ink);
    let font = crate::rich_text::plot_font(ty.tick_pt * scale, &ty);
    let x = geometry.x;
    if geometry.spine {
        painter.line_segment(
            [Pos2::new(x, plot.top), Pos2::new(x, plot.bottom())],
            stroke,
        );
    }
    for &y in &geometry.minor {
        let ends = [Pos2::new(x, y), Pos2::new(x + geometry.minor_reach, y)];
        painter.line_segment(ends, stroke);
    }
    for &(_, y, label) in &geometry.ticks {
        let ends = [Pos2::new(x, y), Pos2::new(x + geometry.tick_reach, y)];
        painter.line_segment(ends, stroke);
        if let Some(label) = label {
            let at = Pos2::new(geometry.label_x, y);
            crate::rich_text::paint(painter, at, Align2::LEFT_CENTER, label, font.clone(), ink);
        }
    }
    if let Some((text, mx, inside)) = &geometry.multiplier {
        let (at, anchor) = if *inside {
            let y = plot.top + TICK_LABEL_PAD * scale;
            (Pos2::new(*mx, y), Align2::RIGHT_TOP)
        } else {
            let y = plot.top - TICK_LABEL_PAD * scale;
            (Pos2::new(*mx, y), Align2::RIGHT_BOTTOM)
        };
        crate::rich_text::paint(painter, at, anchor, text, font, ink);
    }
    if let Some((text, lane, centre)) = geometry.title {
        let font = crate::rich_text::plot_font(ty.label_pt * scale, &ty);
        let galley = crate::rich_text::galley(painter, text, font, ink);
        let size = galley.size();
        let mut title =
            egui::epaint::TextShape::new(Pos2::new(-size.x * 0.5, -size.y * 0.5), galley, ink)
                .with_angle_and_anchor(-std::f32::consts::FRAC_PI_2, Align2::CENTER_CENTER);
        title.pos += Vec2::new(lane + ty.label_pt * 0.5 * scale, centre);
        painter.add(title);
    }
}

pub(crate) fn write_svg(s: &mut String, fig: &Figure, geometry: &RightAxis<'_>, plot: crate::Rect) {
    use crate::rich_text::svg_content;
    use std::fmt::Write;
    let ty = fig.typography;
    let ink = plotium_figure::Color::AXIS.to_hex();
    let x = geometry.x;
    let mut path = String::new();
    if geometry.spine {
        let _ = write!(
            path,
            "M{x:.2} {t:.2}V{b:.2}",
            t = plot.top,
            b = plot.bottom()
        );
    }
    for &y in &geometry.minor {
        let _ = write!(path, "M{x:.2} {y:.2}h{r}", r = geometry.minor_reach);
    }
    for &(_, y, _) in &geometry.ticks {
        let _ = write!(path, "M{x:.2} {y:.2}h{r}", r = geometry.tick_reach);
    }
    if !path.is_empty() {
        let _ = write!(
            s,
            r#"<path class="axis-y2" d="{path}" fill="none" stroke="{ink}" stroke-width="{w}"/>"#,
            w = ty.axis_line_pt,
        );
    }
    for &(_, y, label) in &geometry.ticks {
        let Some(label) = label else { continue };
        let _ = write!(
            s,
            r#"<text x="{lx:.2}" y="{y:.2}" text-anchor="start" font-size="{font}" fill="{ink}">{text}</text>"#,
            lx = geometry.label_x,
            y = y + ty.tick_pt * crate::rich_text::MIDDLE_TO_BASELINE,
            font = ty.tick_pt,
            text = svg_content(label, ty.tick_pt),
        );
    }
    if let Some((text, mx, inside)) = &geometry.multiplier {
        let y = if *inside {
            plot.top + TICK_LABEL_PAD + ty.tick_pt
        } else {
            plot.top - TICK_LABEL_PAD
        };
        let _ = write!(
            s,
            r#"<text x="{mx:.2}" y="{y:.2}" text-anchor="end" font-size="{font}" fill="{ink}">{text}</text>"#,
            font = ty.tick_pt,
            text = svg_content(text, ty.tick_pt),
        );
    }
    if let Some((text, lane, centre)) = geometry.title {
        // As on the left, the rotated glyph tops face the lane's left edge.
        let _ = write!(
            s,
            r#"<text transform="translate({x:.2},{centre:.2}) rotate(-90)" text-anchor="middle" font-size="{font}" fill="{ink}">{text}</text>"#,
            x = lane + ty.label_pt * crate::rich_text::TOP_TO_BASELINE,
            font = ty.label_pt,
            text = svg_content(text, ty.label_pt),
        );
    }
}

#[cfg(test)]
#[path = "y2_axis_tests.rs"]
mod tests;
