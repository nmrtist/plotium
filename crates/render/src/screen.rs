pub use crate::screen_stats::RenderStats;
use crate::{
    Document, DocumentItem, DocumentObject, DocumentViewport, OUTER_PAD, Projector, Rect,
    TICK_LABEL_PAD, axis_layout_placed, error_bar_segments, heatmap_cells, integral,
    minor_tick_values, polygon_outline, projection_points,
    screen_contours::paint_contours,
    screen_lod::{line_columns, scaled_screen_line_points},
};
use egui::{Align2, Color32, FontId, Pos2, Sense, Shape, Stroke, StrokeKind, Ui, Vec2};
use plotium_figure::{AxisFrame, AxisTrace, Color, Figure};

mod color_scale;
mod curves;
mod legend;
mod overlay;
mod sticks;
use overlay::paint_document_overlay;

/// Screen geometry fidelity. Interactive mode bounds costly geometry by pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScreenRenderDetail {
    #[default]
    Full,
    /// Temporarily omit sub-pixel detail during workspace camera movement.
    Interactive,
}

fn col(c: Color) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
}

/// Allocate space in `ui` and paint the whole figure.
pub fn show(ui: &mut Ui, fig: &Figure) {
    let avail = ui.available_size();
    let desired = Vec2::new(avail.x.max(320.0), avail.y.max(240.0));
    let (response, painter) = ui.allocate_painter(desired, Sense::hover());
    let r = response.rect;
    let outer = Rect::new(r.left(), r.top(), r.width(), r.height());
    paint(&painter, outer, fig, 1.0);
}

/// Paint a figure into an existing painter at the supplied page-to-screen scale.
pub fn paint(painter: &egui::Painter, outer: Rect, fig: &Figure, scale: f32) {
    paint_with_stats(painter, outer, fig, scale, None);
}

pub fn paint_with_stats(
    painter: &egui::Painter,
    outer: Rect,
    fig: &Figure,
    scale: f32,
    stats: Option<&mut RenderStats>,
) {
    paint_with_detail_and_stats(painter, outer, fig, scale, ScreenRenderDetail::Full, stats);
}

/// Paint one figure with an explicit screen detail level and optional counters.
pub fn paint_with_detail_and_stats(
    painter: &egui::Painter,
    outer: Rect,
    fig: &Figure,
    scale: f32,
    detail: ScreenRenderDetail,
    stats: Option<&mut RenderStats>,
) {
    paint_with_detail_and_stats_impl(
        painter,
        outer,
        fig,
        scale,
        detail,
        ObjectExtras::default(),
        stats,
    );
}

/// Per-object inputs a page supplies on top of the figure itself.
#[derive(Clone, Copy, Default)]
struct ObjectExtras {
    /// Runtime identity for editor-only geometry caches.
    geometry_generation: Option<u64>,
    /// Screen-space clearance for the y-axis title below the frame top.
    y_title_clearance: crate::YTitleClearance,
    /// Unscaled shared-x stack placement.
    stack: Option<crate::StackPlacement>,
}

