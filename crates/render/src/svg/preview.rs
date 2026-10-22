//! Preview-only SVG export. Dense curves are thinned to the raster the SVG is
//! rasterized into, so a whole-canvas preview stays bounded while looking the
//! same as the full export at that resolution. Final exports never use this.
use super::document::export_document_with_page;
use crate::{Document, Projector};
use plotium_figure::{Series, SeriesOrigin};
use std::collections::HashSet;

/// A curve the preview drew with fewer points than its source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewDownsample {
    /// Position in [`Document::items`].
    pub item: usize,
    /// Position in the plot's `Figure::series`.
    pub series: usize,
    pub origin: Option<SeriesOrigin>,
    pub source_points: usize,
    /// Line vertices for a line series, otherwise markers.
    pub drawn_points: usize,
}

pub struct PreviewSvg {
    pub svg: String,
    pub downsampled: Vec<PreviewDownsample>,
}

/// Envelope columns per raster pixel. With one min/max pair per pixel a
/// zoomed NMR baseline draws visibly lighter than the full trace, which
/// overdraws dozens of strokes per pixel; four halves that difference.
const COLUMNS_PER_PIXEL: f32 = 4.0;
/// Floor matching the screen renderer, so small plots keep their detail.
const MIN_LINE_COLUMNS: usize = 2_048;

/// Render the page for a raster of `pixels_per_pt` device pixels per page
/// point. A line keeps at most a min/max pair per envelope column of its plot
/// area; markers sharing one pixel are drawn once. A line whose visible part
/// fits that budget keeps every visible point. Curves within the budget are
/// written exactly as [`super::export_document`] writes them.
pub fn export_document_preview(document: &Document<'_>, pixels_per_pt: f32) -> PreviewSvg {
    let mut lod = PreviewLod {
        pixels_per_pt: if pixels_per_pt.is_finite() && pixels_per_pt > 0.0 {
            pixels_per_pt
        } else {
            1.0
        },
        item: 0,
        downsampled: Vec::new(),
    };
    let svg = export_document_with_page(
        document,
        None,
        [document.width, document.height],
        true,
        false,
        Some(&mut lod),
    );
    PreviewSvg {
        svg,
        downsampled: lod.downsampled,
    }
}

pub(crate) struct PreviewLod {
    pixels_per_pt: f32,
    item: usize,
    downsampled: Vec<PreviewDownsample>,
}

impl PreviewLod {
    pub(super) fn at_item(&mut self, item: usize) -> &mut Self {
        self.item = item;
        self
    }

    fn columns(&self, proj: &Projector<'_>) -> usize {
        ((proj.plot.width * self.pixels_per_pt * COLUMNS_PER_PIXEL).ceil() as usize)
            .max(MIN_LINE_COLUMNS)
    }

    fn within_budget(&self, proj: &Projector<'_>, points: usize) -> bool {
        points <= self.columns(proj).saturating_mul(2)
    }

    /// The points to draw and whether they were pooled, or `None` when the
    /// line is drawn as exported. Over budget, a line is clipped to its visible
    /// range (one neighbour each side), then pooled only if that is still dense.
    pub(super) fn line(
        &self,
        proj: &Projector<'_>,
        points: &[[f64; 2]],
    ) -> Option<(Vec<[f64; 2]>, bool)> {
        if self.within_budget(proj, points.len()) {
            return None;
        }
        let drawn = crate::line_lod::scaled_line_points(
            points,
            &proj.fig.x,
            !proj.y_axis().effective_scale().is_linear(),
            self.columns(proj),
        );
        Some((drawn.points.into_owned(), drawn.pooled))
    }

    /// Keep the first marker in each device pixel, in source order. Returns
    /// whether any marker was dropped.
    pub(super) fn markers(&self, proj: &Projector<'_>, points: &mut Vec<(f32, f32)>) -> bool {
        if self.within_budget(proj, points.len()) {
            return false;
        }
        let scale = self.pixels_per_pt;
        let mut seen = HashSet::new();
        let before = points.len();
        points
            .retain(|(x, y)| seen.insert(((x * scale).floor() as i64, (y * scale).floor() as i64)));
        points.len() < before
    }

    pub(super) fn record(&mut self, series: usize, source: &Series, drawn_points: usize) {
        self.downsampled.push(PreviewDownsample {
            item: self.item,
            series,
            origin: source.origin,
            source_points: source.points.len(),
            drawn_points,
        });
    }
}

#[cfg(test)]
#[path = "preview_tests.rs"]
mod tests;
