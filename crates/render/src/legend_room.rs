//! Room for an automatic legend: when every inside anchor covers data, widen
//! the automatic axis windows on one anchor's side until it is clear.
use crate::legend_placement::{
    ANCHORS, LEGEND_INSET, anchor_boxes, clip, coverage, for_each_segment, grow,
};
use crate::{Projector, Rect, axis_layout, legend_entries, legend_layout, renders_legend};
use plotium_figure::{Axis, Figure, GuidePlacement};

/// Largest growth of an axis span made to clear a legend slot. A legend that
/// needs more stays where automatic placement puts it.
const MAX_ROOM_GROWTH: f64 = 0.5;

/// Cells per side of the occupancy grid used while searching for growth.
const GRID: usize = 256;

/// Widen automatic axis windows so an automatic legend gets an inside slot
/// free of data, for a plot object of `size` points.
///
/// Nothing changes while some anchor is already clear. Otherwise each anchor
/// may extend the axis ends on its own side — x, y, or both by the same
/// fraction — and the smallest growth that clears an anchor wins; ties keep
/// the anchor order and then x before y. Only axes passed as growable and not
/// categorical are touched. A growable right axis follows the left y axis by
/// the same fraction on the same screen side, so both keep their alignment.
/// Returns whether the figure changed.
pub fn make_legend_room(
    fig: &mut Figure,
    size: [f32; 2],
    grow_x: bool,
    grow_y: bool,
    grow_y2: bool,
) -> bool {
    let growable = |axis: &Axis, allowed: bool| {
        allowed && axis.categories.is_none() && axis.min.is_finite() && axis.max.is_finite()
    };
    let grow_x = growable(&fig.x, grow_x);
    let grow_y = growable(&fig.y, grow_y);
    let grow_y2 = grow_y && fig.y2.as_ref().is_some_and(|axis| growable(axis, grow_y2));
    // Widening one axis of an aspect-locked plot would reshape its box.
    if !(grow_x || grow_y)
        || fig.lock_aspect
        || fig.guide_placement != GuidePlacement::Auto
        || fig.legend_position.is_some()
        || !renders_legend(fig)
    {
        return false;
    }
    let legend = legend_layout(fig, &legend_entries(fig));
    // The same tight zones that automatic placement treats as clear.
    let zones = |fig: &Figure| {
        let layout = axis_layout(fig, size[0], size[1]);
        let outer = Rect::new(0.0, 0.0, size[0], size[1]);
        let plot = Projector::new(fig, outer, &layout.margins).plot;
        let boxes = anchor_boxes(plot, legend.width, legend.height, LEGEND_INSET);
        (plot, boxes.map(|r| grow(r, LEGEND_INSET * 0.25)))
    };
    let (plot, base_zones) = zones(fig);
    if plot.width <= 0.0 || plot.height <= 0.0 || coverage(fig, plot, &base_zones).contains(&0) {
        return false;
    }

    let (base_x, base_y) = (fig.x.clone(), fig.y.clone());
    let base_y2 = fig.y2.clone();
    let grid = Occupancy::new(fig);
    let mut candidates = Vec::new();
    for (index, [fx, fy]) in ANCHORS.iter().enumerate() {
        // `Some(true)` extends the axis maximum, `Some(false)` its minimum:
        // whichever end sits on the anchor's screen side.
        let x_side = (grow_x && *fx != 0.5).then_some((*fx == 1.0) != base_x.reversed);
        let y_side = (grow_y && *fy != 0.5).then_some((*fy == 0.0) != base_y.reversed);
        let mut options = vec![[x_side, None], [None, y_side]];
        if x_side.is_some() && y_side.is_some() {
            options.push([x_side, y_side]);
        }
        for sides in options.into_iter().filter(|s| *s != [None, None]) {
            let mut grid_clear = |fraction: f64| {
                fig.x = widened(&base_x, sides[0], fraction);
                fig.y = widened(&base_y, sides[1], fraction);
                fig.y2 = widened_y2(&base_y, base_y2.as_ref(), sides[1], grow_y2, fraction);
                let (plot, trial) = zones(fig);
                grid.is_clear(fig, &base_x, &base_y, plot, trial[index])
            };
            if !grid_clear(MAX_ROOM_GROWTH) {
                continue;
            }
            // Bisect on the smallest clearing growth. Tick and margin changes
            // can make the test non-monotonic, so only a clear fraction is kept.
            let (mut low, mut high) = (0.0, MAX_ROOM_GROWTH);
            for _ in 0..10 {
                let middle = 0.5 * (low + high);
                if grid_clear(middle) {
                    high = middle;
                } else {
                    low = middle;
                }
            }
            candidates.push((high, index, sides));
        }
    }
    // The grid over-approximates the data, so its answers are confirmed
    // with the exact test that automatic placement uses.
    candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (fraction, index, sides) in candidates {
        fig.x = widened(&base_x, sides[0], fraction);
        fig.y = widened(&base_y, sides[1], fraction);
        fig.y2 = widened_y2(&base_y, base_y2.as_ref(), sides[1], grow_y2, fraction);
        let (plot, trial) = zones(fig);
        if coverage(fig, plot, &trial[index..=index]) == [0] {
            return true;
        }
    }
    fig.x = base_x;
    fig.y = base_y;
    fig.y2 = base_y2;
    false
}

