use crate::{LegendMark, Rect, StackPlacement, legend_entries, legend_entry_origin, placed_legend};
use plotium_figure::Figure;
use std::fmt::Write;

pub(super) fn write(s: &mut String, fig: &Figure, plot: Rect, stack: Option<&StackPlacement>) {
    let entries = legend_entries(fig);
    let font = fig.typography.legend_pt;
    let Some((layout, box_geometry)) = placed_legend(fig, plot, 1.0, stack) else {
        return;
    };
    let sw = layout.swatch;
    // Legends draw neither a frame nor a backing.
    let (bx, by) = (box_geometry.left, box_geometry.top);
    if !fig.guide_title.trim().is_empty() {
        let _ = write!(
            s,
            r#"<text x="{x:.2}" y="{y:.2}" font-size="{font}" font-weight="bold" fill="{axis}">{title}</text>"#,
            x = bx + layout.padding,
            y = by + layout.padding + font * crate::rich_text::TOP_TO_BASELINE,
            axis = fig.typography.legend_color.to_hex(),
            title = crate::rich_text::svg_content(&fig.guide_title, font),
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
                    super::curves::stroke(s, &points, &series);
                }
                super::curves::marker_at(
                    s,
                    (lx + sw * 0.5, ly),
                    appearance.marker(*kind),
                    appearance.marker_radius(*width),
                    &crate::curve_style::MarkerPaint::new(*appearance, *color, *width),
                );
            }

            LegendMark::Line => {
                let _ = write!(
                    s,
                    r#"<line x1="{lx:.2}" y1="{ly:.2}" x2="{x2:.2}" y2="{ly:.2}" stroke="{col}" stroke-width="2"/>"#,
                    x2 = lx + sw,
                    col = color.to_hex(),
                );
            }
            LegendMark::Points => {
                let _ = write!(
                    s,
                    r#"<circle cx="{cx:.2}" cy="{ly:.2}" r="{r:.2}" fill="{col}"/>"#,
                    cx = lx + sw * 0.5,
                    r = layout.dot_radius,
                    col = color.to_hex(),
                );
            }
            LegendMark::LinePoints => {
                let _ = write!(
                    s,
                    r#"<line x1="{lx:.2}" y1="{ly:.2}" x2="{x2:.2}" y2="{ly:.2}" stroke="{col}" stroke-width="2"/><circle cx="{cx:.2}" cy="{ly:.2}" r="{r:.2}" fill="{col}"/>"#,
                    x2 = lx + sw,
                    cx = lx + sw * 0.5,
                    r = layout.dot_radius,
                    col = color.to_hex(),
                );
            }
            LegendMark::Rect { edge, hatch } => {
                let stroke = edge.map_or(String::new(), |(edge, width)| {
                    format!(r#" stroke="{}" stroke-width="{width}""#, edge.to_hex())
                });
                let _ = write!(
                    s,
                    r#"<rect x="{lx:.2}" y="{y:.2}" width="{sw:.2}" height="{height:.2}" rx="1" fill="{col}"{stroke}/>"#,
                    y = ly - layout.rect_height * 0.5,
                    height = layout.rect_height,
                    col = color.to_hex(),
                );
                if let Some(hatch) = hatch {
                    let height = layout.rect_height;
                    let swatch = crate::Rect::new(lx, ly - height * 0.5, sw, height);
                    crate::hatch::write_svg(s, &swatch.corners(), *hatch, swatch);
                }
            }
        }
        let _ = write!(
            s,
            r#"<text x="{tx:.2}" y="{y:.2}" font-size="{font}" fill="{axis}">{txt}</text>"#,
            tx = lx + sw + layout.text_gap,
            y = ly + font * crate::rich_text::MIDDLE_TO_BASELINE,
            axis = fig.typography.legend_color.to_hex(),
            txt = crate::rich_text::svg_content(name, font),
        );
    }
}
