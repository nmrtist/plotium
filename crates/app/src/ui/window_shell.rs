//! Window shell independent of any one window's state: viewport decorations,
//! the two surface fills, and the chrome that meets the window edges.

use egui::{Color32, Margin, Rect, Stroke, Ui};

/// Windows and Linux resize the undecorated window from zones this deep along
/// each edge and this square at each corner (`title_bar::resize_zones`).
pub(crate) const RESIZE_EDGE: f32 = 5.0;
pub(crate) const RESIZE_CORNER: f32 = 12.0;

/// Content inset of edge chrome: clear of the resize edges by at least 1 pt,
/// and of the corner zones along the bottom.
pub(crate) const EDGE_INSET: Margin = Margin {
    left: 8,
    right: 8,
    top: 8,
    bottom: 12,
};

const _: () = assert!(
    EDGE_INSET.left as f32 >= RESIZE_EDGE + 1.0
        && EDGE_INSET.right as f32 >= RESIZE_EDGE + 1.0
        && EDGE_INSET.bottom as f32 >= RESIZE_CORNER
);

pub(crate) fn configure_viewport(viewport: egui::ViewportBuilder) -> egui::ViewportBuilder {
    // Windows and Linux draw a VS Code style title bar inside the content area;
    // macOS keeps the native traffic lights and system menu.
    #[cfg(not(target_os = "macos"))]
    let viewport = viewport.with_decorations(false);
    #[cfg(target_os = "macos")]
    let viewport = viewport
        .with_fullsize_content_view(true)
        .with_titlebar_shown(false)
        .with_title_shown(false)
        .with_titlebar_buttons_shown(true);
    viewport
}

/// The recessed surface behind the board. Fills stay near-neutral so the
/// chrome never skews colour judgement of plot data.
pub(crate) fn workspace_fill(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(21, 22, 24)
    } else {
        Color32::from_rgb(243, 244, 246)
    }
}

/// The raised surface of the title bar, Ribbon, sidebars, and task cards.
pub(crate) fn chrome_fill(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(34, 35, 38)
    } else {
        Color32::WHITE
    }
}

const SEPARATOR_WIDTH: f32 = 1.0;

pub(crate) fn separator_stroke(dark: bool) -> Stroke {
    let color = if dark {
        Color32::from_white_alpha(20)
    } else {
        Color32::from_black_alpha(26)
    };
    Stroke::new(SEPARATOR_WIDTH, color)
}

/// Chrome that meets the window edges: no outer margin, corners, or shadow.
/// Neighbouring surfaces are divided by [`paint_separator`] instead.
pub(crate) fn edge_frame(dark: bool, inner_margin: Margin) -> egui::Frame {
    egui::Frame::new()
        .fill(chrome_fill(dark))
        .inner_margin(inner_margin)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Side {
    Left,
    Right,
    Bottom,
}

/// A 1 px divider drawn inside `rect` along `side`, so it never spills onto
/// the neighbouring surface.
pub(crate) fn paint_separator(ui: &Ui, rect: Rect, side: Side, dark: bool) {
    let stroke = separator_stroke(dark);
    let painter = ui.painter_at(rect);
    let ppp = painter.pixels_per_point();
    match side {
        Side::Left => painter.vline(
            separator_center(rect.left(), 1.0, ppp),
            rect.y_range(),
            stroke,
        ),
        Side::Right => painter.vline(
            separator_center(rect.right(), -1.0, ppp),
            rect.y_range(),
            stroke,
        ),
        Side::Bottom => painter.hline(
            rect.x_range(),
            separator_center(rect.bottom(), -1.0, ppp),
            stroke,
        ),
    };
}

/// Centre of a divider drawn just inside `edge`, toward `inward` (+1 or -1),
/// snapped with the tessellator's own rule so the line does not move again
/// when tessellated; overlays drawn on the divider use the same centre.
pub(crate) fn separator_center(edge: f32, inward: f32, pixels_per_point: f32) -> f32 {
    let mut center = edge + inward * SEPARATOR_WIDTH / 2.0;
    Stroke::new(SEPARATOR_WIDTH, Color32::WHITE)
        .round_center_to_pixel(pixels_per_point, &mut center);
    center
}

/// Linux compositors draw no shadow or border around an undecorated window,
/// so a 1 px outline separates it from same-coloured windows behind it.
/// Windows restores its DWM frame in `main.rs`; macOS keeps the native frame.
#[cfg(not(any(target_os = "macos", windows)))]
pub(crate) fn paint_window_outline(ctx: &egui::Context, dark: bool) {
    let (maximized, fullscreen) = ctx.input(|input| {
        let viewport = input.viewport();
        (
            viewport.maximized.unwrap_or(false),
            viewport.fullscreen.unwrap_or(false),
        )
    });
    if maximized || fullscreen {
        return;
    }
    let color = if dark {
        Color32::from_white_alpha(36)
    } else {
        Color32::from_black_alpha(56)
    };
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("plotium_window_outline"),
    ))
    .rect_stroke(
        ctx.content_rect(),
        0.0,
        Stroke::new(1.0_f32, color),
        egui::StrokeKind::Inside,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separator_stays_inside_pixel_aligned_edges_without_resnapping() {
        for ppp in [1.0_f32, 1.5, 2.0] {
            let pixel = 1.0 / ppp;
            for edge in [240.0, 240.0 + pixel, 0.0] {
                for inward in [-1.0, 1.0] {
                    let center = separator_center(edge, inward, ppp);
                    let near = center - inward * SEPARATOR_WIDTH / 2.0;
                    let far = center + inward * SEPARATOR_WIDTH / 2.0;
                    let inside = (near - edge) * inward >= -1e-4 && (far - edge) * inward > 0.0;
                    assert!(
                        inside,
                        "ppp={ppp} edge={edge} inward={inward} center={center}"
                    );
                    assert_eq!(
                        separator_center(center - inward * SEPARATOR_WIDTH / 2.0, inward, ppp),
                        center
                    );
                }
            }
        }
    }

    #[test]
    fn edge_frame_has_no_card_geometry() {
        for dark in [false, true] {
            let frame = edge_frame(dark, EDGE_INSET);
            assert_eq!(frame.outer_margin, Margin::ZERO);
            assert_eq!(frame.corner_radius, egui::CornerRadius::ZERO);
            assert_eq!(frame.shadow, egui::epaint::Shadow::NONE);
            assert_eq!(frame.stroke, Stroke::NONE);
            assert_ne!(frame.fill, workspace_fill(dark));
        }
    }
}
