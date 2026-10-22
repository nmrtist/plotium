use super::*;
use crate::arrow_geometry::arrow_shape;

/// Author arrows, mirroring [`crate::svg`]'s arrow output.
pub(super) fn write(dc: &mut Dc, proj: &Projector<'_>, fig: &Figure) {
    for arrow in &fig.author_arrows {
        let Some(shape) = arrow_shape(
            proj.project(arrow.from),
            proj.project(arrow.to),
            arrow.width,
            1.0,
            arrow.head,
        ) else {
            continue;
        };
        dc.line(shape.shaft[0], shape.shaft[1], arrow.color, arrow.width);
        for tip in shape.tips {
            if tip.filled {
                dc.polygon(&tip.points, Some(arrow.color), None);
            } else {
                dc.polyline(&tip.points, arrow.color, arrow.width);
            }
        }
    }
}
