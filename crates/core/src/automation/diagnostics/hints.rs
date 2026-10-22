//! Advisory hints: the figure could show its data more clearly. Hints never
//! edit the document and are never failures.
use super::*;
use plotium_figure::{Figure, SeriesEncoding, SeriesKind, SeriesOrigin, YAxisSide};
use plotium_render::Projector;
use plotium_render::diagnostics::TextBox;

// Provisional thresholds, measured at the requested output size.
const MM_PER_PT: f32 = 25.4 / 72.;
const SMALL_VALUE_FRACTION: f64 = 0.01;
const SMALL_BAR_MM: f32 = 0.5;
const NARROW_BAR_MM: f32 = 0.6;
const FEW_SAMPLES: usize = 10;
const LOG_SPAN: f64 = 1000.;
const MISALIGN_MM: f32 = 0.5;
/// Overlaps thinner than this are anti-aliasing contact, not collisions.
const OVERLAP_MM: f32 = 0.2;
/// Hint rows of one pairwise kind per scan; the rest are counted.
const MAX_PAIRS: usize = 50;
const MAX_TEXTS: usize = 4096;
/// Curve points visited per plot when checking text over data.
const TEXT_SCAN_POINTS: usize = 100_000;
/// Frames whose edges differ by less than this share a row or column.
const SHARED_EDGE_MM: f32 = 1.0;

/// What diagnostics need to know about a plot's native curves.
#[derive(Default)]
pub(super) struct PlotFacts {
    transposed: bool,
    /// Kinds of the visible native Series.
    kinds: Vec<(SeriesId, SeriesKind)>,
    /// Table rows at each x of visible bar, box and violin Series.
    rows_per_x: Vec<(SeriesId, Vec<usize>)>,
    /// Series whose rows could not be read, with the reason.
    unreadable: Vec<(SeriesId, String)>,
    /// Series a narrowed query asked about; `None` reports every Series.
    selected: Option<BTreeSet<SeriesId>>,
}

impl PlotFacts {
    pub(super) fn of(app: &PlotiumApp, binding: &DataBinding, data_shape: bool) -> Self {
        let mut facts = Self {
            transposed: binding.coordinates.as_ref().is_some_and(|c| c.transposed),
            ..Self::default()
        };
        for series in binding.series.iter().filter(|s| s.visible) {
            let SeriesEncoding::Line(line) = &series.encoding else {
                continue;
            };
            facts.kinds.push((series.id, line.kind));
            if data_shape && is_mark(line.kind) {
                match app.curve_rows_per_x(binding, series) {
                    Ok(sizes) => facts.rows_per_x.push((series.id, sizes)),
                    Err(reason) => facts.unreadable.push((series.id, reason)),
                }
            }
        }
        facts
    }

    pub(super) fn select(&mut self, selected: Option<BTreeSet<SeriesId>>) {
        self.selected = selected;
    }

    fn reports(&self, id: SeriesId) -> bool {
        self.selected.as_ref().is_none_or(|s| s.contains(&id))
    }

    fn kind(&self, origin: Option<SeriesOrigin>) -> Option<SeriesKind> {
        let id = SeriesId::new(origin?.0);
        self.kinds.iter().find(|(s, _)| *s == id).map(|(_, k)| *k)
    }

    fn has(&self, kind: SeriesKind) -> bool {
        self.kinds.iter().any(|(_, k)| *k == kind)
    }
}

fn is_mark(kind: SeriesKind) -> bool {
    matches!(kind, SeriesKind::Bar | SeriesKind::Box | SeriesKind::Violin)
}

/// A suggested action; `property`, when given, applies to `target`.
fn suggest(action: &str, property: Option<(&str, &TargetRef)>, reason: &str) -> Value {
    json!({"action":action,"property":property.map(|p| p.0),"target":property.map(|p| p.1),"reason":reason})
}