fn paint_with_detail_and_stats_impl(
    painter: &egui::Painter,
    outer: Rect,
    fig: &Figure,
    scale: f32,
    detail: ScreenRenderDetail,
    extras: ObjectExtras,
    mut stats: Option<&mut RenderStats>,
) {
    let ObjectExtras {
        geometry_generation,
        y_title_clearance,
        stack,
    } = extras;
    let ty = fig.typography;
    let frame = outer;
    let (outer, y_title_clearance) = crate::y_title_area(fig, frame, y_title_clearance, scale);
    let layout = axis_layout_placed(
        fig,
        outer.width / scale,
        outer.height / scale,
        stack.as_ref(),
    );
    let xf = crate::XFurniture::of(fig, stack.as_ref());
    let margins = layout.margins.scaled(scale);
    let proj = Projector::new(fig, outer, &margins);
    let plot = proj.plot;

    let to_pos = |x: f32, y: f32| Pos2::new(x, y);

    painter.rect_filled(
        egui::Rect::from_min_size(
            Pos2::new(frame.left, frame.top + xf.background_inset * scale),
            Vec2::new(frame.width, frame.height - xf.background_inset * scale),
        ),
        0.0,
        col(fig.background),
    );

    if xf.show_title && !fig.title.trim().is_empty() {
        crate::rich_text::paint(
            painter,
            Pos2::new(
                frame.left + frame.width / 2.0,
                frame.top + (OUTER_PAD + ty.title_pt * 0.5) * scale,
            ),
            Align2::CENTER_CENTER,
            &fig.title,
            crate::rich_text::plot_font(ty.title_pt * scale, &ty),
            col(Color::BLACK),
        );
    }

    let hidden_frame = fig.axis_frame == AxisFrame::Hidden;
    let (x_ticks, y_ticks) = (layout.x_ticks, layout.y_ticks);

    if fig.show_grid && !hidden_frame {
        let grid_stroke = Stroke::new(1.0 * scale, col(Color::GRID));
        for &xt in &x_ticks.values {
            let (px, _) = proj.project([xt, fig.y.min]);
            painter.line_segment(
                [to_pos(px, plot.top), to_pos(px, plot.bottom())],
                grid_stroke,
            );
        }
        for &yt in &y_ticks.values {
            let (_, py) = proj.project([fig.x.min, yt]);
            painter.line_segment(
                [to_pos(plot.left, py), to_pos(plot.right(), py)],
                grid_stroke,
            );
        }
    }

    let axis_stroke = Stroke::new(ty.axis_line_pt * scale, col(Color::AXIS));
    let y_axis_x = proj.left_band.map(|band| band.left).unwrap_or(plot.left);
    match fig.axis_frame {
        AxisFrame::Open => {
            painter.line_segment(
                [
                    to_pos(plot.left, plot.bottom()),
                    to_pos(plot.right(), plot.bottom()),
                ],
                axis_stroke,
            );
            painter.line_segment(
                [to_pos(y_axis_x, plot.top), to_pos(y_axis_x, plot.bottom())],
                axis_stroke,
            );
        }
        AxisFrame::Box => {
            painter.rect_stroke(
                egui::Rect::from_min_size(
                    Pos2::new(plot.left, plot.top),
                    Vec2::new(plot.width, plot.height),
                ),
                0.0,
                axis_stroke,
                // Stacked boxes share boundary lines, which must coincide.
                if stack.is_some() {
                    StrokeKind::Middle
                } else {
                    StrokeKind::Inside
                },
            );
            if (y_axis_x - plot.left).abs() > f32::EPSILON {
                painter.line_segment(
                    [to_pos(y_axis_x, plot.top), to_pos(y_axis_x, plot.bottom())],
                    axis_stroke,
                );
            }
        }
        AxisFrame::Hidden => {}
    }

    let minor = ty.minor_tick_reach() * scale;
    for xt in minor_tick_values(&fig.x, &x_ticks, ty.minor_ticks) {
        let (px, _) = proj.project([xt, fig.y.min]);
        let ends = [
            to_pos(px, plot.bottom()),
            to_pos(px, plot.bottom() + xf.minor_tick_reach * scale),
        ];
        painter.line_segment(ends, axis_stroke);
    }
    for yt in minor_tick_values(&fig.y, &y_ticks, ty.minor_ticks) {
        let (_, py) = proj.project([fig.x.min, yt]);
        painter.line_segment(
            [to_pos(y_axis_x - minor, py), to_pos(y_axis_x, py)],
            axis_stroke,
        );
    }
    for (&xt, label) in x_ticks.values.iter().zip(&x_ticks.labels) {
        let (px, _) = proj.project([xt, fig.y.min]);
        painter.line_segment(
            [
                to_pos(px, plot.bottom()),
                to_pos(px, plot.bottom() + xf.tick_reach * scale),
            ],
            axis_stroke,
        );
        if xf.show_tick_labels {
            crate::rich_text::paint(
                painter,
                Pos2::new(
                    px,
                    plot.bottom() + (ty.tick_outset() + TICK_LABEL_PAD + ty.tick_pt * 0.5) * scale,
                ),
                Align2::CENTER_CENTER,
                label,
                crate::rich_text::plot_font(ty.tick_pt * scale, &ty),
                col(Color::AXIS),
            );
        }
    }
    // Keep F1 tick numbers outside a left projection band.
    let y_tick_x = y_axis_x - (ty.tick_outset() + TICK_LABEL_PAD) * scale;
    for (&yt, label) in y_ticks.values.iter().zip(&y_ticks.labels) {
        let (_, py) = proj.project([fig.x.min, yt]);
        painter.line_segment(
            [
                to_pos(y_axis_x - ty.tick_reach() * scale, py),
                to_pos(y_axis_x, py),
            ],
            axis_stroke,
        );
        if fig.y.show_tick_labels && xf.shows_y_label_at((py - plot.top) / scale) {
            crate::rich_text::paint(
                painter,
                Pos2::new(y_tick_x, py),
                Align2::RIGHT_CENTER,
                label,
                crate::rich_text::plot_font(ty.tick_pt * scale, &ty),
                col(Color::AXIS),
            );
        }
    }

    if fig.y.show_tick_labels
        && let Some(multiplier) = y_ticks.multiplier()
    {
        let (position, anchor) = if xf.y_multiplier_inside {
            let inset = (ty.tick_length_pt + TICK_LABEL_PAD) * scale;
            (
                Pos2::new(y_axis_x + inset, plot.top + TICK_LABEL_PAD * scale),
                Align2::LEFT_TOP,
            )
        } else {
            (
                Pos2::new(y_axis_x, plot.top - TICK_LABEL_PAD * scale),
                Align2::LEFT_BOTTOM,
            )
        };
        crate::rich_text::paint(
            painter,
            position,
            anchor,
            multiplier,
            crate::rich_text::plot_font(ty.tick_pt * scale, &ty),
            col(Color::AXIS),
        );
    }
    if xf.show_tick_labels
        && let Some(multiplier) = x_ticks.multiplier()
    {
        crate::rich_text::paint(
            painter,
            Pos2::new(
                plot.right(),
                outer.bottom() - (OUTER_PAD + ty.tick_pt * 0.5) * scale,
            ),
            Align2::RIGHT_CENTER,
            multiplier,
            crate::rich_text::plot_font(ty.tick_pt * scale, &ty),
            col(Color::AXIS),
        );
    }

    if !hidden_frame && xf.show_label {
        let multiplier_clearance = if xf.show_tick_labels {
            x_ticks.multiplier_clearance(ty.tick_pt)
        } else {
            0.0
        };
        crate::rich_text::paint(
            painter,
            Pos2::new(
                (plot.left + plot.right()) / 2.0,
                outer.bottom() - (OUTER_PAD + multiplier_clearance + ty.label_pt * 0.5) * scale,
            ),
            Align2::CENTER_CENTER,
            &fig.x.label,
            crate::rich_text::plot_font(ty.label_pt * scale, &ty),
            col(Color::AXIS),
        );
    }
    if !hidden_frame && fig.y.show_label {
        let galley = crate::rich_text::galley(
            painter,
            &fig.y.label,
            crate::rich_text::plot_font(ty.label_pt * scale, &ty),
            col(Color::AXIS),
        );
        let galley_size = galley.size();
        let mut y_label = egui::epaint::TextShape::new(
            Pos2::new(-galley_size.x * 0.5, -galley_size.y * 0.5),
            galley,
            col(Color::AXIS),
        )
        .with_angle_and_anchor(-std::f32::consts::FRAC_PI_2, Align2::CENTER_CENTER);
        y_label.pos += Vec2::new(
            outer.left + (OUTER_PAD + ty.label_pt * 0.5) * scale,
            crate::y_title_center(fig, plot, outer, y_title_clearance, scale),
        );
        painter.add(y_label);
    }

    let clip = egui::Rect::from_min_size(
        Pos2::new(plot.left, plot.top),
        Vec2::new(plot.width, plot.height),
    );
    let clipped = painter.with_clip_rect(clip);

    if let Some(grid) = &fig.heatmap {
        for (cell, color) in heatmap_cells(&proj, grid) {
            // Expand a hair so adjacent cells cannot show anti-aliasing seams.
            let rect = egui::Rect::from_min_size(
                Pos2::new(cell.left, cell.top),
                Vec2::new(cell.width, cell.height),
            )
            .expand(0.5);
            clipped.rect_filled(rect, 0.0, col(color));
        }
    }

    for poly in &fig.polygons {
        let Some(outline) = polygon_outline(&proj, poly) else {
            continue;
        };
        let pts: Vec<Pos2> = outline.iter().map(|&(x, y)| to_pos(x, y)).collect();
        let fill = Color32::from_rgba_unmultiplied(
            poly.fill.r,
            poly.fill.g,
            poly.fill.b,
            (poly.opacity.clamp(0.0, 1.0) * 255.0).round() as u8,
        );
        let stroke = poly
            .stroke
            .map(|(c, w)| Stroke::new(w * scale, col(c)))
            .unwrap_or(Stroke::NONE);
        clipped.add(Shape::convex_polygon(pts, fill, stroke));
    }

    crate::screen_annotations::paint(&clipped, fig, &proj, plot, scale);
    crate::screen_annotations::paint_reference_lines(&clipped, fig, &proj, plot, scale);

    paint_contours(
        painter,
        &clipped,
        fig,
        &proj,
        plot,
        scale,
        detail,
        geometry_generation,
        &mut stats,
    );

    paint_error_bars(&clipped, &proj, fig, scale, false);

    for series in &fig.series {
        curves::paint(&clipped, &proj, series, scale, detail, stats.as_deref_mut());
    }
    paint_error_bars(&clipped, &proj, fig, scale, true);

    for curve in integral::layout(fig, plot, scale) {
        if curve.points.len() >= 2 {
            let points = curve.points.iter().map(|&(x, y)| to_pos(x, y)).collect();
            clipped.add(Shape::line(
                points,
                Stroke::new(curve.width * scale, col(curve.color)),
            ));
        }
        let galley = crate::rich_text::galley(
            &clipped,
            &curve.label.text,
            crate::rich_text::plot_font(curve.label.font_size, &ty),
            col(curve.label.color),
        );
        let size = galley.size();
        let mut label = egui::epaint::TextShape::new(
            Pos2::new(-size.x * 0.5, -size.y * 0.5),
            galley,
            col(curve.label.color),
        )
        .with_angle_and_anchor(-std::f32::consts::FRAC_PI_2, Align2::CENTER_CENTER);
        label.pos += Vec2::new(curve.label.position.0, curve.label.position.1);
        clipped.add(label);
    }

    crate::screen_annotations::paint_arrows(&clipped, fig, &proj, scale);
    for a in fig.text_annotations() {
        let (px, py) = proj.project(a.at);
        if !crate::is_finite_point((px, py)) {
            continue;
        }
        crate::rich_text::paint(
            &clipped,
            Pos2::new(px, py),
            Align2::CENTER_BOTTOM,
            &a.text,
            crate::rich_text::plot_font(a.size * scale, &ty),
            col(a.color),
        );
    }

    if let (Some(trace), Some(band)) = (&fig.top_projection, proj.top_band) {
        paint_projection(painter, fig, trace, plot, band, true, scale);
    }
    if let (Some(trace), Some(band)) = (&fig.left_projection, proj.left_band) {
        paint_projection(painter, fig, trace, plot, band, false, scale);
    }

    legend::paint(painter, plot, fig, scale);
    color_scale::paint(painter, plot, fig, scale);
}

