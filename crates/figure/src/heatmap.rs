//! The dense colormapped cell grid drawn under a figure.
use crate::ColormapId;

/// A dense value grid rendered as filled cells through a colormap. Unlike
/// [`Contour`] — which keeps only extracted line segments — the raw matrix is
/// retained so renderers can fill every cell.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HeatmapGrid {
    pub rows: usize,
    pub cols: usize,
    /// Row-major `rows × cols` values (the [`crate::Contour`]/marching-squares
    /// convention). Non-finite cells are skipped (drawn as background).
    pub values: Vec<f32>,
    /// Data-space x of the leftmost and rightmost cell edges; columns divide
    /// this span evenly.
    pub x_bounds: [f64; 2],
    /// Data-space y of the row-0 edge and the last-row edge; rows divide this
    /// span evenly.
    pub y_bounds: [f64; 2],
    /// `cols + 1` x cell boundaries for unevenly spaced columns, replacing the
    /// even division of `x_bounds`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x_edges: Option<Vec<f64>>,
    /// `rows + 1` y cell boundaries for unevenly spaced rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y_edges: Option<Vec<f64>>,
    pub colormap: ColormapId,
    /// Value range mapped to the colormap ends; cells clamp outside it.
    pub value_range: [f32; 2],
}

impl HeatmapGrid {
    /// The normalized `[0, 1]` colormap position of one cell, or `None` for a
    /// missing / non-finite cell.
    pub fn normalized(&self, row: usize, col: usize) -> Option<f32> {
        let v = *self.values.get(row * self.cols + col)?;
        if !v.is_finite() {
            return None;
        }
        let [lo, hi] = self.value_range;
        let span = hi - lo;
        if span.abs() < f32::EPSILON {
            return Some(0.5);
        }
        Some(((v - lo) / span).clamp(0.0, 1.0))
    }

    /// Data-space rectangle `[x0, x1, y0, y1]` covered by one cell.
    pub fn cell_bounds(&self, row: usize, col: usize) -> [f64; 4] {
        let [x0, x1] = cell_span(self.x_edges.as_deref(), self.x_bounds, self.cols, col);
        let [y0, y1] = cell_span(self.y_edges.as_deref(), self.y_bounds, self.rows, row);
        [x0, x1, y0, y1]
    }
}

fn cell_span(edges: Option<&[f64]>, bounds: [f64; 2], count: usize, index: usize) -> [f64; 2] {
    if let Some(&[start, end]) = edges.and_then(|edges| edges.get(index..index + 2)) {
        return [start, end];
    }
    let step = (bounds[1] - bounds[0]) / count.max(1) as f64;
    let start = bounds[0] + index as f64 * step;
    [start, start + step]
}
