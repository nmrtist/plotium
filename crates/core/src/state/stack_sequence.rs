//! The sequence variable of a line stack, and what it derives: trace colours
//! and the side scale.

use super::StackSpec;
use super::table_charts::sequence::sequence_axis;
use plotium_figure::{Axis, ErrorBar, Figure, GuideVisibility, SequenceScale, Series, SeriesKind};
use std::ops::Range;

/// One numeric position per visible trace: the sequence read from the trace
/// names, otherwise the trace count.
pub(super) struct TraceSequence {
    values: Vec<f64>,
    label: String,
}

impl TraceSequence {
    pub(super) fn of<'a>(names: impl Iterator<Item = &'a str>) -> Self {
        let names: Vec<&str> = names.collect();
        sequence_axis(names.iter().copied()).map_or_else(
            || Self {
                values: (1..=names.len()).map(|trace| trace as f64).collect(),
                label: "Trace".to_owned(),
            },
            |axis| Self {
                values: axis.values,
                label: axis.label,
            },
        )
    }

    /// Where a trace falls along the sequence, in `[0, 1]`.
    pub(super) fn fraction(&self, position: usize) -> f32 {
        let (Some(&first), Some(&last), Some(&value)) = (
            self.values.first(),
            self.values.last(),
            self.values.get(position),
        ) else {
            return 0.0;
        };
        if first == last {
            return 0.0;
        }
        ((value - first) / (last - first)) as f32
    }

    pub(super) fn scale(&self, baselines: &[f64], y_min: f64, y_max: f64) -> SequenceScale {
        SequenceScale {
            axis: Axis::new(self.label.clone(), y_min, y_max),
            ticks: baselines
                .iter()
                .zip(&self.values)
                .map(|(&y, value)| (y, format!("{value}")))
                .collect(),
        }
    }
}

/// One trace of an Offset stack: a run of `Figure::series` on one baseline.
pub(super) struct StackTrace {
    pub name: String,
    pub baseline: f64,
    pub series: Range<usize>,
}

/// Apply the Offset options that derive from the sequence. The first curve of a
/// trace is its data, so it carries the fill.
pub(super) fn apply_stack_options(fig: &mut Figure, stack: &StackSpec, traces: &[StackTrace]) {
    let sequence = TraceSequence::of(traces.iter().map(|trace| trace.name.as_str()));
    for (position, trace) in traces.iter().enumerate() {
        if let Some(map) = stack.gradient {
            let color = map.sample(sequence.fraction(position));
            for series in &mut fig.series[trace.series.clone()] {
                series.color = color;
            }
            for whisker in &mut fig.error_bars {
                if whisker.display_baseline == trace.baseline {
                    whisker.color = color;
                }
            }
        }
        if stack.occlude
            && let Some(data) = fig.series[trace.series.clone()].first_mut()
        {
            data.occludes = data.kind == SeriesKind::Line;
        }
    }
    if stack.occlude {
        // Under a trace's own fill the lower arm would be lost.
        for whisker in &mut fig.error_bars {
            whisker.draw_over_data = true;
        }
    }
    if stack.sequence_scale {
        let baselines: Vec<f64> = traces.iter().map(|trace| trace.baseline).collect();
        fig.sequence_scale = Some(sequence.scale(&baselines, fig.y.min, fig.y.max));
        // The scale names the traces, so a legend would only repeat it.
        fig.guide_visibility = GuideVisibility::Hide;
    }
}

/// One line trace per column of a table figure, which draws its columns as
/// points in order, then their fit curves, and whiskers in column and row
/// order. A column of isolated points has no line to draw and stays points.
pub(super) fn split_columns(
    series: Vec<Series>,
    names: Vec<String>,
    error_bars: Vec<ErrorBar>,
    line_width: f32,
) -> Vec<(String, Vec<Series>, Vec<ErrorBar>)> {
    let mut traces: Vec<(String, Vec<Series>, Vec<ErrorBar>)> = Vec::new();
    for (mut column, name) in series.into_iter().zip(names) {
        if column.kind == SeriesKind::Line
            && let Some((_, curves, _)) = traces.iter_mut().find(|trace| trace.0 == name)
        {
            curves.push(column);
            continue;
        }
        let finite = |point: &[f64; 2]| point.iter().all(|value| value.is_finite());
        if column
            .points
            .windows(2)
            .any(|pair| finite(&pair[0]) && finite(&pair[1]))
        {
            column.kind = SeriesKind::Line;
            column.width = line_width;
        }
        traces.push((name, vec![column], Vec::new()));
    }
    let (mut trace, mut point) = (0, 0);
    for error_bar in error_bars {
        while let Some((_, curves, _)) = traces.get(trace) {
            match curves[0].points.get(point) {
                Some(&observed) if observed == error_bar.center => break,
                Some(_) => point += 1,
                None => (trace, point) = (trace + 1, 0),
            }
        }
        if let Some((_, _, whiskers)) = traces.get_mut(trace) {
            whiskers.push(error_bar);
            point += 1;
        }
    }
    traces
}

#[cfg(test)]
#[path = "stack_sequence_tests.rs"]
mod tests;