fn paint_error_bars(
    painter: &egui::Painter,
    proj: &Projector<'_>,
    fig: &Figure,
    scale: f32,
    draw_over_data: bool,
) {
    for error_bar in &fig.error_bars {
        if error_bar.draw_over_data != draw_over_data {
            continue;
        }
        let Some(segments) = error_bar_segments(proj, error_bar, scale) else {
            continue;
        };
        let stroke = Stroke::new(error_bar.width * scale, col(error_bar.color));
        for &[start, end] in segments.as_slice() {
            painter.line_segment(
                [Pos2::new(start.0, start.1), Pos2::new(end.0, end.1)],
                stroke,
            );
        }
    }
}

fn paint_projection(
    painter: &egui::Painter,
    fig: &Figure,
    trace: &AxisTrace,
    plot: Rect,
    band: Rect,
    along_x: bool,
    scale: f32,
) {
    let band_rect = egui::Rect::from_min_size(
        Pos2::new(band.left, band.top),
        Vec2::new(band.width, band.height),
    );
    // A hairline seats the band against the contour's shared edge.
    let seam = Stroke::new(0.75 * scale, col(Color::AXIS));
    if along_x {
        painter.line_segment(
            [
                Pos2::new(band.left, band.bottom()),
                Pos2::new(band.right(), band.bottom()),
            ],
            seam,
        );
    } else {
        painter.line_segment(
            [
                Pos2::new(band.right(), band.top),
                Pos2::new(band.right(), band.bottom()),
            ],
            seam,
        );
    }
    let pts: Vec<Pos2> = projection_points(fig, trace, plot, band, along_x)
        .into_iter()
        .map(|(x, y)| Pos2::new(x, y))
        .collect();
    if pts.len() < 2 {
        return;
    }
    painter.with_clip_rect(band_rect).add(Shape::line(
        pts,
        Stroke::new(trace.width * scale, col(trace.color)),
    ));
}

