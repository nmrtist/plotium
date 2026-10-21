//! Text and shape overlays drawn in page space on the editor canvas.

use super::col;
use crate::{
    DocumentOverlay, DocumentViewport, OverlayAlign, OverlayKind, OverlayShape, OverlayShapeKind,
    OverlayText, Rect, arrow_head,
};
use egui::{Align2, FontId, Pos2, Shape, Stroke, StrokeKind, Vec2};

pub(super) fn paint_document_overlay(
    painter: &egui::Painter,
    page: Rect,
    overlay: &DocumentOverlay,
    viewport: DocumentViewport,
) {
    if !overlay.visible {
        return;
    }
    let frame = Rect::new(
        page.left + overlay.frame.left * viewport.zoom,
        page.top + overlay.frame.top * viewport.zoom,
        overlay.frame.width * viewport.zoom,
        overlay.frame.height * viewport.zoom,
    );
    match &overlay.kind {
        OverlayKind::Text(t) => paint_overlay_text(painter, frame, t, viewport.zoom),
        OverlayKind::Shape(s) => paint_overlay_shape(painter, frame, s, viewport.zoom),
    }
}

fn paint_overlay_text(painter: &egui::Painter, frame: Rect, t: &OverlayText, zoom: f32) {
    if t.text.trim().is_empty() {
        return;
    }
    let size = (t.font_size * zoom).max(6.0);
    let (x, anchor) = match t.align {
        OverlayAlign::Left => (frame.left, Align2::LEFT_TOP),
        OverlayAlign::Center => (frame.left + frame.width * 0.5, Align2::CENTER_TOP),
        OverlayAlign::Right => (frame.right(), Align2::RIGHT_TOP),
    };
    let font = FontId::proportional(size);
    let mut y = frame.top;
    for line in t.text.lines() {
        crate::rich_text::paint(
            painter,
            Pos2::new(x, y),
            anchor,
            line,
            font.clone(),
            col(t.color),
        );
        if t.bold {
            // egui has no bold weight; a hair-offset second pass fakes it.
            crate::rich_text::paint(
                painter,
                Pos2::new(x + 0.6, y),
                anchor,
                line,
                font.clone(),
                col(t.color),
            );
        }
        y += size * 1.25;
    }
}

fn paint_overlay_shape(painter: &egui::Painter, frame: Rect, s: &OverlayShape, zoom: f32) {
    let stroke = Stroke::new((s.stroke_width * zoom).max(0.5), col(s.stroke));
    let rect = egui::Rect::from_min_size(
        Pos2::new(frame.left, frame.top),
        Vec2::new(frame.width, frame.height),
    );
    match s.shape {
        OverlayShapeKind::Rect => {
            if let Some(fill) = s.fill {
                painter.rect_filled(rect, 0.0, col(fill));
            }
            painter.rect_stroke(rect, 0.0, stroke, StrokeKind::Inside);
        }
        OverlayShapeKind::Ellipse => {
            let pts = ellipse_points(frame);
            if let Some(fill) = s.fill {
                painter.add(Shape::convex_polygon(pts.clone(), col(fill), Stroke::NONE));
            }
            painter.add(Shape::closed_line(pts, stroke));
        }
        OverlayShapeKind::Line => {
            painter.line_segment(
                [
                    Pos2::new(frame.left, frame.top),
                    Pos2::new(frame.right(), frame.bottom()),
                ],
                stroke,
            );
        }
        OverlayShapeKind::Arrow => {
            let origin = (frame.left, frame.top);
            let tip = (frame.right(), frame.bottom());
            let [b1, b2] = arrow_head(origin, tip, zoom);
            painter.line_segment(
                [Pos2::new(origin.0, origin.1), Pos2::new(tip.0, tip.1)],
                stroke,
            );
            for barb in [b1, b2] {
                painter.line_segment([Pos2::new(tip.0, tip.1), Pos2::new(barb.0, barb.1)], stroke);
            }
        }
    }
}

fn ellipse_points(frame: Rect) -> Vec<Pos2> {
    let cx = frame.left + frame.width * 0.5;
    let cy = frame.top + frame.height * 0.5;
    let rx = frame.width * 0.5;
    let ry = frame.height * 0.5;
    (0..48)
        .map(|i| {
            let a = i as f32 / 48.0 * std::f32::consts::TAU;
            Pos2::new(cx + rx * a.cos(), cy + ry * a.sin())
        })
        .collect()
}
