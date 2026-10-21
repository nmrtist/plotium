use crate::curve_style::dashed_segments;
use crate::{Projector, Rect, TextAnchor, range_label_layout};
use egui::{Align2, Color32, Pos2, Stroke};
use plotium_figure::{Figure, LineStyle};

pub(crate) fn paint(
    painter: &egui::Painter,
    figure: &Figure,
    projector: &Projector,
    plot: Rect,
    scale: f32,
) {
    for annotation in &figure.range_annotations {
        let (x0, _) = projector.project([annotation.x0, figure.y.min]);
        let (x1, _) = projector.project([annotation.x1, figure.y.min]);
        let rect = egui::Rect::from_min_max(
            Pos2::new(x0.min(x1), plot.top),
            Pos2::new(x0.max(x1), plot.bottom()),
        );
        let color = Color32::from_rgb(annotation.color.r, annotation.color.g, annotation.color.b);
        let fill = Color32::from_rgba_unmultiplied(
            annotation.color.r,
            annotation.color.g,
            annotation.color.b,
            (annotation.fill_opacity.clamp(0.0, 1.0) * 255.0).round() as u8,
        );
        painter.rect_filled(rect, 0.0, fill);
        painter.rect_stroke(
            rect,
            0.0,
            Stroke::new(annotation.width * scale, color),
            egui::StrokeKind::Inside,
        );
        let Some(label) = range_label_layout(
            plot,
            rect.left(),
            rect.right(),
            figure.typography.tick_pt * scale,
            &annotation.label,
            annotation.label_position,
        ) else {
            continue;
        };
        crate::rich_text::paint(
            painter,
            Pos2::new(label.x, label.top),
            match label.anchor {
                TextAnchor::Left => Align2::LEFT_TOP,
                TextAnchor::Center => Align2::CENTER_TOP,
                TextAnchor::Right => Align2::RIGHT_TOP,
            },
            label.text,
            crate::rich_text::plot_font(figure.typography.tick_pt * scale, &figure.typography),
            color,
        );
    }
}

pub(crate) fn paint_reference_lines(
    painter: &egui::Painter,
    figure: &Figure,
    projector: &Projector,
    plot: Rect,
    scale: f32,
) {
    for item in crate::reference_lines::layout(figure, projector, plot, scale) {
        let line = item.line;
        let color = Color32::from_rgb(line.color.r, line.color.g, line.color.b);
        let stroke = Stroke::new(line.width * scale, color);
        let pos = |(x, y): (f32, f32)| Pos2::new(x, y);
        if line.line_style == LineStyle::Solid {
            painter.line_segment([pos(item.start), pos(item.end)], stroke);
        } else {
            for [a, b] in dashed_segments(
                &[item.start, item.end],
                line.line_style,
                line.width * scale,
                plot,
            ) {
                painter.line_segment([pos(a), pos(b)], stroke);
            }
        }
        if let Some(label) = item.label {
            crate::rich_text::paint(
                painter,
                Pos2::new(label.x, label.top),
                match label.anchor {
                    TextAnchor::Left => Align2::LEFT_TOP,
                    TextAnchor::Center => Align2::CENTER_TOP,
                    TextAnchor::Right => Align2::RIGHT_TOP,
                },
                label.text,
                crate::rich_text::plot_font(label.size, &figure.typography),
                color,
            );
        }
    }
}

/// Author arrows, mirroring [`crate::svg`]'s arrow output.
pub(crate) fn paint_arrows(
    painter: &egui::Painter,
    figure: &Figure,
    projector: &Projector,
    scale: f32,
) {
    let pos = |(x, y): (f32, f32)| Pos2::new(x, y);
    for arrow in &figure.author_arrows {
        let Some(shape) = crate::arrow_geometry::arrow_shape(
            projector.project(arrow.from),
            projector.project(arrow.to),
            arrow.width,
            scale,
            arrow.head,
        ) else {
            continue;
        };
        let color = Color32::from_rgb(arrow.color.r, arrow.color.g, arrow.color.b);
        let stroke = Stroke::new(arrow.width * scale, color);
        painter.line_segment(shape.shaft.map(pos), stroke);
        for tip in shape.tips {
            let points = tip.points.map(pos).to_vec();
            painter.add(if tip.filled {
                egui::Shape::convex_polygon(points, color, Stroke::NONE)
            } else {
                egui::Shape::line(points, stroke)
            });
        }
    }
}
