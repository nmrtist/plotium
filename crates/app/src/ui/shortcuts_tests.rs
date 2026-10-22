use super::*;

/// Two dispatchable bindings must never share an effective chord. The
/// matcher ignores Shift for plain keys, so those normalize shift away.
#[test]
fn dispatchable_chords_are_unambiguous() {
    let mut seen = std::collections::HashSet::new();
    for binding in BINDINGS.iter().filter(|binding| binding.dispatch) {
        for chord in std::iter::once(binding.primary).chain(binding.aliases.iter().copied()) {
            assert!(
                seen.insert((chord.command, chord.command && chord.shift, chord.key)),
                "chord {chord:?} bound twice"
            );
        }
    }
}

#[test]
fn labels_derive_from_the_binding_table() {
    let label = shortcut_label(commands::CommandId::SaveProject).unwrap();
    assert!(label.ends_with("+S"));
    assert!(
        shortcut_label(commands::CommandId::PasteImage).is_some_and(|label| label.ends_with("+V"))
    );
    assert_eq!(
        shortcut_label(commands::CommandId::Tool(Tool::Select)).as_deref(),
        Some("V")
    );
    assert_eq!(
        shortcut_label(commands::CommandId::CycleCursor).as_deref(),
        Some("C")
    );
    assert!(shortcut_label(commands::CommandId::Tool(Tool::Symmetry)).is_none());
    assert!(shortcut_label(commands::CommandId::About).is_none());
}

fn paste_key_event() -> egui::Event {
    egui::Event::Key {
        key: egui::Key::V,
        physical_key: Some(egui::Key::V),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::CTRL,
    }
}

#[test]
fn restored_ctrl_v_and_platform_paste_events_route_to_paste_image() {
    for event in [
        paste_key_event(),
        egui::Event::Paste("clipboard".to_owned()),
    ] {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            events: vec![event],
            modifiers: egui::Modifiers::CTRL,
            ..Default::default()
        };
        let mut command = None;
        let _ = ctx.run_ui(input, |ui| command = shortcut_command(ui.ctx()));
        assert_eq!(command, Some(commands::CommandId::PasteImage));
    }
}

#[test]
fn focused_text_edit_keeps_ctrl_v_for_text_paste() {
    let ctx = egui::Context::default();
    let mut text = String::new();
    let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
        ui.add(egui::TextEdit::singleline(&mut text))
            .request_focus();
    });
    let input = egui::RawInput {
        events: vec![egui::Event::Paste("text".to_owned())],
        modifiers: egui::Modifiers::CTRL,
        ..Default::default()
    };
    let mut command = None;
    let _ = ctx.run_ui(input, |ui| {
        command = shortcut_command(ui.ctx());
        ui.add(egui::TextEdit::singleline(&mut text));
    });
    assert_eq!(command, None);
    assert_eq!(text, "text");
}

fn f_key_event() -> egui::Event {
    egui::Event::Key {
        key: egui::Key::F,
        physical_key: Some(egui::Key::F),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::default(),
    }
}

/// Plain `F` is context-split: over a plot's data area it fits that plot's
/// data viewport, elsewhere it keeps the board Show Selection meaning.
#[test]
fn plain_f_fits_the_plot_under_the_pointer_and_the_board_otherwise() {
    let (mut app, ids) = crate::ui::properties::fixture::contour_page(1);
    app.session.board = plotium_core::state::BoardViewport {
        zoom: 1.0,
        world_center: [500.0, 400.0],
    };
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 800.0));
    let plot = canvas::plot_inner_rect(&app, 0, ids[0], screen).expect("plot on the board");
    let inside = egui::Pos2::new(
        (plot.left + plot.right()) * 0.5,
        (plot.top + plot.bottom()) * 0.5,
    );

    // Zoom the data viewport away from the full range first.
    let plot_object = app.doc.canvases[0]
        .object_mut(ids[0])
        .and_then(|object| object.plot_mut())
        .expect("fixture plot");
    let full_x = plot_object.viewport.full_x;
    let full_y = plot_object.viewport.full_y;
    plot_object.viewport.view_x = plotium_core::state::AxisRange::new(
        full_x.min + full_x.span() * 0.25,
        full_x.max - full_x.span() * 0.25,
    );
    plot_object.apply_viewport();

    let ctx = egui::Context::default();
    let mut clipboard = clipboard_table::ClipboardTablePaste::default();
    let mut frame = |app: &mut PlotiumApp, pointer: egui::Pos2| {
        let _ = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                events: vec![egui::Event::PointerMoved(pointer), f_key_event()],
                ..Default::default()
            },
            |ui| {
                canvas::store_navigation_rect(ui.ctx(), screen);
                handle_plot_fit_shortcut(app, &mut clipboard, ui.ctx());
            },
        );
    };

    frame(&mut app, inside);
    let viewport = app.doc.canvases[0]
        .object(ids[0])
        .and_then(|object| object.plot())
        .expect("fixture plot")
        .viewport
        .clone();
    assert_eq!(viewport.view_x, full_x);
    assert_eq!(viewport.view_y, full_y);
    assert_eq!(app.session.status, "Fit plot to the full data range.");

    // Outside any plot the chord still fits the board to the selection.
    frame(&mut app, egui::Pos2::new(5.0, 5.0));
    assert!(matches!(
        app.session.viewport_mode,
        plotium_core::state::ViewportMode::Fit(_)
    ));
}

