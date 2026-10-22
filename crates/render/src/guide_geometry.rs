//! Guide observations reuse the exact renderer layout, not a second estimator.
use crate::{
    Figure, Projector, Rect, axis_layout_placed, color_scale_rect, legend_entries, legend_rect,
};

pub fn legend_entry_count(fig: &Figure) -> usize {
    legend_entries(fig).len()
}

#[derive(Debug)]
pub struct GuideGeometry {
    pub plot: Rect,
    pub legend: Option<Rect>,
    pub color_scale: Option<Rect>,
    pub entries: usize,
    pub not_rendered_reason: Option<&'static str>,
    pub available_travel: Option<[f32; 2]>,
    pub effective_position: Option<[f32; 2]>,
    pub fits_plot: Option<[bool; 2]>,
}

/// Physical object-local points at scale 1, matching screen and vector exports.
/// Bounds are renderer layout boxes; text widths use its existing estimate.
pub fn guide_geometry(fig: &Figure, size: [f32; 2]) -> GuideGeometry {
    guide_geometry_placed(fig, size, None)
}

/// Guide geometry of a plot that may sit in a shared-x stack.
pub fn guide_geometry_placed(
    fig: &Figure,
    size: [f32; 2],
    stack: Option<&crate::StackPlacement>,
) -> GuideGeometry {
    let layout = axis_layout_placed(fig, size[0], size[1], stack);
    let plot = Projector::new(fig, Rect::new(0., 0., size[0], size[1]), &layout.margins).plot;
    let legend = legend_rect(fig, plot, 1.);
    let entries = legend_entry_count(fig);
    let available_travel = legend.map(|r| {
        [
            (plot.width - r.width).max(0.),
            (plot.height - r.height).max(0.),
        ]
    });
    let effective_position = legend.zip(available_travel).map(|(r, [x, y])| {
        [
            if x > 0. { (r.left - plot.left) / x } else { 0. },
            if y > 0. { (r.top - plot.top) / y } else { 0. },
        ]
    });
    GuideGeometry {
        plot,
        legend,
        entries,
        available_travel,
        effective_position,
        color_scale: color_scale_rect(fig, plot, 1.),
        fits_plot: legend.map(|r| [r.width <= plot.width, r.height <= plot.height]),
        not_rendered_reason: if legend.is_some() {
            None
        } else if matches!(fig.guide_visibility, plotium_figure::GuideVisibility::Hide) {
            Some("hidden")
        } else if entries == 0 {
            Some("no_entries")
        } else {
            Some("auto_single_entry")
        },
    }
}

#[cfg(test)]
#[path = "guide_geometry_tests.rs"]
mod tests;