fn message(code: &str) -> &'static str {
    match code {
        "small_values_invisible" => {
            "Some bars are too short to see beside the others, so small values read as zero."
        }
        "bars_too_narrow" => "Bars print so narrow at this output size that they read as lines.",
        "few_samples_hidden" => {
            "Each group has only a few values, but the chart shows a summary without them."
        }
        "too_many_categories" => {
            "Category labels do not fit along the axis: some are dropped or collide."
        }
        "log_scale_candidate" => {
            "Positive values span three or more decades on a linear axis, which squeezes most of them near zero."
        }
        "bar_baseline_not_zero" => {
            "Bar lengths encode values from their baseline, but the value axis leaves it out, which exaggerates differences."
        }
        "labels_overlap" => "Two text boxes overlap.",
        "text_over_data" => "Text is drawn over data marks.",
        "panels_misaligned" => {
            "Plot areas of neighbouring panels are not aligned or evenly spaced."
        }
        _ => "",
    }
}

fn series_target(plot: &TargetRef, id: SeriesId) -> TargetRef {
    TargetRef {
        resource: plot.resource.clone(),
        component: Some(ComponentRef::Series(id)),
    }
}

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// One bar as printed: its value from the baseline, and its projected size.
struct PrintedBar {
    series: SeriesId,
    value: f64,
    baseline: f64,
    length: f32,
    thickness: f32,
}

impl FigureDiagnosticScan {
    fn mm(&self, pt: f32) -> f32 {
        pt * self.scale * MM_PER_PT
    }

    fn hint(
        &mut self,
        target: &TargetRef,
        part: &str,
        code: &str,
        evidence: Value,
        suggestions: Vec<Value>,
    ) {
        self.emit(json!({"kind":"issue","target":target,"component":part,"code":code,"severity":"hint","message":message(code),"measurement_basis":"renderer_projected_marks","evidence":evidence,"suggestions":suggestions}));
    }

    /// Records a page-space text box for the canvas-wide overlap hint.
    pub(super) fn note_text(&mut self, target: &TargetRef, part: &str, bounds: Rect) {
        if !self.checks(DiagnosticCheck::LayoutAesthetics) || !capture::finite(bounds) {
            return;
        }
        if self.hint_texts.len() < MAX_TEXTS {
            self.hint_texts
                .push((target.clone(), part.to_owned(), bounds));
        } else {
            self.texts_dropped += 1;
        }
    }

    /// Data-shape and per-plot layout hints. `plot` and `texts` are frame-local.
    pub(super) fn plot_hints(
        &mut self,
        target: &TargetRef,
        frame: Rect,
        figure: &Figure,
        plot: Rect,
        (texts, annotations): (&[TextBox], &[TextBox]),
        facts: &PlotFacts,
    ) {
        let proj = Projector {
            fig: figure,
            plot,
            top_band: None,
            left_band: None,
            y_side: YAxisSide::Left,
        };
        // Rectangle corners run along the category axis first, then the value axis.
        let value_axis = usize::from(!facts.transposed);
        let bars: Vec<PrintedBar> = figure
            .polygons
            .iter()
            .filter(|p| facts.kind(p.origin) == Some(SeriesKind::Bar) && p.points.len() == 4)
            .filter_map(|p| {
                let [a, b, c] = [0, 1, 2].map(|i| proj.project(p.points[i]));
                Some(PrintedBar {
                    series: SeriesId::new(p.origin?.0),
                    value: p.points[2][value_axis] - p.points[0][value_axis],
                    baseline: p.points[0][value_axis],
                    length: distance(b, c),
                    thickness: distance(a, b),
                })
            })
            .filter(|b| b.value.is_finite() && b.length.is_finite() && b.thickness.is_finite())
            .filter(|b| facts.reports(b.series))
            .collect();
        if self.checks(DiagnosticCheck::DataShape) {
            for (id, reason) in &facts.unreadable {
                if facts.reports(*id) {
                    let reason = format!("few_samples_hidden: rows unreadable: {reason}");
                    self.coverage(
                        &series_target(target, *id),
                        "data_shape",
                        "unavailable",
                        &reason,
                    );
                }
            }
            self.bar_hints(target, figure, &bars, facts);
            self.sample_hints(target, facts);
            self.category_hints(target, figure, texts, facts);
            self.log_hints(target, figure, facts);
        }
        if self.checks(DiagnosticCheck::LayoutAesthetics) {
            let page = |r: Rect| geometry::translate(r, frame.left, frame.top);
            for t in texts.iter().chain(annotations) {
                self.note_text(target, &t.component, page(t.bounds));
            }
            self.hint_plots.push((target.clone(), frame, page(plot)));
            self.text_over_data(target, &proj, figure, annotations, facts, page);
        }
    }

