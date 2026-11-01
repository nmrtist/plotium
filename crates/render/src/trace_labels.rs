//! Names of Offset stack traces, written just above the line each trace runs
//! along. Every backend draws them through [`crate::LabelLayout`].
use crate::Rect;
use crate::label_placement::text_extent;
use crate::legend_placement::{for_each_data_segment, project};
use crate::text_place::{PlacedText, TextAlign};
use plotium_figure::{Annotation, Color, Figure, GuideVisibility, Series, TraceLabelSide};

/// Space between a name and its trace or the plot edge, in pt.
const GAP_PT: f32 = 2.0;
/// Height one name needs in its stack, in ems.
const ROW_EM: f32 = 1.15;
/// Names shrink to this size, in pt, before every other one is dropped.
const MIN_SIZE_PT: f32 = 5.0;
/// Column steps from each plot end towards the middle tried on an automatic side.
const COLUMN_STEPS: usize = 12;

/// A trace's name and where it stands.
#[derive(Clone, Debug)]
pub(crate) struct TraceText {
    pub annotation: Annotation,
    pub at: PlacedText,
}

impl TraceText {
    pub(crate) fn bounds(&self) -> Rect {
        let (width, height) = text_extent(&self.annotation.text, self.annotation.size);
        crate::text_place::bounds(self.at, width, height)
    }

    pub(crate) fn scaled(self, scale: f32) -> Self {
        Self {
            at: PlacedText {
                x: self.at.x * scale,
                y: self.at.y * scale,
                ..self.at
            },
            ..self
        }
    }
}

/// A trace on the stack, lowest on the plot first.
struct Row<'a> {
    text: &'a str,
    color: Color,
    /// Output y the name stands on.
    level: f32,
    width: f32,
}

/// The vertical band of the plot the names stand in.
#[derive(Clone, Copy)]
struct Column {
    left: f32,
    /// Names end at the right of the band instead of starting at its left.
    flush_right: bool,
}

/// Output y of the line a trace mostly runs along: the median of its samples
/// inside the plot, so a tall peak does not lift the name off that line.
fn trace_level(fig: &Figure, plot: Rect, series: &Series) -> Option<f32> {
    let mut levels: Vec<f32> = series
        .points
        .iter()
        .map(|point| project(fig, plot, *point, series.appearance.y_axis))
        .filter(|point| {
            crate::is_finite_point(*point) && (plot.left..=plot.right()).contains(&point.0)
        })
        .map(|point| point.1)
        .collect();
    if levels.is_empty() {
        return None;
    }
    let middle = levels.len() / 2;
    let (_, median, _) = levels.select_nth_unstable_by(middle, f32::total_cmp);
    (plot.top..=plot.bottom())
        .contains(median)
        .then_some(*median)
}

/// The columns to try, preferred first: the right end, the left end, then
/// inwards from both.
fn columns(side: TraceLabelSide, plot: Rect, widest: f32) -> Vec<Column> {
    let first = plot.left + GAP_PT;
    let last = (plot.right() - GAP_PT - widest).max(first);
    let right = Column {
        left: last,
        flush_right: true,
    };
    let left = |left| Column {
        left,
        flush_right: false,
    };
    match side {
        TraceLabelSide::Right => vec![right],
        TraceLabelSide::Left => vec![left(first)],
        TraceLabelSide::Auto => {
            let step = (last - first) / (2 * COLUMN_STEPS) as f32;
            let mut columns = vec![right, left(first)];
            for inward in 1..=COLUMN_STEPS {
                let inward = inward as f32 * step;
                columns.extend([left(last - inward), left(first + inward)]);
            }
            columns
        }
    }
}

