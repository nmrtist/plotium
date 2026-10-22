use crate::{ticks::estimated_text_width, *};

pub struct TextBox {
    pub component: String,
    pub bounds: Rect,
    pub container: Option<Rect>,
}
/// Native SVG line spacing and alignment; the width is the renderer's existing
/// estimate, not a shaped glyph extent. The editor's minimum screen font differs.
pub fn overlay_text_bounds(
    frame: Rect,
    text: &str,
    font: f32,
    align: OverlayAlign,
) -> Option<Rect> {
    if text.trim().is_empty() {
        return None;
    }
    let width = text
        .lines()
        .map(|s| estimated_text_width(s, font))
        .fold(0., f32::max);
    let height = font * (1. + 1.25 * (text.lines().count().saturating_sub(1)) as f32);
    let x = match align {
        OverlayAlign::Left => frame.left,
        OverlayAlign::Center => frame.left + (frame.width - width) * 0.5,
        OverlayAlign::Right => frame.right() - width,
    };
    Some(Rect::new(x, frame.top, width, height))
}
pub fn figure_text_boxes(fig: &Figure, size: [f32; 2]) -> Vec<TextBox> {
    figure_text_boxes_placed(fig, size, None)
}

/// Text boxes of a plot that may sit in a shared-x stack.
pub fn figure_text_boxes_placed(
    fig: &Figure,
    size: [f32; 2],
    stack: Option<&StackPlacement>,
) -> Vec<TextBox> {
    let layout = axis_layout_placed(fig, size[0], size[1], stack);
    let xf = XFurniture::of(fig, stack);
    let outer = Rect::new(0., 0., size[0], size[1]);
    let proj = Projector::new(fig, outer, &layout.margins);
    let plot = proj.plot;
    let x_lane_bottom = size[1] - proj.bottom_letterbox(outer, &layout.margins);
    let ty = fig.typography;
    let y_title_cy = (plot.top + plot.bottom()) * 0.5;
    let y_title_left = y_title_left(fig, &layout, &proj, outer, y_title_cy, stack, 1.);
    let mut out = Vec::new();
    let mut add = |part: &str,
                   text: &str,
                   x: f32,
                   y: f32,
                   font: f32,
                   anchor: f32,
                   vertical: bool,
                   container: Option<Rect>| {
        if text.trim().is_empty() {
            return;
        }
        let w = estimated_text_width(text, font);
        let bounds = if vertical {
            Rect::new(x - font * 0.5, y - w * anchor, font, w)
        } else {
            Rect::new(x - w * anchor, y, w, font)
        };
        out.push(TextBox {
            component: part.into(),
            bounds,
            container,
        });
    };
    add(
        "title",
        if xf.show_title { &fig.title } else { "" },
        size[0] * 0.5,
        OUTER_PAD,
        ty.title_pt,
        0.5,
        false,
        None,
    );
    let y_axis_x = proj.left_band.map_or(plot.left, |b| b.left);
    if xf.show_tick_labels {
        for (v, label) in layout.x_ticks.values.iter().zip(&layout.x_ticks.labels) {
            let (x, _) = proj.project([*v, fig.y.min]);
            add(
                &format!("axis.x.tick:{v}"),
                label,
                x,
                plot.bottom() + ty.tick_outset() + TICK_LABEL_PAD,
                ty.tick_pt,
                0.5,
                false,
                None,
            );
        }
        if let Some(m) = layout.x_ticks.multiplier() {
            add(
                "axis.x.multiplier",
                &m,
                plot.right(),
                x_lane_bottom - OUTER_PAD - ty.tick_pt,
                ty.tick_pt,
                1.,
                false,
                None,
            );
        }
    }
    if fig.y.show_tick_labels {
        for (v, label) in layout.y_ticks.values.iter().zip(&layout.y_ticks.labels) {
            let (_, y) = proj.project([fig.x.min, *v]);
            if !xf.shows_y_label_at(y - plot.top) {
                continue;
            }
            add(
                &format!("axis.y.tick:{v}"),
                label,
                y_axis_x - ty.tick_outset() - TICK_LABEL_PAD,
                y - ty.tick_pt * 0.5,
                ty.tick_pt,
                1.,
                false,
                None,
            );
        }
        if let Some(m) = layout.y_ticks.multiplier() {
            add(
                "axis.y.multiplier",
                &m,
                if xf.y_multiplier_inside {
                    y_axis_x + ty.tick_length_pt + TICK_LABEL_PAD
                } else {
                    y_axis_x
                },
                if xf.y_multiplier_inside {
                    plot.top + TICK_LABEL_PAD
                } else {
                    plot.top - TICK_LABEL_PAD - ty.tick_pt
                },
                ty.tick_pt,
                0.,
                false,
                None,
            );
        }
    }
    if fig.axis_frame != plotium_figure::AxisFrame::Hidden {
        if xf.show_label {
            let clearance = if xf.show_tick_labels {
                layout.x_ticks.multiplier_clearance(ty.tick_pt)
            } else {
                0.
            };
            add(
                "axis.x.label",
                &fig.x.label,
                (plot.left + plot.right()) * 0.5,
                x_lane_bottom - OUTER_PAD - clearance - ty.label_pt,
                ty.label_pt,
                0.5,
                false,
                None,
            );
        }
        if fig.y.show_label {
            add(
                "axis.y.label",
                &fig.y.label,
                y_title_left + ty.label_pt * 0.5,
                y_title_cy,
                ty.label_pt,
                0.5,
                true,
                None,
            );
        }
    }
    if let Some(r) = legend_rect(fig, plot, 1.) {
        let entries = legend_entries(fig);
        let l = legend_layout(fig, &entries);
        add(
            "guide.legend.title",
            &fig.guide_title,
            r.left + l.padding,
            r.top + l.padding,
            ty.legend_pt,
            0.,
            false,
            Some(r),
        );
        // The owning guide is the addressable part; entry positions are only geometry.
        for (i, (name, _, _)) in entries.iter().enumerate() {
            let (x, y) = legend_entry_origin(&l, i);
            add(
                "guide.legend.text",
                name,
                r.left + x + l.swatch + 5.,
                r.top + y - ty.legend_pt * 0.5,
                ty.legend_pt,
                0.,
                false,
                Some(r),
            );
        }
    }
    if let (Some(heatmap), Some(r)) = (&fig.heatmap, color_scale_rect(fig, plot, 1.)) {
        let font = ty.legend_pt;
        let [min, max] = heatmap.value_range;
        for (part, value, x, y, anchor) in if r.width > r.height {
            [
                ("guide.color_scale.min", min, r.left, r.bottom() + 2., 0.),
                ("guide.color_scale.max", max, r.right(), r.bottom() + 2., 1.),
            ]
        } else {
            [
                (
                    "guide.color_scale.min",
                    min,
                    r.right() + 3.,
                    r.bottom() - font,
                    0.,
                ),
                ("guide.color_scale.max", max, r.right() + 3., r.top, 0.),
            ]
        } {
            add(
                part,
                &crate::svg::color_scale::format_value(value),
                x,
                y,
                font,
                anchor,
                false,
                None,
            );
        }
    }
    out
}
