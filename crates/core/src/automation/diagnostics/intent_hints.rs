//! Hints that weigh the chart against the purpose its author stated. They
//! suggest chart forms Plotium can draw and never change the chart.
use super::*;
use crate::state::PlotPurpose;

/// Fold span at which side-by-side bars or a linear axis hide a ratio.
const FOLD_SPAN: f64 = 10.;

/// The chart features the purpose rules read.
struct ChartShape {
    bars: usize,
    distributions: bool,
    lines: bool,
    points: bool,
    categorical: bool,
    raw_samples: bool,
    bar_fold: Option<f64>,
    curve_fold: Option<f64>,
    log_value_axis: bool,
}

/// Ratio of the largest to the smallest value. Only all-positive values
/// have one: signed values such as log ratios already show the change.
fn fold(values: impl Iterator<Item = f64>) -> Option<f64> {
    let values: Vec<f64> = values.filter(|v| v.is_finite()).collect();
    let low = values.iter().copied().fold(f64::INFINITY, f64::min);
    let high = values.iter().copied().fold(0f64, f64::max);
    (values.len() >= 2 && low > 0.).then(|| high / low)
}

impl ChartShape {
    fn of(figure: &Figure, bars: &[PrintedBar], facts: &PlotFacts) -> Self {
        let value_axis = usize::from(!facts.transposed);
        let (category_axis, value) = if facts.transposed {
            (&figure.y, &figure.x)
        } else {
            (&figure.x, &figure.y)
        };
        let bar_series: BTreeSet<SeriesId> = bars.iter().map(|b| b.series).collect();
        // The right axis has its own scale; the rules weigh the main one.
        let curve_values = figure
            .series
            .iter()
            .filter(|s| s.appearance.y_axis == YAxisSide::Left)
            .filter(|s| {
                matches!(
                    facts.kind(s.origin),
                    Some(SeriesKind::Line | SeriesKind::Points)
                )
            })
            .flat_map(|s| s.points.iter().map(move |p| p[value_axis]));
        Self {
            bars: bar_series.len(),
            distributions: facts.has(SeriesKind::Box) || facts.has(SeriesKind::Violin),
            lines: facts.has(SeriesKind::Line),
            points: facts.has(SeriesKind::Points),
            categorical: category_axis.categories.is_some(),
            raw_samples: facts
                .rows_per_x
                .iter()
                .any(|(_, rows)| rows.iter().any(|n| *n > 1)),
            bar_fold: fold(bars.iter().map(|b| b.value)),
            curve_fold: fold(curve_values),
            log_value_axis: !value.scale.is_linear(),
        }
    }

    fn marks(&self) -> Vec<&'static str> {
        [
            (self.bars > 0, "bar"),
            (self.distributions, "box_or_violin"),
            (self.lines, "line"),
            (self.points, "scatter"),
        ]
        .into_iter()
        .filter_map(|(present, name)| present.then_some(name))
        .collect()
    }
}

const REDRAW: &str = "append the same columns with the new kind (plot.series.append), then delete the old Series (figure.edit series_delete) and restate its style, replicates and error bars";