/// Paint a fixed-size page document through a zoomable screen viewport.
pub fn paint_document(
    painter: &egui::Painter,
    screen: Rect,
    document: &Document<'_>,
    viewport: DocumentViewport,
) {
    paint_document_impl(
        painter,
        screen,
        document,
        viewport,
        true,
        ScreenRenderDetail::Full,
        None,
    );
}

/// Paint the editable board representation. The page background stays bounded,
/// while document items may remain visible outside the page so users can
/// recover and reposition temporarily overflowing content.
pub fn paint_document_for_editor(
    painter: &egui::Painter,
    screen: Rect,
    document: &Document<'_>,
    viewport: DocumentViewport,
) {
    paint_document_for_editor_with_detail(
        painter,
        screen,
        document,
        viewport,
        ScreenRenderDetail::Full,
    );
}

/// Paint an editable document using the requested screen detail level.
pub fn paint_document_for_editor_with_detail(
    painter: &egui::Painter,
    screen: Rect,
    document: &Document<'_>,
    viewport: DocumentViewport,
    detail: ScreenRenderDetail,
) {
    paint_document_for_editor_with_detail_and_stats(
        painter, screen, document, viewport, detail, None,
    );
}

/// Paint an editable document with explicit detail and optional counters.
pub fn paint_document_for_editor_with_detail_and_stats(
    painter: &egui::Painter,
    screen: Rect,
    document: &Document<'_>,
    viewport: DocumentViewport,
    detail: ScreenRenderDetail,
    stats: Option<&mut RenderStats>,
) {
    paint_document_impl(painter, screen, document, viewport, false, detail, stats);
}