    fn bar_hints(
        &mut self,
        target: &TargetRef,
        figure: &Figure,
        bars: &[PrintedBar],
        facts: &PlotFacts,
    ) {
        let Some(longest) = bars.iter().map(|b| b.length).reduce(f32::max) else {
            return;
        };
        let axis = if facts.transposed {
            &figure.x
        } else {
            &figure.y
        };
        let (axis_name, break_property, range_property) = if facts.transposed {
            ("x", "object.axes.x_break", "object.axes.x_range")
        } else {
            ("y", "object.axes.y_break", "object.axes.y_range")
        };
        let span = (axis.max - axis.min).abs();
        // Small bars matter only beside bars that are clearly visible.
        if self.mm(longest) >= 10. * SMALL_BAR_MM {
            let series: BTreeSet<SeriesId> = bars.iter().map(|b| b.series).collect();
            for id in series {
                let small: Vec<&PrintedBar> = bars
                    .iter()
                    .filter(|b| b.series == id && b.value != 0.)
                    .filter(|b| {
                        b.value.abs() < SMALL_VALUE_FRACTION * span
                            || self.mm(b.length) < SMALL_BAR_MM
                    })
                    .collect();
                let Some(shortest) = small.iter().map(|b| b.length).reduce(f32::min) else {
                    continue;
                };
                let smallest = small
                    .iter()
                    .map(|b| b.value.abs())
                    .fold(f64::INFINITY, f64::min);
                let largest = bars.iter().map(|b| b.value.abs()).fold(0., f64::max);
                let evidence = json!({"small_bars":small.len(),"smallest_value":smallest,"largest_value":largest,"shortest_bar_mm":self.mm(shortest),"value_axis":axis_name,"thresholds":{"value_fraction_of_axis":SMALL_VALUE_FRACTION,"bar_length_mm":SMALL_BAR_MM}});
                let series = series_target(target, id);
                self.hint(&series, "series.bar", "small_values_invisible", evidence, vec![
                    suggest("Write each bar's value next to it.", Some(("series.value_labels", &series)), "Readers get the small values without changing the scale."),
                    suggest("Break the value axis above the small bars.", Some((break_property, target)), "Both the small and the large bars keep a readable length."),
                    suggest("Draw the values as points with error bars on a log axis.", None, "Bars cannot use a log axis; points can, and a log axis shows fold differences."),
                ]);
            }
        }
        let thinnest = bars
            .iter()
            .map(|b| b.thickness)
            .fold(f32::INFINITY, f32::min);
        if self.mm(thinnest) < NARROW_BAR_MM {
            let categorical = if facts.transposed {
                &figure.y
            } else {
                &figure.x
            }
            .categories
            .is_some();
            let mut suggestions = Vec::new();
            if categorical && !facts.transposed {
                suggestions.push(suggest(
                    "Turn the chart horizontal.",
                    Some(("object.chart.orientation", target)),
                    "Categories then stack down the page, where there is room for wider bars.",
                ));
            }
            suggestions.push(suggest(
                "Show fewer categories or split them into panels.",
                None,
                "Each remaining bar gets more width.",
            ));
            suggestions.push(suggest(
                "Draw the values as points or a line.",
                None,
                "Points stay readable at any spacing.",
            ));
            self.hint(target, "plot.bars", "bars_too_narrow", json!({"bars":bars.len(),"narrowest_bar_mm":self.mm(thinnest),"threshold_mm":NARROW_BAR_MM}), suggestions);
        }
        let (low, high) = (axis.min.min(axis.max), axis.min.max(axis.max));
        let cut: Vec<f64> = bars
            .iter()
            .map(|b| b.baseline)
            .filter(|v| *v < low || *v > high)
            .collect();
        if let Some(baseline) = cut.first() {
            self.hint(target, "plot.bars", "bar_baseline_not_zero", json!({"bars_cut":cut.len(),"baseline":baseline,"axis_range":[axis.min,axis.max],"value_axis":axis_name}), vec![
                suggest("Extend the value axis to the baseline.", Some((range_property, target)), "Bar lengths then stay proportional to the values."),
                suggest("Draw the values as points.", None, "Points do not encode a length, so the axis may start anywhere."),
            ]);
        }
    }

