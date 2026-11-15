use super::*;
use egui::{Event, FullOutput, Modifiers, Pos2, RawInput, Rect, ViewportCommand, ViewportId};

struct Titlebar {
    ctx: egui::Context,
    app: PlotiumApp,
    clipboard: ClipboardTablePaste,
    time: f64,
    width: f32,
    traffic_lights: Option<Vec2>,
    maximized: bool,
    fullscreen: bool,
}

impl Titlebar {
    fn new() -> Self {
        let mut app = PlotiumApp::new_with_settings(Default::default());
        app.doc.dirty = true;
        Self {
            ctx: crate::typography::test_context(),
            app,
            clipboard: ClipboardTablePaste::default(),
            time: 0.0,
            width: 1500.0,
            traffic_lights: Some(vec2(76.0, 28.0)),
            maximized: false,
            fullscreen: false,
        }
    }

    fn frame(&mut self, events: Vec<Event>) -> FullOutput {
        self.time += 0.05;
        let mut input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(self.width, 300.0))),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let viewport = input.viewports.get_mut(&ViewportId::ROOT).unwrap();
        viewport.maximized = Some(self.maximized);
        viewport.fullscreen = Some(self.fullscreen);
        let chrome = self
            .traffic_lights
            .map_or_else(super::super::RibbonChrome::default, |size| {
                super::super::RibbonChrome::macos(size, self.fullscreen)
            });
        self.ctx.run_ui(input, |ui| {
            task_row(&mut self.app, &mut self.clipboard, ui, chrome);
        })
    }

    fn text_center(&mut self, text: &str) -> Pos2 {
        self.frame(vec![]);
        self.frame(vec![])
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(shape) if shape.galley.text() == text => {
                    Some(shape.pos + shape.galley.size() * 0.5)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing title-row text: {text}"))
    }

    fn press(&mut self, pos: Pos2, button: PointerButton, pressed: bool) -> FullOutput {
        self.frame(vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers: Modifiers::NONE,
            },
        ])
    }

    fn double_click(&mut self, pos: Pos2, button: PointerButton) -> Vec<ViewportCommand> {
        self.frame(vec![Event::PointerMoved(pos)]);
        let mut commands = Vec::new();
        for _ in 0..2 {
            commands.extend(window_commands(self.press(pos, button, true)));
            commands.extend(window_commands(self.press(pos, button, false)));
        }
        commands
    }
}

fn window_commands(output: FullOutput) -> Vec<ViewportCommand> {
    output.viewport_output[&ViewportId::ROOT].commands.clone()
}

#[test]
fn titlebar_project_title_double_click_maximizes_and_restores_without_selection() {
    for maximized in [false, true] {
        let mut bar = Titlebar::new();
        bar.maximized = maximized;
        let pos = bar.text_center("* Untitled");
        let commands = bar.double_click(pos, PointerButton::Primary);
        assert_eq!(commands, vec![ViewportCommand::Maximized(!maximized)]);
        assert_eq!(
            bar.ctx
                .with_plugin(|state: &mut egui::text_selection::LabelSelectionState| {
                    state.has_selection()
                }),
            Some(false)
        );
    }
}

#[test]
fn titlebar_empty_space_double_click_maximizes() {
    let mut bar = Titlebar::new();
    // The gap after the task tabs, before the trailing controls.
    let pos = bar.text_center("View") + vec2(360.0, 0.0);
    let commands = bar.double_click(pos, PointerButton::Primary);
    assert_eq!(commands, vec![ViewportCommand::Maximized(true)]);
}

#[test]
fn titlebar_drag_still_moves_window_from_title_and_empty_space() {
    for title in [true, false] {
        let mut bar = Titlebar::new();
        let pos = if title {
            bar.text_center("* Untitled")
        } else {
            bar.text_center("View") + vec2(360.0, 0.0)
        };
        bar.frame(vec![Event::PointerMoved(pos)]);
        assert!(window_commands(bar.press(pos, PointerButton::Primary, true)).is_empty());
        let commands = window_commands(bar.frame(vec![Event::PointerMoved(pos + vec2(20.0, 0.0))]));
        assert_eq!(commands, vec![ViewportCommand::StartDrag]);
    }
}

#[test]
fn titlebar_secondary_double_click_does_not_resize() {
    let mut bar = Titlebar::new();
    let pos = bar.text_center("* Untitled");
    assert!(bar.double_click(pos, PointerButton::Secondary).is_empty());
}

#[test]
fn titlebar_task_tab_double_click_does_not_resize_window() {
    let mut bar = Titlebar::new();
    let pos = bar.text_center("Figure");
    assert!(bar.double_click(pos, PointerButton::Primary).is_empty());
    assert_eq!(bar.app.session.ui.ribbon_tab, WorkflowTab::Figure);
}

#[test]
fn titlebar_double_click_does_not_change_fullscreen_window() {
    let mut bar = Titlebar::new();
    bar.fullscreen = true;
    let pos = bar.text_center("* Untitled");
    assert!(bar.double_click(pos, PointerButton::Primary).is_empty());
}

#[test]
#[cfg(target_os = "windows")]
fn narrow_ribbon_moves_controls_below_clickable_tabs() {
    let mut bar = Titlebar::new();
    bar.traffic_lights = None;
    bar.width = 480.0;
    let view = bar.text_center("View");
    let search = bar.text_center(icon::MAGNIFYING_GLASS);
    assert!(search.y > view.y + 10.0);
    assert!(view.x < bar.width && search.x < bar.width);
    bar.press(view, PointerButton::Primary, true);
    bar.press(view, PointerButton::Primary, false);
    assert_eq!(bar.app.session.ui.ribbon_tab, WorkflowTab::View);

    bar.width = 1500.0;
    let view = bar.text_center("View");
    let search = bar.text_center(&format!("{} Search commands", icon::MAGNIFYING_GLASS));
    assert!((search.y - view.y).abs() < 2.0);
    assert!(search.x > view.x);
}

#[test]
#[cfg(target_os = "windows")]
fn ribbon_wraps_close_to_the_last_tab() {
    let mut bar = Titlebar::new();
    bar.traffic_lights = None;
    bar.app.session.ui.ribbon_tab = WorkflowTab::View;

    // Start wide, then find the first split during a resize. Measure the
    // actual gap in the last single row, with View's highlight showing its edge.
    let mut last_gap = None;
    for width in (480..=760).rev().step_by(2) {
        bar.width = width as f32;
        let output = bar.frame(vec![]);
        let pill = output.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| match shape {
                egui::Shape::Rect(rect) => Some(rect.rect),
                _ => None,
            }),
            _ => None,
        });
        let mut toggles = output.shapes.iter().filter_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect) if rect.rect.size() == vec2(16.0, 12.0) => Some(rect.rect),
            _ => None,
        });
        let (Some(pill), Some(left_toggle)) = (pill, toggles.next_back()) else {
            continue;
        };
        if left_toggle.center().y > pill.bottom() {
            let gap = last_gap.expect("a one-row layout before the split");
            assert!((0.0..=16.0).contains(&gap), "gap before wrapping: {gap}");
            return;
        }
        // The sidebar glyph is centered in a wider clickable button.
        let glyph_inset = (chrome_layout::SIDEBAR_TOGGLE_WIDTH - left_toggle.width()) / 2.0;
        last_gap = Some(left_toggle.left() - glyph_inset - pill.right());
    }
    panic!("Ribbon did not wrap in the tested width range");
}