#[test]
fn escape_exits_an_active_tool_after_other_fallbacks() {
    let mut app = PlotiumApp::new_with_settings(plotium_core::settings::Settings::default());
    app.set_tool(Tool::Integrate);

    handle_escape(&mut app, 0.0);

    assert_eq!(app.session.tool, Tool::BrowseZoom);
    assert_eq!(app.session.status, "Exited tool mode.");
}

#[test]
fn escape_finishes_a_pending_wheel_property_gesture() {
    let mut app = PlotiumApp::new_with_settings(plotium_core::settings::Settings::default());
    app.session.ui.wheel_property = Some(plotium_core::actions::PendingWheelPropertyEdit {
        canvas: 0,
        object: plotium_core::state::ObjectId::new(1),
        property: plotium_core::properties::contour::BASE_MAGNITUDE,
        targets: Vec::new(),
        accumulator: 0.0,
        last_input_time: 0.0,
        gesture_started: false,
    });

    handle_escape(&mut app, 1.0);

    assert!(app.session.ui.wheel_property.is_none());
    assert_eq!(app.session.status, "Cancelled interaction.");
}

#[test]
fn close_shortcut_matches_platform_window_semantics() {
    #[cfg(target_os = "macos")]
    {
        assert_eq!(
            binding(commands::CommandId::CloseWindow)
                .unwrap()
                .primary
                .key,
            egui::Key::W
        );
        assert_eq!(
            binding(commands::CommandId::Quit).unwrap().primary.key,
            egui::Key::Q
        );
        assert!(binding(commands::CommandId::CloseProject).is_none());
    }
    #[cfg(not(target_os = "macos"))]
    assert_eq!(
        binding(commands::CommandId::CloseProject)
            .unwrap()
            .primary
            .key,
        egui::Key::W
    );
}

fn app_with_pages(pages: usize) -> (PlotiumApp, plotium_core::state::PanelId) {
    let mut app = PlotiumApp::new_with_settings(plotium_core::settings::Settings::default());
    for _ in 0..pages {
        app.doc
            .canvases
            .push(plotium_core::state::CanvasDocument::new(
                "page".to_owned(),
                [180.0, 120.0],
            ));
    }
    let panel = app.doc.canvases[0].create_panel(
        "Panel".to_owned(),
        plotium_core::state::ObjectFrame::new(10.0, 10.0, 100.0, 80.0),
    );
    app.session.active_canvas = Some(0);
    (app, panel)
}

fn select_panel_on_canvas(app: &mut PlotiumApp, panel: plotium_core::state::PanelId) {
    app.select_panel(0, panel);
    app.session.ui.selection_scope = plotium_core::state::SelectionScope::CanvasObjects;
}

#[test]
fn delete_removes_the_selected_panel_undoably() {
    let (mut app, panel) = app_with_pages(1);
    select_panel_on_canvas(&mut app, panel);

    delete_selection(&mut app, &egui::Context::default());
    assert!(app.doc.canvases[0].panel(panel).is_none());

    app.undo();
    assert!(app.doc.canvases[0].panel(panel).is_some());
}

#[test]
fn an_empty_page_click_deselects_the_panel_so_delete_keeps_it() {
    let (mut app, panel) = app_with_pages(1);
    select_panel_on_canvas(&mut app, panel);

    canvas::clear_canvas_interaction_state(
        &mut app,
        0,
        canvas::CanvasInteractionClearScope::Selection,
    );
    assert!(app.session.ui.hierarchical_selection.paths().is_empty());

    delete_selection(&mut app, &egui::Context::default());
    assert!(app.doc.canvases[0].panel(panel).is_some());
}