    fn sample_hints(&mut self, target: &TargetRef, facts: &PlotFacts) {
        // Overlaid points already show the individual values.
        if facts.has(SeriesKind::Points) {
            return;
        }
        for (id, sizes) in facts.rows_per_x.iter().filter(|(id, _)| facts.reports(*id)) {
            let kind = facts.kind(Some(SeriesOrigin(id.get())));
            let (Some(&fewest), Some(&most)) = (sizes.iter().min(), sizes.iter().max()) else {
                continue;
            };
            // One row per bar is a summary table; its raw values are unknown.
            if most >= FEW_SAMPLES || (kind == Some(SeriesKind::Bar) && most < 2) {
                continue;
            }
            self.hint(&series_target(target, *id), "series.mark", "few_samples_hidden", json!({"groups":sizes.len(),"fewest_values":fewest,"most_values":most,"threshold":FEW_SAMPLES}), vec![
                suggest("Append a scatter Series of the same columns, then set series.points.spread on it.", None, "Each value shows next to its group's summary."),
                suggest("Use a dot plot with mean and error bars instead.", None, "With few values per group the values themselves are the clearest summary."),
            ]);
        }
    }

    fn category_hints(
        &mut self,
        target: &TargetRef,
        figure: &Figure,
        texts: &[TextBox],
        facts: &PlotFacts,
    ) {
        let (axis, prefix) = if facts.transposed {
            (&figure.y, "axis.y.tick:")
        } else {
            (&figure.x, "axis.x.tick:")
        };
        let Some(categories) = &axis.categories else {
            return;
        };
        let (low, high) = (axis.min.min(axis.max), axis.min.max(axis.max));
        let shown = categories
            .iter()
            .enumerate()
            .filter(|(i, name)| (low..=high).contains(&(*i as f64)) && !name.trim().is_empty())
            .count();
        let mut labels: Vec<Rect> = texts
            .iter()
            .filter(|t| t.component.starts_with(prefix))
            .map(|t| t.bounds)
            .collect();
        // No labels at all means the axis hides them, not that they do not fit.
        if labels.is_empty() {
            return;
        }
        labels.sort_by(|a, b| a.left.total_cmp(&b.left).then(a.top.total_cmp(&b.top)));
        let collisions = labels
            .windows(2)
            .filter(|w| self.overlaps(w[0], w[1]).is_some())
            .count();
        if labels.len() >= shown && collisions == 0 {
            return;
        }
        let mut suggestions = Vec::new();
        if !facts.transposed {
            suggestions.push(suggest(
                "Turn the chart horizontal.",
                Some(("object.chart.orientation", target)),
                "Horizontal category labels have a full line each.",
            ));
        }
        suggestions.push(suggest(
            "Shorten the category names or sort and group them.",
            None,
            "Shorter labels fit at the tick spacing.",
        ));
        suggestions.push(suggest(
            "Use a dot plot or split the categories into panels.",
            None,
            "Fewer labels per axis fit without collisions.",
        ));
        self.hint(
            target,
            if facts.transposed { "axis.y" } else { "axis.x" },
            "too_many_categories",
            json!({"categories":shown,"labelled":labels.len(),"colliding_neighbours":collisions}),
            suggestions,
        );
    }

