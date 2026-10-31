//! The side scale of a waterfall stack and the back-to-front paint order that
//! lets each trace hide the ones behind it.
use crate::{Axis, Figure, Series};

/// A right-side scale naming traces by their sequence variable (time,
/// temperature, potential) at their baselines. Stands in for the right axis.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SequenceScale {
    /// Title and visibility; the ticks come from `ticks`.
    pub axis: Axis,
    /// Left-axis y of each trace baseline with its label.
    pub ticks: Vec<(f64, String)>,
}

impl Figure {
    /// Series with their indices in paint order. Among occluding traces the
    /// highest baseline is the rearmost and paints first; curves of one trace
    /// keep their order, so the first one's fill lies under the rest.
    pub fn series_in_paint_order(&self) -> impl Iterator<Item = (usize, &Series)> {
        let mut order: Vec<usize> = (0..self.series.len()).collect();
        if self.series.iter().any(|series| series.occludes) {
            let baseline = |index: usize| self.series[index].display_baseline;
            order.sort_by(|a, b| baseline(*b).total_cmp(&baseline(*a)));
        }
        order.into_iter().map(|index| (index, &self.series[index]))
    }
}