/// The column whose names data cuts through least, earliest on ties. A line
/// crossing a whole name costs far more than a hump grazing its foot, so each
/// data segment counts by the square of its height inside the name's row.
fn clearest_column(
    fig: &Figure,
    plot: Rect,
    columns: Vec<Column>,
    rows: &[Row],
    height: f32,
    widest: f32,
) -> Option<Column> {
    if columns.len() < 2 {
        return columns.into_iter().next();
    }
    let span = |column: &Column, row: &Row| {
        let offset = if column.flush_right {
            widest - row.width
        } else {
            0.0
        };
        (column.left + offset, column.left + offset + row.width)
    };
    let mut costs = vec![0.0_f32; columns.len()];
    for_each_data_segment(fig, plot, |a, b| {
        let (left, right) = (a.0.min(b.0), a.0.max(b.0));
        let (top, bottom) = (a.1.min(b.1), a.1.max(b.1));
        for row in rows {
            let inside = bottom.min(row.level) - top.max(row.level - height);
            if inside < 0.0 {
                continue;
            }
            for (column, cost) in columns.iter().zip(&mut costs) {
                let (start, end) = span(column, row);
                if left <= end && right >= start {
                    *cost += inside * inside;
                }
            }
        }
    });
    columns
        .into_iter()
        .zip(costs)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(column, _)| column)
}

/// Where the trace names of a figure stand, and the names there was no room for.
#[derive(Clone, Debug, Default)]
pub(crate) struct TracePlacement {
    pub texts: Vec<TraceText>,
    pub omitted: Vec<String>,
}

/// Place the trace names of `fig`, in the output units of `plot`, which are
/// points. The guide's Show and Hide settings turn them off.
pub(crate) fn place(fig: &Figure, plot: Rect) -> TracePlacement {
    let Some(labels) = &fig.trace_labels else {
        return TracePlacement::default();
    };
    if !matches!(fig.guide_visibility, GuideVisibility::Auto) {
        return TracePlacement::default();
    }
    let mut levels: Vec<(&str, Color, f32)> = labels
        .traces
        .iter()
        .filter_map(|trace| {
            let series = fig.series.get(trace.series)?;
            let level = trace_level(fig, plot, series)?;
            Some((trace.text.as_str(), series.color, level))
        })
        .collect();
    levels.sort_by(|a, b| b.2.total_cmp(&a.2));
    let (Some(lowest), Some(highest)) = (levels.first(), levels.last()) else {
        return TracePlacement::default();
    };
    // Traces may fill only part of the plot, so measure along their own run.
    let pitch = match levels.len() {
        1 => plot.height,
        count => (lowest.2 - highest.2) / (count - 1) as f32,
    };
    let legend_pt = fig.typography.legend_pt;
    let smallest = MIN_SIZE_PT.min(legend_pt);
    let fitting = pitch / ROW_EM;
    let size = fitting.clamp(smallest, legend_pt);
    let stride = if fitting >= smallest {
        1
    } else if fitting > 0.0 {
        (smallest / fitting).ceil() as usize
    } else {
        levels.len()
    };
    let height = text_extent("x", size).1;
    // Backgrounds differ between traces, so a name can reach the one above it.
    let mut ceiling = plot.bottom();
    let mut rows: Vec<Row> = Vec::new();
    let mut omitted = Vec::new();
    for (index, (text, color, level)) in levels.into_iter().enumerate() {
        let level = (level - GAP_PT).min(ceiling).max(plot.top + height);
        if index % stride != 0 || level > ceiling {
            omitted.push(text.to_owned());
            continue;
        }
        ceiling = level - ROW_EM * size;
        rows.push(Row {
            text,
            color,
            level,
            width: text_extent(text, size).0,
        });
    }
    let widest = rows.iter().map(|row| row.width).fold(0.0, f32::max);
    let candidates = columns(labels.side, plot, widest);
    let Some(column) = clearest_column(fig, plot, candidates, &rows, height, widest) else {
        return TracePlacement {
            texts: Vec::new(),
            omitted,
        };
    };
    let at = |row: &Row| PlacedText {
        x: if column.flush_right {
            column.left + widest
        } else {
            column.left
        },
        y: row.level - 0.5 * height,
        align: if column.flush_right {
            TextAlign::End
        } else {
            TextAlign::Start
        },
        middle: true,
    };
    let texts = rows
        .iter()
        .map(|row| TraceText {
            annotation: Annotation {
                text: row.text.to_owned(),
                at: [0.0, 0.0],
                display_baseline: 0.0,
                color: row.color,
                size,
                anchor: plotium_figure::TextAnchor::Point,
                offset_pt: None,
            },
            at: at(row),
        })
        .collect();
    TracePlacement { texts, omitted }
}

#[cfg(test)]
#[path = "trace_labels_tests.rs"]
mod tests;