    fn log_hints(&mut self, target: &TargetRef, figure: &Figure, facts: &PlotFacts) {
        if facts.kinds.iter().any(|(_, k)| is_mark(*k)) {
            return;
        }
        let value_axis = usize::from(!facts.transposed);
        for side in [YAxisSide::Left, YAxisSide::Right] {
            let (axis, part, property) = match (facts.transposed, side) {
                (true, YAxisSide::Left) => (Some(&figure.x), "axis.x", "object.axes.x_scale"),
                (true, YAxisSide::Right) => (None, "", ""),
                (false, YAxisSide::Left) => (Some(&figure.y), "axis.y", "object.axes.y_scale"),
                (false, YAxisSide::Right) => {
                    (figure.y2.as_ref(), "axis.y2", "object.axes.y2_scale")
                }
            };
            if !axis.is_some_and(|a| a.scale.is_linear()) {
                continue;
            }
            let values: Vec<f64> = figure
                .series
                .iter()
                .filter(|s| s.appearance.y_axis == side)
                .filter(|s| {
                    matches!(
                        facts.kind(s.origin),
                        Some(SeriesKind::Line | SeriesKind::Points)
                    )
                })
                .flat_map(|s| s.points.iter().map(|p| p[value_axis]))
                .filter(|v| v.is_finite())
                .collect();
            let (low, high) = values
                .iter()
                .fold((f64::INFINITY, 0f64), |(l, h), v| (l.min(*v), h.max(*v)));
            if values.len() < 2 || low <= 0. || high / low < LOG_SPAN {
                continue;
            }
            self.hint(target, part, "log_scale_candidate", json!({"smallest":low,"largest":high,"decades":(high / low).log10(),"threshold_decades":LOG_SPAN.log10()}), vec![
                suggest("Use a log scale on this axis.", Some((property, target)), "Equal ratios get equal distances, so every decade is readable."),
            ]);
        }
    }

    fn overlaps(&self, a: Rect, b: Rect) -> Option<Rect> {
        geometry::intersection(a, b)
            .filter(|r| self.mm(r.width) > OVERLAP_MM && self.mm(r.height) > OVERLAP_MM)
    }

    fn text_over_data(
        &mut self,
        target: &TargetRef,
        proj: &Projector<'_>,
        figure: &Figure,
        annotations: &[TextBox],
        facts: &PlotFacts,
        page: impl Fn(Rect) -> Rect,
    ) {
        let marks: Vec<(SeriesId, Rect)> = figure
            .polygons
            .iter()
            .filter(|p| facts.kind(p.origin).is_some_and(is_mark))
            .filter(|p| p.origin.is_some_and(|o| facts.reports(SeriesId::new(o.0))))
            .filter_map(|p| {
                let projected: Vec<(f32, f32)> =
                    p.points.iter().map(|q| proj.project(*q)).collect();
                let (l, t) = projected
                    .iter()
                    .fold((f32::INFINITY, f32::INFINITY), |(l, t), q| {
                        (l.min(q.0), t.min(q.1))
                    });
                let (r, b) = projected
                    .iter()
                    .fold((f32::NEG_INFINITY, f32::NEG_INFINITY), |(r, b), q| {
                        (r.max(q.0), b.max(q.1))
                    });
                let rect = geometry::intersection(Rect::new(l, t, r - l, b - t), proj.plot)?;
                Some((SeriesId::new(p.origin?.0), rect))
            })
            .collect();
        let mut budget = TEXT_SCAN_POINTS;
        let mut emitted = 0;
        for text in annotations {
            let hit = marks
                .iter()
                .find_map(|(id, r)| self.overlaps(text.bounds, *r).map(|i| (*id, i)))
                .or_else(|| {
                    figure
                        .series
                        .iter()
                        .filter(|s| matches!(s.kind, SeriesKind::Line | SeriesKind::Points))
                        .filter(|s| facts.kind(s.origin).is_some())
                        .find_map(|s| {
                            let mut cursor = geometry::CurveScan::default();
                            let proj = Projector {
                                y_side: s.appearance.y_axis,
                                ..*proj
                            };
                            let result = geometry::scan_curve(
                                s,
                                &proj,
                                Some(proj.plot),
                                Some(text.bounds),
                                &mut cursor,
                                budget,
                                &|| false,
                            );
                            budget = budget.saturating_sub(result.visited);
                            self.scanned_points += result.visited;
                            let id = SeriesId::new(s.origin?.0);
                            result
                                .first_intersection
                                .filter(|_| result.intersections > 0)
                                .map(|i| (id, i))
                        })
                });
            let Some((id, intersection)) = hit else {
                continue;
            };
            emitted += 1;
            if emitted > MAX_PAIRS {
                continue;
            }
            let evidence = json!({"text_bounds":self.rect(page(text.bounds)),"intersection":self.rect(page(intersection)),"other_target":series_target(target, id)});
            self.hint(
                target,
                &text.component,
                "text_over_data",
                evidence,
                vec![suggest(
                    "Move the text into empty space, or set its anchor away from the data.",
                    None,
                    "Text over marks hides both.",
                )],
            );
        }
        if budget == 0 {
            self.coverage(
                target,
                "text_over_data",
                "unavailable",
                "Point budget reached; later text was not checked against curves.",
            );
        }
        self.unlisted(target, "text_over_data", emitted);
    }

