use super::col;
use crate::{
    LegendMark, Rect, StackPlacement, legend_entries, legend_entry_origin, legend_layout,
    legend_rect_placed, renders_legend,
};
use egui::{Align2, Pos2, Stroke, Vec2};
use plotium_figure::Figure;

pub(super) fn paint(
    painter: &egui::Painter,
    plot: Rect,
    fig: &Figure,
    scale: f32,
    stack: Option<&StackPlacement>,
) {
    let entries = legend_entries(fig);
    if !renders_legend(fig) {
        return;
    }
    let layout = legend_layout(fig, &entries);
    let font = fig.typography.legend_pt * scale;
    let sw = layout.swatch * scale;
    let Some(box_geometry) = legend_rect_placed(fig, plot, scale, stack) else {
        return;
    };
    // Legends draw neither a frame nor a backing.
    let (bx, by) = (box_geometry.left, box_geometry.top);
    let font_id = crate::rich_text::plot_font(font, &fig.typography);
    if !fig.guide_title.trim().is_empty() {
        crate::rich_text::paint(
            painter,
            Pos2::new(bx + layout.padding * scale, by + layout.padding * scale),
            Align2::LEFT_TOP,
            &fig.guide_title,
            font_id.clone(),
            col(fig.typography.legend_color),
        );
    }
    for (i, (name, color, mark)) in entries.iter().enumerate() {
        let (ox, oy) = legend_entry_origin(&layout, i);
        let ly = by + oy * scale;
        let lx = bx + ox * scale;
        match mark {
            LegendMark::Curve {
                kind,
                appearance,
                width,
            } => {
                let series = crate::curve_style::legend_series(
                    *color,
                    *kind,
                    *appearance,
                    *width,
                    lx,
                    ly,
                    sw,
                );
                let points: Vec<_> = series
                    .points
                    .iter()
                    .map(|p| (p[0] as f32, p[1] as f32))
                    .collect();
                if *kind != plotium_figure::SeriesKind::Points {
                    super::curves::stroke(painter, &points, &series, scale, box_geometry);
                }
                let marker = appearance.marker(*kind);
                super::curves::marker_at(
                    painter,
                    (lx + sw * 0.5, ly),
                    marker,
                    appearance.marker_radius(*width) * scale,
                    &crate::curve_style::MarkerPaint::new(*appearance, *color, *width)
                        .scaled(scale),
                );
            }

            LegendMark::Line => {
                painter.line_segment(
                    [Pos2::new(lx, ly), Pos2::new(lx + sw, ly)],
                    Stroke::new(2.0 * scale, col(*color)),
                );
            }
            LegendMark::Points => {
                painter.circle_filled(Pos2::new(lx + sw * 0.5, ly), 3.0 * scale, col(*color));
            }
            LegendMark::LinePoints => {
                painter.line_segment(
                    [Pos2::new(lx, ly), Pos2::new(lx + sw, ly)],
                    Stroke::new(2.0 * scale, col(*color)),
                );
                painter.circle_filled(Pos2::new(lx + sw * 0.5, ly), 3.0 * scale, col(*color));
            }
            LegendMark::Rect { edge, hatch } => {
                let swatch = egui::Rect::from_min_size(
                    Pos2::new(lx, ly - 4.0 * scale),
                    Vec2::new(sw, 8.0 * scale),
                );
                painter.rect_filled(swatch, 1.0 * scale, col(*color));
                if let Some(hatch) = hatch {
                    let area = crate::Rect::new(
                        swatch.left(),
                        swatch.top(),
                        swatch.width(),
                        swatch.height(),
                    );
                    crate::hatch::paint(painter, &area.corners(), *hatch, area, scale);
                }
                if let Some((edge, width)) = edge {
                    painter.rect_stroke(
                        swatch,
                        1.0 * scale,
                        Stroke::new(width * scale, col(*edge)),
                        egui::StrokeKind::Middle,
                    );
                }
            }
        }
        crate::rich_text::paint(
            painter,
            Pos2::new(lx + sw + 5.0 * scale, ly),
            Align2::LEFT_CENTER,
            name,
            font_id.clone(),
            col(fig.typography.legend_color),
        );
    }
}