pub fn paint_document_with_stats(
    painter: &egui::Painter,
    screen: Rect,
    document: &Document<'_>,
    viewport: DocumentViewport,
    stats: Option<&mut RenderStats>,
) {
    paint_document_impl(
        painter,
        screen,
        document,
        viewport,
        true,
        ScreenRenderDetail::Full,
        stats,
    );
}

fn paint_document_impl(
    painter: &egui::Painter,
    screen: Rect,
    document: &Document<'_>,
    viewport: DocumentViewport,
    clip_items_to_page: bool,
    detail: ScreenRenderDetail,
    mut stats: Option<&mut RenderStats>,
) {
    if let Some(stats) = stats.as_deref_mut() {
        stats.record_document(detail);
    }
    let page = Rect::new(
        screen.left + viewport.pan[0],
        screen.top + viewport.pan[1],
        document.width * viewport.zoom,
        document.height * viewport.zoom,
    );
    let page_rect = egui::Rect::from_min_size(
        Pos2::new(page.left, page.top),
        Vec2::new(page.width, page.height),
    );
    let page_painter = painter.with_clip_rect(page_rect);
    page_painter.rect_filled(page_rect, 0.0, col(document.background));
    let item_painter = if clip_items_to_page {
        page_painter
    } else {
        painter.clone()
    };

    for item in &document.items {
        match item {
            DocumentItem::Plot(object) => paint_document_object(
                &item_painter,
                page,
                object,
                viewport,
                detail,
                stats.as_deref_mut(),
            ),
            DocumentItem::Overlay(overlay) => {
                paint_document_overlay(&item_painter, page, overlay, viewport)
            }
            DocumentItem::Raster(raster) => {
                paint_document_raster(&item_painter, page, raster, viewport)
            }
            DocumentItem::PanelLabel {
                frame,
                text,
                visible,
            } if *visible => {
                let pos = Pos2::new(
                    page.left + (frame.left + text.position[0]) * viewport.zoom,
                    page.top + (frame.top + text.position[1]) * viewport.zoom,
                );
                let font = FontId::proportional((text.font_size * viewport.zoom).max(6.0));
                let color = crate::screen_raster::contrasting_label_color(
                    &document.items,
                    frame,
                    text,
                    document.background,
                );
                crate::rich_text::paint(
                    &item_painter,
                    pos,
                    Align2::LEFT_TOP,
                    &text.text,
                    font.clone(),
                    col(color),
                );
                crate::rich_text::paint(
                    &item_painter,
                    pos + Vec2::new(0.6, 0.0),
                    Align2::LEFT_TOP,
                    &text.text,
                    font,
                    col(color),
                );
            }
            DocumentItem::PanelLabel { .. } => {}
        }
    }
}

