use super::*;

fn pass(ctx: &egui::Context, time: f64, f: impl FnMut(&egui::Context)) {
    let mut f = f;
    let _ = ctx.run_ui(
        egui::RawInput {
            time: Some(time),
            ..Default::default()
        },
        |ui| f(ui.ctx()),
    );
}

#[test]
fn reduced_motion_lands_springs_and_disables_egui_animation() {
    let ctx = egui::Context::default();
    let id = Id::new("test_spring");
    seed_spring(&ctx, id, 0.0);
    sync(&ctx, ReduceMotion::Off);
    let moving = spring(&ctx, id, 10.0, 1.0 / 60.0);
    assert!(moving > 0.0 && moving < 10.0, "{moving}");

    sync(&ctx, ReduceMotion::On);
    assert!(reduced(&ctx));
    assert_eq!(spring(&ctx, id, 10.0, 1.0 / 60.0), 10.0);
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        assert_eq!(ctx.style_of(theme).animation_time, 0.0);
    }

    sync(&ctx, ReduceMotion::Off);
    assert!(!reduced(&ctx));
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        assert!(ctx.style_of(theme).animation_time > 0.0);
    }
}

#[test]
fn appear_restarts_after_a_surface_was_hidden() {
    let ctx = egui::Context::default();
    let id = Id::new("test_surface");
    let mut seen = Vec::new();
    for time in [0.0, 0.05, 1.0] {
        pass(&ctx, time, |ctx| seen.push(appear(ctx, id)));
    }
    assert_eq!(seen[0], 0.0);
    assert!(seen[1] > 0.0 && seen[1] < 1.0, "{seen:?}");
    assert_eq!(seen[2], 1.0);

    pass(&ctx, 2.0, |_| {});
    let mut reopened = 1.0;
    pass(&ctx, 3.0, |ctx| reopened = appear(ctx, id));
    assert_eq!(reopened, 0.0);

    sync(&ctx, ReduceMotion::On);
    pass(&ctx, 5.0, |_| {});
    let mut reduced_open = 0.0;
    pass(&ctx, 6.0, |ctx| reduced_open = appear(ctx, id));
    assert_eq!(reduced_open, 1.0);
}

#[test]
fn hover_jumps_when_motion_is_reduced() {
    let ctx = egui::Context::default();
    sync(&ctx, ReduceMotion::On);
    let mut value = 0.0;
    pass(&ctx, 0.0, |ctx| {
        value = hover(ctx, Id::new("test_hover"), true)
    });
    assert_eq!(value, 1.0);
}

#[test]
fn scale_about_keeps_the_pivot_fixed() {
    let pivot = egui::pos2(100.0, 50.0);
    let transform = scale_about(pivot, 0.5);
    assert_eq!(transform * pivot, pivot);
    assert_eq!(transform * egui::pos2(200.0, 50.0), egui::pos2(150.0, 50.0));
}

#[test]
fn reversing_a_slide_continues_from_where_it_is() {
    let ctx = egui::Context::default();
    let id = Id::new("test_slide");
    let mut time = 0.0;
    let mut step = |open: bool| {
        time += 1.0 / 60.0;
        let mut shown = 0.0;
        pass(&ctx, time, |ctx| shown = slide(ctx, id, open));
        shown
    };
    step(false);
    let mut last = step(true);
    for _ in 0..4 {
        last = step(true);
    }
    assert!(last > 0.0 && last < 1.0, "{last}");
    let reversed = step(false);
    assert!(
        reversed < last && last - reversed < 0.2,
        "{last} -> {reversed}"
    );
}
