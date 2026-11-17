use super::*;
use plotium_core::settings::{ReduceMotion, Settings};

const FRAME: f64 = 1.0 / 60.0;

struct Harness {
    ctx: egui::Context,
    app: PlotiumApp,
    time: f64,
    /// The next auto id after the sidebars, which later siblings start from.
    next_auto_id: Id,
}

impl Harness {
    fn new() -> Self {
        let mut app = PlotiumApp::new_with_settings(Settings::default());
        app.session.primary_sidebar_visible = true;
        app.session.secondary_sidebar_visible = false;
        app.session.primary_sidebar_width = 300.0;
        Self {
            ctx: crate::typography::test_context(),
            app,
            time: 0.0,
            next_auto_id: Id::NULL,
        }
    }

    fn frame(&mut self) {
        self.time += FRAME;
        let Self {
            ctx,
            app,
            time,
            next_auto_id,
        } = self;
        let _ = ctx.run_ui(
            egui::RawInput {
                time: Some(*time),
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1400.0, 900.0))),
                ..Default::default()
            },
            |ui| {
                render(app, ui, false, ui.available_width());
                *next_auto_id = ui.next_auto_id();
            },
        );
    }

    fn reserved(&self, primary: bool) -> bool {
        reserves_space(&self.ctx, primary)
    }

    fn set_visible(&mut self, primary: bool, visible: bool) {
        if primary {
            self.app.session.primary_sidebar_visible = visible;
        } else {
            self.app.session.secondary_sidebar_visible = visible;
        }
    }

    fn widths(&self) -> (f32, f32) {
        (
            self.app.session.primary_sidebar_width,
            self.app.session.secondary_sidebar_width,
        )
    }

    /// Runs frames until the sidebar's reserved space matches `reserved`,
    /// checking that no width is written meanwhile; returns the frame count.
    fn slide_until(&mut self, primary: bool, reserved: bool) -> usize {
        let widths = self.widths();
        let mut frames = 0;
        self.frame();
        while self.reserved(primary) != reserved {
            assert_eq!(self.widths(), widths);
            frames += 1;
            assert!(frames < 60, "the slide never ended");
            self.frame();
        }
        frames
    }
}

#[test]
fn placement_keeps_the_starting_layout_until_the_slide_ends() {
    assert_eq!(Placement::resolve(true, 1.0, false), Placement::Docked);
    assert_eq!(Placement::resolve(false, 0.0, true), Placement::Hidden);
    let opening = Placement::resolve(true, 0.4, false);
    assert_eq!(
        opening,
        Placement::Sliding {
            shown: 0.4,
            reserved: false
        }
    );
    assert!(!opening.reserves_space());
    let closing = Placement::resolve(false, 0.4, true);
    assert!(closing.reserves_space());
    // Reversing mid-slide keeps the layout the slide started from.
    assert!(Placement::resolve(true, 0.6, true).reserves_space());
    assert!(!Placement::resolve(false, 0.6, false).reserves_space());
}

#[test]
fn sliding_keeps_the_stored_width_and_relayouts_once_at_the_end() {
    for primary in [true, false] {
        let mut harness = Harness::new();
        harness.set_visible(primary, true);
        harness.frame();
        harness.frame();
        assert!(harness.reserved(primary));
        let docked = harness.widths();

        harness.set_visible(primary, false);
        let frames = harness.slide_until(primary, false);
        assert!(frames > 3, "closed in {frames} frames");

        harness.set_visible(primary, true);
        let frames = harness.slide_until(primary, true);
        assert!(frames > 3, "opened in {frames} frames");
        harness.frame();
        assert_eq!(harness.widths(), docked);
    }
}

#[test]
fn both_sidebars_slide_together_without_writing_widths() {
    let mut harness = Harness::new();
    harness.set_visible(false, true);
    harness.frame();
    harness.frame();
    let docked = harness.widths();
    harness.set_visible(true, false);
    harness.set_visible(false, false);
    harness.slide_until(true, false);
    assert!(!harness.reserved(false));
    harness.set_visible(true, true);
    harness.set_visible(false, true);
    harness.slide_until(true, true);
    assert!(harness.reserved(false));
    harness.frame();
    assert_eq!(harness.widths(), docked);
}

#[test]
fn reduced_motion_docks_and_hides_on_the_same_frame() {
    let mut harness = Harness::new();
    motion::sync(&harness.ctx, ReduceMotion::On);
    harness.frame();
    assert!(harness.reserved(true));
    harness.set_visible(true, false);
    harness.frame();
    assert!(!harness.reserved(true));
    harness.set_visible(true, true);
    harness.frame();
    assert!(harness.reserved(true));
}

#[test]
fn a_sliding_sidebar_paints_just_above_the_workspace_from_its_first_frame() {
    let mut harness = Harness::new();
    harness.frame();
    // Primed, so the first slide frame is not an invisible sizing pass.
    let area = sidebar_id(true).with("sliding");
    assert!(egui::AreaState::load(&harness.ctx, area).is_some());
    harness.set_visible(true, false);
    harness.frame();
    assert!(harness.reserved(true));
    let sliding = egui::LayerId::new(egui::Order::Background, sidebar_id(true).with("sliding"));
    let order: Vec<_> = harness.ctx.memory(|memory| memory.layer_ids().collect());
    let background = order
        .iter()
        .position(|layer| *layer == egui::LayerId::background());
    let overlay = order.iter().position(|layer| *layer == sliding);
    assert_eq!(overlay, background.map(|index| index + 1), "{order:?}");
}

#[test]
fn later_siblings_keep_their_auto_ids_through_a_slide() {
    let mut harness = Harness::new();
    harness.frame();
    let docked = harness.next_auto_id;
    harness.set_visible(true, false);
    harness.frame();
    assert!(harness.reserved(true));
    assert_eq!(harness.next_auto_id, docked);
    harness.slide_until(true, false);
    assert_eq!(harness.next_auto_id, docked);
    harness.set_visible(true, true);
    harness.frame();
    assert!(!harness.reserved(true));
    assert_eq!(harness.next_auto_id, docked);
}

#[test]
fn an_opening_sidebar_owns_the_pointer_over_its_visible_part() {
    let mut harness = Harness::new();
    harness.set_visible(true, false);
    harness.frame();
    harness.set_visible(true, true);
    harness.frame();
    harness.frame();
    assert!(!harness.reserved(true));
    let near_edge = Pos2::new(4.0, 450.0);
    let layer = harness.ctx.layer_id_at(near_edge);
    assert_eq!(
        layer,
        Some(egui::LayerId::new(
            egui::Order::Background,
            sidebar_id(true).with("sliding")
        ))
    );
    let beyond = Pos2::new(1000.0, 450.0);
    assert_eq!(
        harness.ctx.layer_id_at(beyond),
        Some(egui::LayerId::background())
    );
}
