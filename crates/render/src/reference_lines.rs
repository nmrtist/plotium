//! Output-space layout of author reference lines, shared by every backend.
use crate::curve_style::dash_pattern;
use crate::{Projector, Rect, TextAnchor};
use plotium_figure::{Figure, LineStyle, ReferenceLine, ReferenceOrientation};

/// Gap between a reference line and its label, in output units at scale 1.
const LABEL_GAP: f32 = 2.0;

pub(crate) struct ReferenceLabel<'a> {
    pub text: &'a str,
    pub x: f32,
    pub top: f32,
    pub size: f32,
    pub anchor: TextAnchor,
}

pub(crate) struct ReferenceLineLayout<'a> {
    pub line: &'a ReferenceLine,
    pub start: (f32, f32),
    pub end: (f32, f32),
    pub label: Option<ReferenceLabel<'a>>,
}

/// Lay out every reference line whose value falls inside the plot rectangle.
/// A line outside the visible range has no visible part, so it and its label
/// are dropped instead of relying on each backend's clip.
pub(crate) fn layout<'a>(
    fig: &'a Figure,
    proj: &Projector<'_>,
    plot: Rect,
    scale: f32,
) -> Vec<ReferenceLineLayout<'a>> {
    let gap = LABEL_GAP * scale;
    let tick_pt = fig.typography.tick_pt;
    let vertical = |value: f64| proj.project([value, fig.y.min]).0;
    let horizontal = |value: f64| proj.project([fig.x.min, value]).1;
    fig.reference_lines
        .iter()
        .filter_map(|line| {
            if !line.value.is_finite() {
                return None;
            }
            let size = line.label_size.unwrap_or(tick_pt) * scale;
            let has_label = !line.label.trim().is_empty();
            match line.orientation {
                ReferenceOrientation::Vertical => {
                    let x = vertical(line.value);
                    if !x.is_finite() || x < plot.left || x > plot.right() {
                        return None;
                    }
                    // Keep the label on the roomier side so it stays inside the clip.
                    let right_half = x > plot.left + plot.width * 0.5;
                    let label = has_label.then(|| ReferenceLabel {
                        text: &line.label,
                        x: if right_half { x - gap } else { x + gap },
                        top: plot.top + gap,
                        size,
                        anchor: if right_half {
                            TextAnchor::Right
                        } else {
                            TextAnchor::Left
                        },
                    });
                    Some(ReferenceLineLayout {
                        line,
                        start: (x, plot.bottom()),
                        end: (x, plot.top),
                        label,
                    })
                }
                ReferenceOrientation::Horizontal => {
                    let y = horizontal(line.value);
                    if !y.is_finite() || y < plot.top || y > plot.bottom() {
                        return None;
                    }
                    // Above the line unless that would leave the plot top.
                    let above = y - gap - size >= plot.top;
                    let label = has_label.then(|| ReferenceLabel {
                        text: &line.label,
                        x: plot.right() - gap,
                        top: if above { y - gap - size } else { y + gap },
                        size,
                        anchor: TextAnchor::Right,
                    });
                    Some(ReferenceLineLayout {
                        line,
                        start: (plot.left, y),
                        end: (plot.right(), y),
                        label,
                    })
                }
            }
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

pub(crate) fn write_svg(s: &mut String, fig: &Figure, proj: &Projector<'_>, plot: Rect) {
    use std::fmt::Write as _;
    for item in layout(fig, proj, plot, 1.0) {
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
                text = crate::rich_text::svg_content(label.text, label.size),
            );
        }
    }
}

#[cfg(test)]
#[path = "reference_lines_tests.rs"]
mod tests;
