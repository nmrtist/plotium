use super::*;
use egui::{Event, FullOutput, Modifiers, Pos2, RawInput, Rect, ViewportCommand, ViewportId};

struct Titlebar {
    ctx: egui::Context,
    app: PlotiumApp,
    clipboard: ClipboardTablePaste,
    time: f64,
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
            maximized: false,
            fullscreen: false,
        }
    }

    fn frame(&mut self, events: Vec<Event>) -> FullOutput {
        self.time += 0.05;
        let mut input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1500.0, 300.0))),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let viewport = input.viewports.get_mut(&ViewportId::ROOT).unwrap();
        viewport.maximized = Some(self.maximized);
        viewport.fullscreen = Some(self.fullscreen);
        self.ctx.run_ui(input, |ui| {
            task_row(
                &mut self.app,
                &mut self.clipboard,
                ui,
                super::super::RibbonChrome::macos(vec2(76.0, 28.0), self.fullscreen),
            );
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