    /// Canvas-wide layout hints, once every item has been scanned.
    pub(super) fn canvas_hints(&mut self) {
        self.hints_done = true;
        if !self.checks(DiagnosticCheck::LayoutAesthetics) {
            return;
        }
        let texts = std::mem::take(&mut self.hint_texts);
        let mut pairs = 0;
        for (i, (a, part_a, ra)) in texts.iter().enumerate() {
            for (b, part_b, rb) in &texts[i + 1..] {
                // A legend lays out its own entries, and colliding tick labels
                // of one axis are reported as too_many_categories.
                let axis_ticks =
                    |part: &str| part.split_once(".tick:").map(|(axis, _)| axis.to_owned());
                if a == b
                    && ((part_a.starts_with("guide.legend") && part_b.starts_with("guide.legend"))
                        || axis_ticks(part_a).is_some_and(|x| axis_ticks(part_b) == Some(x)))
                {
                    continue;
                }
                let Some(r) = self.overlaps(*ra, *rb) else {
                    continue;
                };
                pairs += 1;
                if pairs > MAX_PAIRS {
                    continue;
                }
                let (dx, dy) = (
                    (rb.left + rb.right()) - (ra.left + ra.right()),
                    (rb.top + rb.bottom()) - (ra.top + ra.bottom()),
                );
                let direction = if r.width < r.height {
                    if dx >= 0. { "right" } else { "left" }
                } else if dy >= 0. {
                    "down"
                } else {
                    "up"
                };
                let evidence = json!({"bounds":self.rect(*ra),"other_target":b,"other_component":part_b,"other_bounds":self.rect(*rb),"intersection":self.rect(r),"move":{"component":part_b,"direction":direction,"by_mm":self.mm(if r.width < r.height { r.width } else { r.height })}});
                self.hint(
                    a,
                    part_a,
                    "labels_overlap",
                    evidence,
                    vec![suggest(
                        &format!("Move {part_b} {direction}, or shorten one of the texts."),
                        None,
                        "Overlapping text cannot be read.",
                    )],
                );
            }
        }
        let canvas = self.params.canvas.clone();
        self.unlisted(&canvas, "labels_overlap", pairs);
        if self.texts_dropped > 0 {
            let reason = format!(
                "{} text boxes beyond {MAX_TEXTS} were not checked for overlap.",
                self.texts_dropped
            );
            self.coverage(&canvas, "labels_overlap", "unavailable", &reason);
        }
        self.panel_alignment();
    }

    /// Reports hint rows held back beyond [`MAX_PAIRS`].
    fn unlisted(&mut self, target: &TargetRef, check: &str, found: usize) {
        if found > MAX_PAIRS {
            let reason = format!("{} more {check} hints are not listed.", found - MAX_PAIRS);
            self.coverage(target, check, "unavailable", &reason);
        }
    }

