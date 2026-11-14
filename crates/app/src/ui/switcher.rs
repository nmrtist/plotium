use super::motion;
use egui::{Color32, CornerRadius, Id, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use plotium_core::state::PrimaryView;

const TRACK_HEIGHT: f32 = 34.0;
const SEGMENT_INSET: f32 = 3.0;

/// A segmented control with a sliding, spring-animated selection pill. Returns
/// `true` if the selection changed this frame.
pub fn segmented(ui: &mut Ui, current: &mut PrimaryView) -> bool {
    let dark = ui.visuals().dark_mode;
    let track_fill = if dark {
        Color32::from_white_alpha(20)
    } else {
        Color32::from_black_alpha(14)
    };
    let card_fill = if dark {
        ui.visuals().widgets.active.bg_fill
    } else {
        Color32::WHITE
    };
    let text_strong = ui.visuals().strong_text_color();
    let text_muted = ui.visuals().weak_text_color();

    let width = ui.available_width();
    let (track_rect, _) = ui.allocate_exact_size(Vec2::new(width, TRACK_HEIGHT), Sense::hover());
    let track_radius = CornerRadius::same(9);
    let segment_radius = CornerRadius::same(6);

    let painter = ui.painter().clone();
    painter.rect_filled(track_rect, track_radius, track_fill);

    let inner = track_rect.shrink(SEGMENT_INSET);
    let views = PrimaryView::all();
    let count = views.len() as f32;
    let slot_w = inner.width() / count;

    let active_index = views.iter().position(|v| v == current).unwrap_or(0);
    let dt = ui.input(|i| i.stable_dt);
    let pill_pos = motion::spring(
        ui.ctx(),
        super::viewport_local_id(ui.ctx(), "primary_view_pill"),
        active_index as f32,
        dt,
    );
    let card_rect = Rect::from_min_size(
        Pos2::new(inner.left() + pill_pos * slot_w, inner.top()),
        Vec2::new(slot_w, inner.height()),
    );
    // The pill floats like the chrome cards do: no hairline box, just a soft
    // drop shadow lifting it off the track.
    let pill_shadow = egui::epaint::Shadow {
        offset: [0, 1],
        blur: 4,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 96 } else { 30 }),
    };
    painter.add(pill_shadow.as_shape(card_rect, segment_radius));
    painter.rect(
        card_rect,
        segment_radius,
        card_fill,
        Stroke::NONE,
        egui::StrokeKind::Inside,
    );

    let mut changed = false;
    for (index, view) in views.iter().enumerate() {
        let left = egui::lerp(inner.left()..=inner.right(), index as f32 / count);
        let right = egui::lerp(inner.left()..=inner.right(), (index + 1) as f32 / count);
        let rect = Rect::from_min_max(
            Pos2::new(left, inner.top()),
            Pos2::new(right, inner.bottom()),
        );
        let response = ui.interact(
            rect,
            Id::new(("primary_view_segment", index)),
            Sense::click(),
        );
        let selected = *current == *view;

        let label_t = (1.0 - (pill_pos - index as f32).abs()).clamp(0.0, 1.0);
        let hover = motion::hover(
            ui.ctx(),
            response.id,
            !selected && label_t < 0.5 && response.hovered(),
        );
        if hover > 0.0 {
            painter.rect_filled(
                rect,
                segment_radius,
                Color32::from_white_alpha(10).gamma_multiply(hover),
            );
        }

        let galley = painter.layout_no_wrap(
            view.label().to_owned(),
            crate::typography::headline_font(),
            motion::lerp_color(text_muted, text_strong, label_t),
        );
        painter.galley(
            Pos2::new(
                rect.center().x - galley.size().x / 2.0,
                rect.center().y - galley.size().y / 2.0,
            ),
            galley,
            Color32::PLACEHOLDER,
        );

        if response.clicked() && !selected {
            *current = *view;
            changed = true;
        }
    }
    changed
}
