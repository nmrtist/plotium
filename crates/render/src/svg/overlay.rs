use crate::rich_text::svg_content;
use crate::{DocumentOverlay, OverlayAlign, OverlayKind, OverlayShapeKind, arrow_head};
use std::fmt::Write as _;

pub(super) fn write_overlay(s: &mut String, overlay: &DocumentOverlay<'_>) {
    let f = overlay.frame;
    match &overlay.kind {
        OverlayKind::Text(t) => {
            if t.text.trim().is_empty() {
                return;
            }
            let (x, anchor) = match t.align {
                OverlayAlign::Left => (f.left, "start"),
                OverlayAlign::Center => (f.left + f.width * 0.5, "middle"),
                OverlayAlign::Right => (f.right(), "end"),
            };
            let weight = if t.bold { "bold" } else { "normal" };
            let _ = write!(
                s,
                r#"<text x="{x:.2}" y="{y:.2}" text-anchor="{anchor}" font-size="{sz:.2}" font-weight="{weight}" fill="{col}">"#,
                y = f.top + t.font_size,
                sz = t.font_size,
                col = t.color.to_hex(),
            );
            // Absolute line baselines, because a line may end in a script.
            for (i, line) in t.text.lines().enumerate() {
                let line_y = f.top + t.font_size * (1.0 + 1.25 * i as f32);
                let _ = write!(
                    s,
                    r#"<tspan x="{x:.2}" y="{line_y:.2}">{}</tspan>"#,
                    svg_content(line, t.font_size)
                );
            }
            let _ = write!(s, "</text>");
        }
        OverlayKind::Shape(sh) => {
            let fill = sh
                .fill
                .map(|c| c.to_hex())
                .unwrap_or_else(|| "none".to_owned());
            let stroke = sh.stroke.to_hex();
            let sw = sh.stroke_width.max(0.5);
            match sh.shape {
                OverlayShapeKind::Rect => {
                    let _ = write!(
                        s,
                        r#"<rect x="{l:.2}" y="{t:.2}" width="{w:.2}" height="{h:.2}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}"/>"#,
                        l = f.left,
                        t = f.top,
                        w = f.width,
                        h = f.height,
                    );
                }
                OverlayShapeKind::Ellipse => {
                    let _ = write!(
                        s,
                        r#"<ellipse cx="{cx:.2}" cy="{cy:.2}" rx="{rx:.2}" ry="{ry:.2}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}"/>"#,
                        cx = f.left + f.width * 0.5,
                        cy = f.top + f.height * 0.5,
                        rx = f.width * 0.5,
                        ry = f.height * 0.5,
                    );
                }
                OverlayShapeKind::Line => {
                    let _ = write!(
                        s,
                        r#"<line x1="{x1:.2}" y1="{y1:.2}" x2="{x2:.2}" y2="{y2:.2}" stroke="{stroke}" stroke-width="{sw}"/>"#,
                        x1 = f.left,
                        y1 = f.top,
                        x2 = f.right(),
                        y2 = f.bottom(),
                    );
                }
                OverlayShapeKind::Arrow => {
                    let origin = (f.left, f.top);
                    let tip = (f.right(), f.bottom());
                    let [b1, b2] = arrow_head(origin, tip, 1.0);
                    let _ = write!(
                        s,
                        r#"<path d="M{ox:.2} {oy:.2}L{tx:.2} {ty:.2}M{tx:.2} {ty:.2}L{b1x:.2} {b1y:.2}M{tx:.2} {ty:.2}L{b2x:.2} {b2y:.2}" fill="none" stroke="{stroke}" stroke-width="{sw}"/>"#,
                        ox = origin.0,
                        oy = origin.1,
                        tx = tip.0,
                        ty = tip.1,
                        b1x = b1.0,
                        b1y = b1.1,
                        b2x = b2.0,
                        b2y = b2.1,
                    );
                }
            }
        }
    }
}