use crate::screen_raster::paint_document_raster;

fn paint_document_object(
    painter: &egui::Painter,
    page: Rect,
    object: &DocumentObject,
    viewport: DocumentViewport,
    detail: ScreenRenderDetail,
    stats: Option<&mut RenderStats>,
) {
    if !object.visible {
        return;
    }
    let frame = Rect::new(
        page.left + object.frame.left * viewport.zoom,
        page.top + object.frame.top * viewport.zoom,
        object.frame.width * viewport.zoom,
        object.frame.height * viewport.zoom,
    );
    paint_with_detail_and_stats_impl(
        painter,
        frame,
        object.figure,
        viewport.zoom,
        detail,
        ObjectExtras {
            geometry_generation: object.geometry_generation,
            y_title_clearance: object.y_title_clearance.scaled(viewport.zoom),
            stack: object.stack,
        },
        stats,
    );
    if let Some(title) = &object.title {
        let pos = Pos2::new(
            frame.left + title.position[0] * viewport.zoom,
            frame.top + title.position[1] * viewport.zoom,
        );
        let font = FontId::proportional((title.font_size * viewport.zoom).max(6.0));
        crate::rich_text::paint(
            painter,
            pos,
            Align2::LEFT_TOP,
            &title.text,
            font.clone(),
            col(Color::BLACK),
        );
        crate::rich_text::paint(
            painter,
            pos + Vec2::new(0.6, 0.0),
            Align2::LEFT_TOP,
            &title.text,
            font,
            col(Color::BLACK),
        );
    }
}

#[cfg(test)]
#[path = "screen_tests.rs"]
mod tests;