/// Why the chart does not fit the purpose, and what would.
fn mismatch(
    purpose: PlotPurpose,
    shape: &ChartShape,
    target: &TargetRef,
    scale_property: &'static str,
) -> Option<(&'static str, Vec<Value>)> {
    let redraw =
        |kind: &str, reason: &str| suggest(&format!("Redraw as {kind}: {REDRAW}."), None, reason);
    // Nothing drawn: no chart form to weigh.
    if shape.marks().is_empty() {
        return None;
    }
    match purpose {
        PlotPurpose::Compare if shape.lines && shape.categorical => Some((
            "Lines between categories suggest a continuity the groups do not have.",
            vec![
                redraw("bars", "Bars compare separate groups at a glance."),
                redraw(
                    "a scatter",
                    "Points compare groups and stay readable when values differ widely.",
                ),
            ],
        )),
        PlotPurpose::Trend if shape.categorical => Some((
            "A trend needs a numeric x; categories have no distance between them.",
            vec![
                suggest(
                    "Bind a numeric x column (plot.series.bind).",
                    None,
                    "Distances along x then carry the trend.",
                ),
                suggest(
                    "If the point is a difference between groups, ask the user whether the purpose is compare.",
                    None,
                    "Categories suit comparisons.",
                ),
            ],
        )),
        PlotPurpose::Trend if shape.bars > 0 => Some((
            "Bars split a continuous trend into separate blocks.",
            vec![
                redraw("a line", "A line shows how the value changes along x."),
                redraw("a scatter", "Points keep each measured x visible."),
            ],
        )),
        // Overlaid points already show the spread, and on a numeric x bars or
        // lines may be a histogram or a density.
        PlotPurpose::Distribution if shape.categorical && !shape.distributions && !shape.points => {
            Some((
                "One value per group hides how the values spread.",
                if shape.raw_samples {
                    vec![
                        redraw(
                            "a box or violin without the y error column",
                            "A box or violin summarises the spread of the raw values.",
                        ),
                        suggest(
                            "Overlay the raw values as a scatter Series with series.points.spread.",
                            None,
                            "Each value stays visible next to the summary.",
                        ),
                    ]
                } else {
                    vec![
                        suggest(
                            "Add the raw values per group to the table, then draw a box or violin with the points overlaid.",
                            None,
                            "The spread needs the individual values.",
                        ),
                        suggest(
                            "If only means and errors exist, state the replicates and error type (series.replicates, series.error_bars).",
                            None,
                            "Readers then know what the error bars summarise.",
                        ),
                    ]
                },
            ))
        }
        PlotPurpose::Relationship if shape.categorical || shape.bars > 0 || shape.distributions => {
            Some((
                "A relationship needs paired numeric values on both axes.",
                vec![
                    suggest(
                        "Draw one quantity against the other as a scatter (plot.create kind scatter).",
                        None,
                        "Each pair becomes one point, so the relationship shows as a pattern.",
                    ),
                    suggest(
                        "Add a fitted line if a model describes the relationship.",
                        None,
                        "The fit states the relationship the points suggest.",
                    ),
                ],
            ))
        }
        PlotPurpose::Composition if shape.bars == 0 => Some((
            "Lines and points do not show parts adding up to a whole.",
            vec![suggest(
                "Ask the user to draw stacked bars in the desktop app: the table's grouped bar chart with Stack grouped bars on.",
                None,
                "Each bar is the whole and its segments are the parts.",
            )],
        )),
        PlotPurpose::Change
            if shape.bars >= 2 || shape.bar_fold.is_some_and(|f| f >= FOLD_SPAN) =>
        {
            Some((
                "Side-by-side bars make readers judge the ratio by eye.",
                vec![
                    suggest(
                        "Compute the ratio in a new column (data.transform) and plot it directly.",
                        None,
                        "The fold difference becomes the value readers see.",
                    ),
                    redraw(
                        "points on a log axis",
                        "Equal folds get equal distances on a log axis.",
                    ),
                ],
            ))
        }
        // Bars, boxes and violins cannot use a log axis.
        PlotPurpose::Change
            if shape.bars == 0
                && !shape.distributions
                && !shape.log_value_axis
                && shape.curve_fold.is_some_and(|f| f >= FOLD_SPAN) =>
        {
            Some((
                "On a linear axis equal folds look unequal.",
                vec![suggest(
                    "Use a log scale on the value axis.",
                    Some((scale_property, target)),
                    "Equal ratios get equal distances.",
                )],
            ))
        }
        _ => None,
    }
}

impl FigureDiagnosticScan {
    pub(super) fn intent_hints(
        &mut self,
        target: &TargetRef,
        figure: &Figure,
        bars: &[PrintedBar],
        facts: &PlotFacts,
    ) {
        let Some(purpose) = facts.intent.purpose else {
            self.coverage(
                target,
                "intent",
                "not_applicable",
                "no_purpose_stated: ask the user what the plot should show and set object.intent.purpose",
            );
            return;
        };
        // Table chart types draw marks without Series origins the rules read.
        if !facts.explicit {
            self.coverage(
                target,
                "intent",
                "not_applicable",
                "chart_type_not_covered: intent rules cover plots built from explicit columns",
            );
            return;
        }
        let shape = ChartShape::of(figure, bars, facts);
        let scale_property = if facts.transposed {
            "object.axes.x_scale"
        } else {
            "object.axes.y_scale"
        };
        let Some((reason, suggestions)) = mismatch(purpose, &shape, target, scale_property) else {
            return;
        };
        let claim = &facts.intent.claim;
        let evidence = json!({"interpretation":reason,"purpose":purpose.id(),"claim":(!claim.is_empty()).then_some(claim),
            "chart":{"marks":shape.marks(),"category_axis":shape.categorical,"bar_series":shape.bars,"raw_samples":shape.raw_samples,"bar_fold":shape.bar_fold,"curve_fold":shape.curve_fold},"threshold_fold":FOLD_SPAN});
        self.hint(target, "plot", "intent_mismatch", evidence, suggestions);
    }
}
