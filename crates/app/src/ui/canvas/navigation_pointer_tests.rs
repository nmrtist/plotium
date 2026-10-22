use super::*;

fn fixture() -> (PlotiumApp, ObjectId, EguiRect, PlotRect, EguiRect) {
    let (mut app, ids) = crate::ui::properties::fixture::contour_page(1);
    app.session.board = BoardViewport {
        zoom: 1.0,
        world_center: [500.0, 400.0],
    };
    let screen = EguiRect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 800.0));
    let id = ids[0];
    let plot = plot_inner_rect(&app, 0, id, screen).unwrap();
    let outer = plot_rect(
        content_screen_rect(app.session.board, &app.doc.canvases[0], id, screen).unwrap(),
    );
    (app, id, screen, plot, outer)
}

fn input(pointer: Pos2, events: Vec<egui::Event>) -> egui::RawInput {
    egui::RawInput {
        events: std::iter::once(egui::Event::PointerMoved(pointer))
            .chain(events)
            .collect(),
        ..Default::default()
    }
}

fn button(p: Pos2, button: egui::PointerButton, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos: p,
        button,
        pressed,
        modifiers: Default::default(),
    }
}

fn space() -> egui::Event {
    egui::Event::Key {
        key: egui::Key::Space,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }
}

#[test]
fn axis_hover_and_locked_drag_match_single_axis_zoom_for_each_tool() {
    for axis in [ZoomAxis::X, ZoomAxis::Y] {
        for tool in [Tool::Select, Tool::BrowseZoom, Tool::Regions, Tool::Text] {
            let (mut app, id, screen, plot, outer) = fixture();
            app.set_tool(tool);
            let p = match axis {
                ZoomAxis::X => Pos2::new(
                    plot.left + plot.width * 0.25,
                    (plot.bottom() + outer.bottom()) * 0.5,
                ),
                _ => Pos2::new(
                    (outer.left() + plot.left) * 0.5,
                    plot.top + plot.height * 0.25,
                ),
            };
            let before = app.doc.canvases[0]
                .object(id)
                .unwrap()
                .plot()
                .unwrap()
                .viewport
                .clone();
            let ctx = egui::Context::default();
            let output = ctx.run_ui(input(p, vec![]), |ui| {
                assert_eq!(
                    pointer_navigation(&app, 0, screen, p, ui),
                    Some(PointerNavigation::Zoom(id, axis))
                );
                assert_eq!(
                    navigation_cursor(&app, 0, screen, ui),
                    Some((egui::CursorIcon::Crosshair, axis_zoom_hint(axis)))
                );
                canvas_cursor(&app, 0, screen, ui);
            });
            assert_eq!(
                output.platform_output.cursor_icon,
                egui::CursorIcon::Crosshair
            );
            let _ = ctx.run_ui(input(p, vec![button(p, egui::PointerButton::Primary, true)]), |ui| {
                assert!(handle_navigation(&mut app, 0, screen, ui));
                assert!(matches!(app.interaction(), Interaction::Zoom(d) if d.object == id && d.axis == axis));
            });
            // Leaving both the axis and canvas, and pressing Space, must not
            // replace the locked zoom with a pan cursor or target.
            let outside = screen.max + egui::vec2(20.0, 20.0);
            let output = ctx.run_ui(input(outside, vec![space()]), |ui| {
                assert!(handle_navigation(&mut app, 0, screen, ui));
                canvas_cursor(&app, 0, screen, ui);
            });
            assert_eq!(
                output.platform_output.cursor_icon,
                egui::CursorIcon::Crosshair
            );
            let end = Pos2::new(plot.left + plot.width * 0.75, plot.top + plot.height * 0.75);
            let _ = ctx.run_ui(
                input(end, vec![button(end, egui::PointerButton::Primary, false)]),
                |ui| {
                    assert!(handle_navigation(&mut app, 0, screen, ui));
                },
            );
            assert!(matches!(app.interaction(), Interaction::Idle));
            let after = &app.doc.canvases[0]
                .object(id)
                .unwrap()
                .plot()
                .unwrap()
                .viewport;
            match axis {
                ZoomAxis::X => {
                    assert_ne!(after.view_x, before.view_x);
                    assert_eq!(after.view_y, before.view_y);
                }
                _ => {
                    assert_ne!(after.view_y, before.view_y);
                    assert_eq!(after.view_x, before.view_x);
                }
            }
        }
    }
}

#[test]
fn pan_modifiers_override_axis_zoom_and_use_grabbing_during_drag() {
    for middle in [false, true] {
        for command in [false, true] {
            let (mut app, id, screen, plot, outer) = fixture();
            let p = Pos2::new(
                plot.left + plot.width * 0.5,
                (plot.bottom() + outer.bottom()) * 0.5,
            );
            let ctx = egui::Context::default();
            let events = if middle { vec![] } else { vec![space()] };
            let mut hover = input(p, events);
            hover.modifiers.ctrl = command;
            let _ = ctx.run_ui(hover, |ui| {
                if !middle {
                    assert_eq!(
                        navigation_cursor(&app, 0, screen, ui),
                        Some((egui::CursorIcon::Grab, None))
                    );
                }
            });
            let mut press = input(
                p,
                vec![button(
                    p,
                    if middle {
                        egui::PointerButton::Middle
                    } else {
                        egui::PointerButton::Primary
                    },
                    true,
                )],
            );
            press.modifiers.ctrl = command;
            // Match the event modifiers to the frame modifiers.
            if let egui::Event::PointerButton { modifiers, .. } = &mut press.events[1] {
                modifiers.ctrl = command;
            }
            let output = ctx.run_ui(press, |ui| {
                assert!(handle_navigation(&mut app, 0, screen, ui));
                if command {
                    assert!(matches!(app.interaction(), Interaction::Idle));
                } else {
                    assert!(matches!(app.interaction(), Interaction::Pan(d) if d.object == id));
                }
                canvas_cursor(&app, 0, screen, ui);
            });
            assert_eq!(
                output.platform_output.cursor_icon,
                egui::CursorIcon::Grabbing
            );
        }
    }
}

#[test]
fn unavailable_axis_zoom_has_no_zoom_feedback_or_navigation() {
    for blocked in ["command", "typing", "active", "corner", "empty"] {
        let (mut app, id, screen, plot, outer) = fixture();
        let mut p = Pos2::new(
            plot.left + plot.width * 0.5,
            (plot.bottom() + outer.bottom()) * 0.5,
        );
        if blocked == "corner" {
            p.x = outer.left() + 1.0;
        }
        if blocked == "empty" {
            app.doc.canvases[0].objects.clear();
        }
        if blocked == "active" {
            app.set_tool(Tool::BrowseZoom);
            app.begin_interaction(Interaction::Zoom(ZoomDrag {
                canvas: 0,
                object: id,
                start: [p.x, p.y],
                current: [p.x, p.y],
                axis: ZoomAxis::Box,
            }));
        }
        let ctx = egui::Context::default();
        let mut raw = input(p, vec![]);
        raw.modifiers.ctrl = blocked == "command";
        let _ = ctx.run_ui(raw, |ui| {
            if blocked == "typing" {
                ui.ctx()
                    .memory_mut(|m| m.request_focus(egui::Id::new("editor")));
            }
            assert_eq!(
                pointer_navigation(&app, 0, screen, p, ui),
                None,
                "{blocked}"
            );
            assert!(
                navigation_cursor(&app, 0, screen, ui).is_none_or(|(_, hint)| hint.is_none()),
                "{blocked}"
            );
        });
    }
}
