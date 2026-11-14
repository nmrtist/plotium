use super::*;

/// Author span fills, mirroring [`crate::svg`]'s span output. GDI has no
/// per-primitive alpha, so the fill is pre-blended against the background and
/// the caller paints fills before the grid, which stays visible through them.
pub(super) fn write_fills(dc: &mut Dc, proj: &Projector<'_>) {
    for item in crate::spans::layout(proj, 1.0) {
        let span = item.span;
        let fill = blend(
            span.color,
            span.opacity.clamp(0.0, 1.0),
            proj.fig.background,
        );
        dc.rect(item.rect, Some(fill), None);
    }
}

/// Author span labels, painted above the data.
pub(super) fn write_labels(dc: &mut Dc, proj: &Projector<'_>) {
    for item in crate::spans::layout(proj, 1.0) {
        let Some(label) = item.label else {
            continue;
        };
        dc.text(
            &label.text,
            (label.x, label.top + label.size),
            TextStyle::new(
                label.size,
                item.span.color,
                match label.anchor {
                    TextAnchor::Left => TA_LEFT,
                    TextAnchor::Center => TA_CENTER,
                    TextAnchor::Right => TA_RIGHT,
                },
            ),
        );
    }
}