#[test]
fn escape_deselects_a_selected_panel() {
    let (mut app, panel) = app_with_pages(1);
    select_panel_on_canvas(&mut app, panel);

    handle_escape(&mut app, 0.0);

    assert!(app.session.ui.hierarchical_selection.paths().is_empty());
    assert_eq!(app.session.status, "Selection cleared.");
}

#[test]
fn escape_deselects_a_panel_left_on_an_inactive_page() {
    let (mut app, panel) = app_with_pages(2);
    select_panel_on_canvas(&mut app, panel);
    app.session.active_canvas = Some(1);

    handle_escape(&mut app, 0.0);

    assert!(app.session.ui.hierarchical_selection.paths().is_empty());
}

#[test]
fn delete_keeps_a_locked_panel_and_explains_why() {
    let (mut app, panel) = app_with_pages(1);
    app.doc.canvases[0].panel_mut(panel).unwrap().locked = true;
    select_panel_on_canvas(&mut app, panel);

    delete_selection(&mut app, &egui::Context::default());

    assert!(app.doc.canvases[0].panel(panel).is_some());
    assert!(!app.session.status.is_empty());
}

#[test]
fn delete_removes_selected_board_pages_undoably() {
    let (mut app, _) = app_with_pages(3);
    let page = |app: &PlotiumApp, ci: usize| {
        plotium_core::state::BoardFrameId::Page(app.doc.canvases[ci].resource_id)
    };
    let kept = app.doc.canvases[1].resource_id;
    app.session.ui.frame_selection = vec![page(&app, 0), page(&app, 2)];
    app.session.ui.selection_scope = plotium_core::state::SelectionScope::Board;

    delete_selection(&mut app, &egui::Context::default());
    assert_eq!(app.doc.canvases.len(), 1);
    assert_eq!(app.doc.canvases[0].resource_id, kept);
    assert!(app.session.ui.frame_selection.is_empty());

    app.undo();
    app.undo();
    assert_eq!(app.doc.canvases.len(), 3);
}

#[test]
fn analysis_tools_keep_delete_for_their_own_marks() {
    let (mut app, panel) = app_with_pages(2);
    select_panel_on_canvas(&mut app, panel);
    app.set_tool(Tool::Peaks);

    delete_selection(&mut app, &egui::Context::default());
    assert!(app.doc.canvases[0].panel(panel).is_some());

    app.session.ui.frame_selection = vec![plotium_core::state::BoardFrameId::Page(
        app.doc.canvases[1].resource_id,
    )];
    app.session.ui.selection_scope = plotium_core::state::SelectionScope::Board;
    delete_selection(&mut app, &egui::Context::default());
    assert_eq!(app.doc.canvases.len(), 2);
}

#[test]
fn focused_text_edit_keeps_delete_for_text_editing() {
    let (mut app, panel) = app_with_pages(1);
    select_panel_on_canvas(&mut app, panel);
    let ctx = egui::Context::default();
    let mut text = "abc".to_owned();
    let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
        ui.add(egui::TextEdit::singleline(&mut text))
            .request_focus();
    });
    let input = egui::RawInput {
        events: vec![egui::Event::Key {
            key: egui::Key::Backspace,
            physical_key: Some(egui::Key::Backspace),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
        ..Default::default()
    };
    let _ = ctx.run_ui(input, |ui| {
        handle_delete_shortcut(&mut app, ui.ctx());
        ui.add(egui::TextEdit::singleline(&mut text));
    });

    assert!(app.doc.canvases[0].panel(panel).is_some());
}

#[test]
fn tab_traversal_moves_delete_from_the_page_to_the_traversed_panel() {
    let (mut app, panel) = app_with_pages(1);
    app.session.ui.frame_selection = vec![plotium_core::state::BoardFrameId::Page(
        app.doc.canvases[0].resource_id,
    )];
    app.session.ui.selection_scope = plotium_core::state::SelectionScope::Board;
    let ctx = egui::Context::default();
    let tab = egui::RawInput {
        events: vec![egui::Event::Key {
            key: egui::Key::Tab,
            physical_key: Some(egui::Key::Tab),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
        ..Default::default()
    };
    let _ = ctx.run_ui(tab, |ui| handle_hierarchy_traversal(&mut app, ui.ctx()));
    delete_selection(&mut app, &ctx);

    assert_eq!(app.doc.canvases.len(), 1);
    assert!(app.doc.canvases[0].panel(panel).is_none());
}