    fn panel_alignment(&mut self) {
        let plots = std::mem::take(&mut self.hint_plots);
        let spans = |a0: f32, a1: f32, b0: f32, b1: f32| a0.max(b0) < a1.min(b1);
        let tolerance = SHARED_EDGE_MM / self.mm(1.);
        let shared = |a: f32, b: f32| (a - b).abs() < tolerance;
        let mut emitted = 0;
        for (i, (a, fa, pa)) in plots.iter().enumerate() {
            for (b, fb, pb) in &plots[i + 1..] {
                // Only frames placed as one column (or row) should align their plots.
                let column =
                    shared(fa.left, fb.left) && !spans(fa.top, fa.bottom(), fb.top, fb.bottom());
                let row = shared(fa.bottom(), fb.bottom())
                    && !spans(fa.left, fa.right(), fb.left, fb.right());
                let (edge, offset) = if column {
                    ("left", (pa.left - pb.left).abs())
                } else if row {
                    ("bottom", (pa.bottom() - pb.bottom()).abs())
                } else {
                    continue;
                };
                if self.mm(offset) <= MISALIGN_MM {
                    continue;
                }
                emitted += 1;
                if emitted > MAX_PAIRS {
                    continue;
                }
                let evidence = json!({"edge":edge,"offset_mm":self.mm(offset),"plot_area":self.rect(*pa),"other_target":b,"other_plot_area":self.rect(*pb),"threshold_mm":MISALIGN_MM});
                self.hint(
                    a,
                    "plot.area",
                    "panels_misaligned",
                    evidence,
                    vec![
                        suggest(
                            &format!("Align the {edge} edges of the two plot areas."),
                            None,
                            "Aligned axes let readers compare panels directly.",
                        ),
                        suggest(
                            "Match tick-label widths or axis-title presence between the panels.",
                            None,
                            "Different label widths shift the plot area inside equal frames.",
                        ),
                    ],
                );
            }
        }
        if let Some((target, _, _)) = plots.first() {
            self.unlisted(&target.clone(), "panels_misaligned", emitted);
        }
        for vertical in [false, true] {
            self.uneven_gaps(&plots, vertical);
        }
    }

    /// Gaps between three or more frames in one row (or column) should match.
    fn uneven_gaps(&mut self, plots: &[(TargetRef, Rect, Rect)], vertical: bool) {
        let along = |r: &Rect| {
            if vertical {
                (r.top, r.bottom())
            } else {
                (r.left, r.right())
            }
        };
        let across = |r: &Rect| {
            if vertical {
                (r.left, r.right())
            } else {
                (r.top, r.bottom())
            }
        };
        let mut seen: Vec<Vec<usize>> = Vec::new();
        for (_, f, _) in plots {
            let mut line: Vec<usize> = (0..plots.len())
                .filter(|j| {
                    let (a0, a1) = across(f);
                    let (b0, b1) = across(&plots[*j].1);
                    a0.max(b0) < a1.min(b1)
                })
                .collect();
            line.sort_by(|x, y| along(&plots[*x].1).0.total_cmp(&along(&plots[*y].1).0));
            if line.len() < 3 || seen.contains(&line) {
                continue;
            }
            let gaps: Vec<f32> = line
                .windows(2)
                .map(|w| along(&plots[w[1]].1).0 - along(&plots[w[0]].1).1)
                .collect();
            // Overlapping frames (an inset) are not a sequence of panels.
            if gaps.iter().any(|g| *g < 0.) {
                seen.push(line);
                continue;
            }
            let (low, high) = gaps
                .iter()
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(l, h), g| {
                    (l.min(*g), h.max(*g))
                });
            if self.mm(high - low) > MISALIGN_MM {
                let mm: Vec<f32> = gaps.iter().map(|g| self.mm(*g)).collect();
                let members: Vec<&TargetRef> = line.iter().map(|j| &plots[*j].0).collect();
                self.hint(&plots[line[0]].0, "plot.frame", "panels_misaligned", json!({"direction":if vertical {"column"} else {"row"},"gaps_mm":mm,"panels":members,"threshold_mm":MISALIGN_MM}), vec![
                    suggest("Space the panels evenly.", Some(("canvas.layout.gutter_mm", &self.params.canvas)), "Equal gutters read as one figure."),
                ]);
            }
            seen.push(line);
        }
    }
}

#[cfg(test)]
#[path = "hints_tests.rs"]
mod tests;
