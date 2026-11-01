use super::*;
use crate::{LegendMark, legend_entries, legend_entry_origin, placed_legend};

pub(super) fn write(dc: &mut Dc, fig: &Figure, plot: Rect, stack: Option<&crate::StackPlacement>) {
    let entries = legend_entries(fig);
    let font = fig.typography.legend_pt;
    let Some((layout, box_geometry)) = placed_legend(fig, plot, 1.0, stack) else {
        return;
    };
    let sw = layout.swatch;
    let dot = layout.dot_radius;
    // Legends draw neither a frame nor a backing.
    let (bx, by) = (box_geometry.left, box_geometry.top);
    if !fig.guide_title.trim().is_empty() {
        dc.text(
            &fig.guide_title,
            (bx + layout.padding, by + layout.padding + font),
            TextStyle::new(font, fig.typography.legend_color, TA_LEFT).bold(),
        );
    }
    for (i, (name, color, mark)) in entries.iter().enumerate() {
        let (ox, oy) = legend_entry_origin(&layout, i);
        let ly = by + oy;
        let lx = bx + ox;
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
                    curves::stroke(dc, &points, &series, plot);
                }
                curves::marker_at(
                    dc,
                    (lx + sw * 0.5, ly),
                    appearance.marker(*kind),
                    appearance.marker_radius(*width),
                    &crate::curve_style::MarkerPaint::new(*appearance, *color, *width),
                );
            }
            LegendMark::Line => dc.line((lx, ly), (lx + sw, ly), *color, 2.0),
            LegendMark::Points => dc.ellipse(
                Rect::new(lx + sw * 0.5 - dot, ly - dot, dot * 2.0, dot * 2.0),
                Some(*color),
                None,
            ),
            LegendMark::LinePoints => {
                dc.line((lx, ly), (lx + sw, ly), *color, 2.0);
                dc.ellipse(
                    Rect::new(lx + sw * 0.5 - dot, ly - dot, dot * 2.0, dot * 2.0),
                    Some(*color),
                    None,
                );
            }
            LegendMark::Rect { edge, hatch } => {
                let height = layout.rect_height;
                dc.swatch(
                    Rect::new(lx, ly - height * 0.5, sw, height),
                    *color,
                    *edge,
                    *hatch,
                )
            }
        }
        dc.text(
            name,
            (lx + sw + layout.text_gap, ly),
            TextStyle::new(font, fig.typography.legend_color, TA_LEFT).middle(),
        );
    }
}
