use plotium_figure::{Color, Figure};

/// How a legend entry's swatch is drawn.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum LegendMark {
    Curve {
        kind: plotium_figure::SeriesKind,
        appearance: plotium_figure::CurveAppearance,
        width: f32,
    },
    Line,
    Points,
    LinePoints,
    /// A filled swatch, outlined like the shape it stands for.
    Rect {
        edge: Option<(Color, f32)>,
    },
}

impl LegendMark {
    fn merged(self, other: Self) -> Self {
        match (self, other) {
            (Self::Line, Self::Points)
            | (Self::Points, Self::Line)
            | (Self::LinePoints, Self::Line)
            | (Self::LinePoints, Self::Points)
            | (Self::Line, Self::LinePoints)
            | (Self::Points, Self::LinePoints) => Self::LinePoints,
            _ => self,
        }
    }
}

/// Legend entries across series and named polygons. Series with the same name
/// and color merge into one semantic entry, including a combined point+line
/// swatch for measured samples with a fitted curve.
pub(crate) fn legend_entries(fig: &Figure) -> Vec<(&str, Color, LegendMark)> {
    let mut entries: Vec<(&str, Color, LegendMark)> = Vec::new();
    for series in &fig.series {
        if series.points.is_empty() || series.name.is_empty() {
            continue;
        }
        let mark = if !matches!(
            series.kind,
            plotium_figure::SeriesKind::Stick | plotium_figure::SeriesKind::Bar
        ) && series.appearance != plotium_figure::CurveAppearance::default()
        {
            LegendMark::Curve {
                kind: series.kind,
                appearance: series.appearance,
                width: series.width,
            }
        } else {
            match series.kind {
                plotium_figure::SeriesKind::Line => LegendMark::Line,
                plotium_figure::SeriesKind::Points => LegendMark::Points,
                plotium_figure::SeriesKind::Stick => LegendMark::Line,
                plotium_figure::SeriesKind::Bar => LegendMark::Rect { edge: None },
            }
        };
        if let Some((_, _, existing)) = entries.iter_mut().find(|(name, color, existing)| {
            *name == series.name
                && *color == series.color
                && (!(matches!(mark, LegendMark::Curve { .. })
                    || matches!(existing, LegendMark::Curve { .. }))
                    || *existing == mark)
        }) {
            *existing = existing.merged(mark);
        } else {
            entries.push((series.name.as_str(), series.color, mark));
        }
    }
    for poly in &fig.polygons {
        if poly.name.is_empty()
            || entries
                .iter()
                .any(|(n, color, _)| *n == poly.name && *color == poly.fill)
        {
            continue;
        }
        // A background-colored stroke only separates stacked segments; on the
        // swatch it would be invisible or a stray rim.
        let edge = poly
            .stroke
            .filter(|(color, width)| *color != fig.background && *width > 0.0);
        entries.push((poly.name.as_str(), poly.fill, LegendMark::Rect { edge }));
    }
    entries
}
