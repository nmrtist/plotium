use crate::Projector;
use crate::arrow_geometry::arrow_shape;
use plotium_figure::Figure;
use std::fmt::Write as _;

/// Author arrows; round caps and joins match the EMF pen.
pub(super) fn write(output: &mut String, projector: &Projector<'_>, figure: &Figure) {
    for arrow in &figure.author_arrows {
        let Some(shape) = arrow_shape(
            projector.project(arrow.from),
            projector.project(arrow.to),
            arrow.width,
            1.0,
            arrow.head,
        ) else {
            continue;
        };
        let [(ax, ay), (bx, by)] = shape.shaft;
        let _ = write!(
            output,
            r#"<g class="annotation-arrow" stroke="{color}" fill="{color}" stroke-width="{width}" stroke-linecap="round" stroke-linejoin="round"><path d="M{ax:.2} {ay:.2}L{bx:.2} {by:.2}" fill="none"/>"#,
            color = arrow.color.to_hex(),
            width = arrow.width,
        );
        for tip in shape.tips {
            let [(ax, ay), (tx, ty), (bx, by)] = tip.points;
            let (close, paint) = if tip.filled {
                ("Z", r#"stroke="none""#)
            } else {
                ("", r#"fill="none""#)
            };
            let _ = write!(
                output,
                r#"<path d="M{ax:.2} {ay:.2}L{tx:.2} {ty:.2}L{bx:.2} {by:.2}{close}" {paint}/>"#,
            );
        }
        output.push_str("</g>");
    }
}
