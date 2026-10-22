//! Lowering of explicit bar curves into the figure IR's filled rectangles and
//! whiskers, so renderers and exporters need no bar-specific code.
use plotium_figure::{Color, ErrorBar, Figure, LineEncoding, Polygon, Series};
use std::collections::HashMap;

/// Caps span this fraction of their bar, the usual journal proportion.
const CAP_FRACTION: f64 = 0.45;
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
}

impl BarStyle {
    pub fn of(line: &LineEncoding) -> Self {
        let edge = line.bar_edge_width() as f32;
        Self {
            width: line.bar_width(),
            edge: (edge > 0.0).then_some((line.bar_edge_color, edge)),
        }
    }

    /// The resolved outline around a bar filled with `fill`.
    fn edge_for(self, fill: Color) -> Option<(Color, f32)> {
        self.edge
            .map(|(color, width)| (bar_edge_color(color, fill), width))
    }
}

/// One visible bar curve after styling, scaling and stack offsets.
pub(super) struct PreparedBars {
    pub series: Vec<Series>,
    pub errors: Vec<ErrorBar>,
    pub style: BarStyle,
    pub baseline: f64,
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

/// Data-space extents of the lowered bars: `[xmin, xmax, ymin, ymax]`.
pub(super) type BarExtents = [f64; 4];

/// Draws bar curves side by side: each slot is split evenly between the bar
/// curves that have a bar in it, and each bar fills its own width fraction of
/// its share. A curve alone in a slot keeps the whole slot, so one-bar-per-curve
/// charts stay centred. Category slots are one unit wide; numeric slots are the
/// smallest gap between distinct bar x positions, so uneven rulers never overlap.
pub(super) fn lower_bars(
    figure: &mut Figure,
    bars: Vec<PreparedBars>,
    categorical: bool,
) -> Result<Option<BarExtents>, String> {
    if bars.is_empty() {
        return Ok(None);
    }
    let slot = if categorical {
        1.0
    } else {
        numeric_slot(&bars)
    };
    // Curves present at each x position, in curve order. Adding zero folds
    // -0.0 into 0.0 so both land in one slot.
    let key = |x: f64| (x + 0.0).to_bits();
    let mut occupants: HashMap<u64, Vec<usize>> = HashMap::new();
    for (index, bar) in bars.iter().enumerate() {
        for point in bar.series.iter().flat_map(|series| &series.points) {
            let curves = occupants.entry(key(point[0])).or_default();
            if curves.last() != Some(&index) {
                curves.push(index);
            }
        }
    }
    // Offset from the slot centre and the share of the slot for one curve at x.
    let dodge = |index: usize, x: f64| {
        let curves = occupants.get(&key(x)).map_or(&[][..], Vec::as_slice);
        let count = curves.len().max(1);
        let rank = curves.iter().position(|c| *c == index).unwrap_or(0);
        let share = slot / count as f64;
        ((rank as f64 + 0.5) * share - 0.5 * slot, share)
    };
    let mut extents = [
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ];
    for (index, bar) in bars.iter().enumerate() {
        for series in &bar.series {
            for &[x, y] in &series.points {
                let (dx, share) = dodge(index, x);
                let half = 0.5 * bar.style.width * share;
                let (x0, x1) = (x + dx - half, x + dx + half);
                if ![x0, x1].into_iter().all(f64::is_finite) {
                    return Err("Bar presentation overflowed.".into());
                }
                extents[0] = extents[0].min(x0);
                extents[1] = extents[1].max(x1);
                extents[2] = extents[2].min(y.min(bar.baseline));
                extents[3] = extents[3].max(y.max(bar.baseline));
                let rect =
                    Polygon::rect(series.name.clone(), x0, x1, bar.baseline, y, series.color);
                figure
                    .polygons
                    .push(match bar.style.edge_for(series.color) {
                        Some((color, width)) => rect.with_stroke(color, width),
                        None => rect,
                    });
            }
        }
        for error in &bar.errors {
            let mut error = error.clone();
            // Whiskers are built at their bar's undodged x.
            let (dx, share) = dodge(index, error.center[0]);
            error.center[0] += dx;
            // Caps scale with the bar rather than a fixed point width, which
            // on a narrow grouped bar reads as a flat bracket.
            error.cap_data_width = Some(CAP_FRACTION * bar.style.width * share);
            // An outlined bar draws its whiskers in the outline's ink; styling
            // has already given each whisker its bar's fill.
            if let Some((color, width)) = bar.style.edge_for(error.color) {
                error.color = color;
                error.width = width;
            }
            // A filled bar would hide the lower whisker drawn beneath it.
            figure.error_bars.push(error.over_data());
        }
    }
    Ok(extents[0].is_finite().then_some(extents))
}

fn numeric_slot(bars: &[PreparedBars]) -> f64 {
    let mut xs: Vec<f64> = bars
        .iter()
        .flat_map(|bar| &bar.series)
        .flat_map(|series| series.points.iter().map(|p| p[0]))
        .collect();
    xs.sort_by(f64::total_cmp);
    xs.dedup();
    let gap = xs
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .fold(f64::INFINITY, f64::min);
    // A single position has no neighbour to collide with.
    if gap.is_finite() { gap } else { 1.0 }
}
