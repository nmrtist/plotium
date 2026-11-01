use super::{Candidate, CanvasDocument, ObjectFrame, PanelId};

pub(super) fn initial_image_size(page_size: [f32; 2], aspect: f32) -> [f32; 2] {
    let aspect = if aspect.is_finite() && aspect > 0.0 {
        aspect
    } else {
        1.0
    };
    let bounds = [
        (page_size[0] * 0.65).max(12.0),
        (page_size[1] * 0.65).max(12.0),
    ];
    if bounds[0] / bounds[1] > aspect {
        [bounds[1] * aspect, bounds[1]]
    } else {
        [bounds[0], bounds[0] / aspect]
    }
}

/// Page-relative for a page item, panel-relative inside a panel.
pub(super) fn image_frame(
    page: &CanvasDocument,
    panel: Option<PanelId>,
    candidate: &Candidate,
    position: [f32; 2],
) -> ObjectFrame {
    let panel = panel.and_then(|panel| page.panel(panel));
    let origin = panel.map_or([0.0, 0.0], |panel| [panel.frame.x, panel.frame.y]);
    let [x, y] = [position[0] - origin[0], position[1] - origin[1]];
    if candidate.format == "svg" {
        let bounds = panel.map_or(page.size_pt(), |panel| {
            [panel.frame.width, panel.frame.height]
        });
        let [width, height] = natural_vector_size(candidate.pixel_size, bounds);
        ObjectFrame::new(
            x.min(bounds[0] - width).max(0.0),
            y.min(bounds[1] - height).max(0.0),
            width,
            height,
        )
    } else {
        let aspect = candidate.pixel_size[0] as f32 / candidate.pixel_size[1].max(1) as f32;
        let [width, height] = initial_image_size(page.size_pt(), aspect);
        ObjectFrame::new(x, y, width, height)
    }
}

/// A vector image's size in points, whose nominal pixels are CSS pixels,
/// shrunk only when it exceeds `bounds`.
pub(super) fn natural_vector_size(pixel_size: [u32; 2], bounds: [f32; 2]) -> [f32; 2] {
    const POINTS_PER_PIXEL: f32 = 0.75;
    let natural = pixel_size.map(|edge| edge.max(1) as f32 * POINTS_PER_PIXEL);
    let scale = (bounds[0] / natural[0])
        .min(bounds[1] / natural[1])
        .min(1.0);
    natural.map(|edge| edge * scale)
}

pub(super) fn physical_image_size_mm(pixel_size: [u32; 2], dpi: Option<[f32; 2]>) -> [f32; 2] {
    const DEFAULT_DPI: f32 = 96.0;
    const MM_PER_INCH: f32 = 25.4;
    let source_dpi = dpi
        .map(|value| value[0])
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(DEFAULT_DPI);
    let aspect = pixel_size[0].max(1) as f32 / pixel_size[1].max(1) as f32;
    let natural_width = pixel_size[0].max(1) as f32 / source_dpi * MM_PER_INCH;
    let width = natural_width.min(plotium_core::state::NATURE_SINGLE_COLUMN.width_mm);
    [width, width / aspect]
}
