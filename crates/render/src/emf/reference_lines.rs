use super::*;
use crate::curve_style::dashed_segments;
use plotium_figure::LineStyle;

pub(super) fn write(dc: &mut Dc, fig: &Figure, proj: &Projector<'_>, plot: Rect) {
    for item in crate::reference_lines::layout(fig, proj, plot, 1.0) {
        let line = item.line;
        if line.line_style == LineStyle::Solid {
            dc.line(item.start, item.end, line.color, line.width);
        } else {
            dc.segments(
                &dashed_segments(&[item.start, item.end], line.line_style, line.width, plot),
                line.color,
                line.width,
            );
        }
        if let Some(label) = item.label {
            dc.text(
                label.text,
                (label.x, label.top + label.size),
                TextStyle::new(
                    label.size,
                    line.color,
                    match label.anchor {
                        TextAnchor::Left => TA_LEFT,
                        TextAnchor::Center => TA_CENTER,
                        TextAnchor::Right => TA_RIGHT,
                    },
                ),
            );
        }
    }
}
