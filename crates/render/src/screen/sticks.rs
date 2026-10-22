use super::*;

pub(super) fn paint(
    painter: &egui::Painter,
    projector: &Projector<'_>,
    series: &plotium_figure::Series,
    scale: f32,
) {
    let stroke = Stroke::new(series.width * scale, col(series.color));
    for point in &series.points {
        if let Some([(x, baseline), (_, y)]) = crate::stick_segment(projector, *point) {
            painter.line_segment([Pos2::new(x, baseline), Pos2::new(x, y)], stroke);
        }
    }
}
