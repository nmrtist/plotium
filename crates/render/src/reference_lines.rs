//! Output-space layout of author reference lines, shared by every backend.
use crate::curve_style::dash_pattern;
use crate::label_placement::line_position;
use crate::{LabelLayout, Rect, ReferenceLabelBox, TextAnchor};
use plotium_figure::{Figure, LineStyle, ReferenceLine, ReferenceOrientation};

pub(crate) struct ReferenceLineLayout<'a> {
    pub line: &'a ReferenceLine,
    pub start: (f32, f32),
    pub end: (f32, f32),
    pub label: Option<ReferenceLabelBox>,
}

/// Lay out every reference line whose value falls inside the plot rectangle,
/// with its label where `labels` placed it. A line outside the visible range
/// has no visible part, so it and its label are dropped instead of relying on
/// each backend's clip.
pub(crate) fn layout<'a>(
    fig: &'a Figure,
    plot: Rect,
    labels: &LabelLayout,
) -> Vec<ReferenceLineLayout<'a>> {
    fig.reference_lines
        .iter()
        .zip(&labels.reference)
        .filter_map(|(line, label)| {
            let at = line_position(fig, plot, line)?;
            let (start, end) = match line.orientation {
                ReferenceOrientation::Vertical => ((at, plot.bottom()), (at, plot.top)),
                ReferenceOrientation::Horizontal => ((plot.left, at), (plot.right(), at)),
            };
            Some(ReferenceLineLayout {
                line,
                start,
                end,
                label: *label,
            })
        })
        .collect()
}

/// SVG `text-anchor` for a reference label.
fn svg_anchor(anchor: TextAnchor) -> &'static str {
    match anchor {
        TextAnchor::Left => "start",
        TextAnchor::Center => "middle",
        TextAnchor::Right => "end",
    }
}

pub(crate) fn write_svg(s: &mut String, fig: &Figure, plot: Rect, labels: &LabelLayout) {
    use std::fmt::Write as _;
    for item in layout(fig, plot, labels) {
        let line = item.line;
        let dash = if line.line_style == LineStyle::Solid {
            String::new()
        } else {
            format!(
                r#" stroke-dasharray="{}""#,
                dash_pattern(line.line_style, line.width)
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        let _ = write!(
            s,
            r#"<line class="reference-line" x1="{x1:.2}" y1="{y1:.2}" x2="{x2:.2}" y2="{y2:.2}" stroke="{color}" stroke-width="{width}"{dash}/>"#,
            x1 = item.start.0,
            y1 = item.start.1,
            x2 = item.end.0,
            y2 = item.end.1,
            color = line.color.to_hex(),
            width = line.width,
        );
        if let Some(label) = item.label {
            let _ = write!(
                s,
                r#"<text class="reference-line-label" x="{x:.2}" y="{y:.2}" text-anchor="{anchor}" font-size="{size:.2}" fill="{color}">{text}</text>"#,
                x = label.x,
                y = label.top + label.size,
                anchor = svg_anchor(label.anchor),
                size = label.size,
                color = line.color.to_hex(),
                text = crate::rich_text::svg_content(&line.label, label.size),
            );
        }
    }
}

#[cfg(test)]
#[path = "reference_lines_tests.rs"]
mod tests;
