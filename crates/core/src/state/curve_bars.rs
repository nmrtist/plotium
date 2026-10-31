//! Bar styling, category slots and numeric slot widths of explicit curves;
//! [`super::curve_layout`] lays the marks out.
use plotium_figure::{Color, ErrorBar, Hatch, HatchPattern, LineEncoding, Series};
use std::collections::HashMap;

/// Caps span this fraction of their bar, the usual journal proportion.
pub(super) const CAP_FRACTION: f64 = 0.45;
/// The outline a bar draws around `fill`: the explicit color, else a
/// near-black ink tinted toward the fill's hue, so outlines and whiskers stay
/// clearly darker than any fill while still belonging to it.
pub(crate) fn bar_edge_color(explicit: Option<Color>, fill: Color) -> Color {
    explicit.unwrap_or_else(|| fill.ink_shade())
}

/// Bar presentation read from a bar curve's line encoding.
#[derive(Clone, Copy)]
pub(super) struct BarStyle {
    /// Bar width as a fraction of this curve's share of a slot.
    pub width: f64,
    /// Outline color (`None` derives it from each bar's fill) and width in
    /// points; `None` draws no outline.
    pub edge: Option<(Option<Color>, f32)>,
    /// Hatch lines take the explicit outline color, or derive it like the
    /// outline does.
    pub hatch: Option<(HatchPattern, Option<Color>)>,
}

impl BarStyle {
    /// `aux_line_pt` is the document's supporting-line width.
    pub fn of(line: &LineEncoding, aux_line_pt: Option<f32>) -> Self {
        let edge = line.bar_edge_width(aux_line_pt) as f32;
        Self {
            width: line.bar_width(),
            edge: (edge > 0.0).then_some((line.bar_edge_color, edge)),
            hatch: line.bar_hatch.map(|pattern| (pattern, line.bar_edge_color)),
        }
    }

    /// The resolved outline around a bar filled with `fill`.
    pub(super) fn edge_for(self, fill: Color) -> Option<(Color, f32)> {
        self.edge
            .map(|(color, width)| (bar_edge_color(color, fill), width))
    }

    /// The resolved hatch over a bar filled with `fill`.
    pub(super) fn hatch_for(self, fill: Color) -> Option<Hatch> {
        self.hatch.map(|(pattern, color)| Hatch {
            pattern,
            color: bar_edge_color(color, fill),
        })
    }
}

/// Category slots shared by every text-x curve of one plot, in first-occurrence
/// order across curves and rows.
#[derive(Default)]
pub(super) struct CategorySlots {
    names: Vec<String>,
    index: HashMap<String, usize>,
}

impl CategorySlots {
    /// Rewrites curve-local category indices to plot-wide slots.
    pub fn remap(&mut self, local: &[String], runs: &mut [Vec<[f64; 2]>], errors: &mut [ErrorBar]) {
        let slots: Vec<f64> = local
            .iter()
            .map(|name| {
                *self.index.entry(name.clone()).or_insert_with(|| {
                    self.names.push(name.clone());
                    self.names.len() - 1
                }) as f64
            })
            .collect();
        // Extraction emits exact small integer indices into `local`.
        let xs = runs
            .iter_mut()
            .flatten()
            .map(|point| &mut point[0])
            .chain(errors.iter_mut().map(|error| &mut error.center[0]));
        for x in xs {
            if let Some(slot) = slots.get(*x as usize) {
                *x = *slot;
            }
        }
    }

    pub fn into_names(self) -> Vec<String> {
        self.names
    }
}

/// Slot width at each numeric x position of `series`, keyed by the position
/// with -0.0 folded into 0.0.
pub(super) fn numeric_slots_at<'a>(series: impl Iterator<Item = &'a Series>) -> HashMap<u64, f64> {
    let mut xs: Vec<f64> = series
        .flat_map(|series| series.points.iter().map(|p| p[0] + 0.0))
        .collect();
    xs.sort_by(f64::total_cmp);
    xs.dedup();
    let widths = slot_widths(&xs);
    xs.iter()
        .zip(widths)
        .map(|(x, width)| (x.to_bits(), width))
        .collect()
}

/// Slot width at each of the sorted, distinct finite `positions`: the median
/// gap between neighbours, narrowed only where a position's own neighbours
/// sit closer. One close pair then narrows its own bars instead of every bar,
/// and evenly spaced positions keep their full gap. A lone position has no
/// neighbour to collide with and gets one unit.
pub(crate) fn slot_widths(positions: &[f64]) -> Vec<f64> {
    let mut gaps: Vec<f64> = positions.windows(2).map(|p| p[1] - p[0]).collect();
    if gaps.is_empty() {
        return vec![1.0; positions.len()];
    }
    let local: Vec<f64> = (0..positions.len())
        .map(|i| {
            let left = i.checked_sub(1).map_or(f64::INFINITY, |j| gaps[j]);
            let right = gaps.get(i).copied().unwrap_or(f64::INFINITY);
            left.min(right)
        })
        .collect();
    gaps.sort_by(f64::total_cmp);
    let median = gaps[gaps.len() / 2];
    // Gaps between extreme positions can overflow; fall back to one unit.
    local
        .into_iter()
        .map(|gap| {
            Some(gap.min(median))
                .filter(|w| w.is_finite())
                .unwrap_or(1.0)
        })
        .collect()
}
