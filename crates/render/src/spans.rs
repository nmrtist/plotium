//! Output-space layout of author spans, shared by every backend. Span fills
//! are painted before every data layer, the heatmap included; their labels
//! are painted with the other annotations above the data, so a bar cannot
//! hide them.
use crate::{Projector, Rect, TextAnchor, clip_to_plot, range_label_layout};
use plotium_figure::{SpanAnnotation, SpanAxis};

pub(crate) struct SpanLabel {
    pub text: String,
    pub x: f32,
    pub top: f32,
    pub anchor: TextAnchor,
    pub size: f32,
}

pub(crate) struct SpanLayout<'a> {
    pub span: &'a SpanAnnotation,
    /// The visible part of the band, inside the plot rectangle.
    pub rect: Rect,
    pub label: Option<SpanLabel>,
}

/// Output position of a range end along the span's axis. An end at or below
/// zero on a log axis clips to the plot edge it lies beyond.
fn edge(proj: &Projector<'_>, axis: SpanAxis, value: f64) -> Option<f32> {
    let point = match axis {
        SpanAxis::X => [value, proj.fig.y.min],
        SpanAxis::Y => [proj.fig.x.min, value],
    };
    let (x, y) = clip_to_plot(proj.project(point), proj.plot)?;
    Some(match axis {
        SpanAxis::X => x,
        SpanAxis::Y => y,
    })
}

/// Lay out every span with a visible part. `scale` converts the label size
/// from pt to output units.
pub(crate) fn layout<'a>(proj: &Projector<'a>, scale: f32) -> Vec<SpanLayout<'a>> {
    let plot = proj.plot;
    let fig = proj.fig;
    fig.spans
        .iter()
        .filter_map(|span| {
            let a = edge(proj, span.axis, span.range[0])?;
            let b = edge(proj, span.axis, span.range[1])?;
            let rect = match span.axis {
                SpanAxis::X => {
                    let left = a.min(b).max(plot.left);
                    let right = a.max(b).min(plot.right());
                    (right > left).then(|| Rect::new(left, plot.top, right - left, plot.height))
                }
                SpanAxis::Y => {
                    let top = a.min(b).max(plot.top);
                    let bottom = a.max(b).min(plot.bottom());
                    (bottom > top).then(|| Rect::new(plot.left, top, plot.width, bottom - top))
                }
            }?;
            let size = span.label_size.unwrap_or(fig.typography.tick_pt) * scale;
            let label = (!span.label.trim().is_empty())
                .then(|| range_label_layout(plot, rect.left, rect.right(), size, &span.label, None))
                .flatten()
                .map(|placed| SpanLabel {
                    text: placed.text,
                    x: placed.x,
                    top: match span.axis {
                        SpanAxis::X => placed.top,
                        // Inside the band's top edge, kept within the plot.
                        SpanAxis::Y => (rect.top + 2.0 * scale)
                            .min(plot.bottom() - size)
                            .max(plot.top),
                    },
                    anchor: placed.anchor,
                    size,
                });
            Some(SpanLayout { span, rect, label })
        })
        .collect()
}

pub(crate) fn write_svg_fills(s: &mut String, proj: &Projector<'_>) {
    use std::fmt::Write as _;
    for item in layout(proj, 1.0) {
        let span = item.span;
        let _ = write!(
            s,
            r#"<rect class="span" x="{x:.2}" y="{y:.2}" width="{w:.2}" height="{h:.2}" fill="{color}" fill-opacity="{opacity:.3}"/>"#,
            x = item.rect.left,
            y = item.rect.top,
            w = item.rect.width,
            h = item.rect.height,
            color = span.color.to_hex(),
            opacity = span.opacity.clamp(0.0, 1.0),
        );
    }
}

pub(crate) fn write_svg_labels(s: &mut String, proj: &Projector<'_>) {
    use std::fmt::Write as _;
    for item in layout(proj, 1.0) {
        if let Some(label) = item.label {
            let _ = write!(
                s,
                r#"<text class="span-label" x="{x:.2}" y="{y:.2}" text-anchor="{anchor}" font-size="{size:.2}" fill="{color}">{text}</text>"#,
                x = label.x,
                y = label.top + label.size,
                anchor = match label.anchor {
                    TextAnchor::Left => "start",
                    TextAnchor::Center => "middle",
                    TextAnchor::Right => "end",
                },
                size = label.size,
                color = item.span.color.to_hex(),
                text = crate::rich_text::svg_content(&label.text, label.size),
            );
        }
    }
}

#[cfg(test)]
#[path = "spans_tests.rs"]
mod tests;