/// `axis` with one end moved outward by `fraction` of its span, measured in
/// the axis scale so a log axis grows by decades and never reaches zero.
fn widened(axis: &Axis, side: Option<bool>, fraction: f64) -> Axis {
    let mut axis = axis.clone();
    let scale = axis.effective_scale();
    let (min, max) = (scale.forward(axis.min), scale.forward(axis.max));
    let extra = (max - min).abs() * fraction;
    match side {
        Some(true) => axis.max = scale.inverse(max + extra),
        Some(false) => axis.min = scale.inverse(min - extra),
        None => {}
    }
    axis
}

/// The right axis widened toward the screen side `side` gives the left axis,
/// or unchanged when it may not grow.
fn widened_y2(
    base_y: &Axis,
    base_y2: Option<&Axis>,
    side: Option<bool>,
    grow: bool,
    fraction: f64,
) -> Option<Axis> {
    base_y2.map(|axis| {
        let side = side
            .filter(|_| grow)
            .map(|max| (max != base_y.reversed) != axis.reversed);
        widened(axis, side, fraction)
    })
}

/// Which cells of the original axis windows hold data, as prefix sums. Every
/// cell a drawn segment touches is marked, so a clear query is truly clear.
struct Occupancy {
    sums: Vec<u32>,
}

impl Occupancy {
    fn new(fig: &Figure) -> Self {
        let side = GRID as f32;
        let bounds = Rect::new(0.0, 0.0, side, side);
        let mut cells = vec![false; GRID * GRID];
        for_each_segment(fig, bounds, |a, b| {
            let Some((t0, t1)) = clip(a, b, bounds) else {
                return;
            };
            let at = |t: f32| (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            let (start, end) = (at(t0), at(t1));
            // Steps of at most one cell keep each step's bounding box tight.
            let steps = (end.0 - start.0).abs().max((end.1 - start.1).abs()).ceil() as usize;
            let steps = steps.max(1);
            let cell = |v: f32| (v.floor().max(0.0) as usize).min(GRID - 1);
            let mut previous = start;
            for step in 1..=steps {
                let t = step as f32 / steps as f32;
                let next = (
                    start.0 + (end.0 - start.0) * t,
                    start.1 + (end.1 - start.1) * t,
                );
                for row in cell(previous.1.min(next.1))..=cell(previous.1.max(next.1)) {
                    for column in cell(previous.0.min(next.0))..=cell(previous.0.max(next.0)) {
                        cells[row * GRID + column] = true;
                    }
                }
                previous = next;
            }
        });
        let width = GRID + 1;
        let mut sums = vec![0u32; width * width];
        for row in 0..GRID {
            for column in 0..GRID {
                sums[(row + 1) * width + column + 1] = u32::from(cells[row * GRID + column])
                    + sums[row * width + column + 1]
                    + sums[(row + 1) * width + column]
                    - sums[row * width + column];
            }
        }
        Self { sums }
    }

    /// Whether `zone` on the trial plot covers no occupied cell. `base_x` and
    /// `base_y` are the windows the grid was built from.
    fn is_clear(&self, fig: &Figure, base_x: &Axis, base_y: &Axis, plot: Rect, zone: Rect) -> bool {
        let side = GRID as f64;
        let x_at = |px: f32| {
            let value = fig.x.value_at(f64::from((px - plot.left) / plot.width));
            base_x.normalize(value) * side
        };
        let y_at = |py: f32| {
            let value = fig
                .y
                .value_at(f64::from(1.0 - (py - plot.top) / plot.height));
            (1.0 - base_y.normalize(value)) * side
        };
        let span = |a: f64, b: f64| {
            let (low, high) = (a.min(b), a.max(b));
            if high < 0.0 || low >= side || !(low.is_finite() && high.is_finite()) {
                return None;
            }
            Some((low.max(0.0) as usize, (high.floor() as usize).min(GRID - 1)))
        };
        let (Some((left, right)), Some((top, bottom))) = (
            span(x_at(zone.left), x_at(zone.right())),
            span(y_at(zone.top), y_at(zone.bottom())),
        ) else {
            return true;
        };
        let width = GRID + 1;
        let total = self.sums[(bottom + 1) * width + right + 1] + self.sums[top * width + left]
            - self.sums[top * width + right + 1]
            - self.sums[(bottom + 1) * width + left];
        total == 0
    }
}

#[cfg(test)]
#[path = "legend_room_tests.rs"]
mod tests;
