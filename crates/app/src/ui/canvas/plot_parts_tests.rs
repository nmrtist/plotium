use super::*;
use plotium_core::state::{ContentItem, ContentKind, TextBox};

fn key(key: egui::Key, pressed: bool, modifiers: egui::Modifiers) -> egui::RawInput {
    egui::RawInput {
        events: vec![egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed,
            repeat: false,
            modifiers,
        }],
        modifiers,
        ..Default::default()
    }
}

/// A page with a panel holding two texts, and one loose text.
fn app_with_texts() -> (PlotiumApp, PanelId, [ObjectId; 3]) {
    let mut app = PlotiumApp::new_with_settings(plotium_core::settings::Settings::default());
    let mut page = CanvasDocument::new("page".to_owned(), [300.0, 200.0]);
    let ids = [(); 3].map(|_| page.allocate_object_id());
    for (index, id) in ids.into_iter().enumerate() {
        page.objects.push(ContentItem {
            id,
            name: format!("text {index}"),
            frame: ObjectFrame::new(10.0, 10.0 + index as f32 * 20.0, 40.0, 10.0),
            locked: false,
            visible: true,
            kind: ContentKind::Text(TextBox::label("x".to_owned())),
        });
    }
    let panel = page.create_panel(
        "Panel".to_owned(),
        ObjectFrame::new(20.0, 30.0, 100.0, 80.0),
    );
    page.panel_mut(panel).unwrap().item_order = vec![ids[0], ids[1]];
    app.doc.canvases.push(page);
    app.session.active_canvas = Some(0);
    (app, panel, ids)
}

fn run(app: &mut PlotiumApp, ctx: &egui::Context, input: egui::RawInput) {
    let rect = egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
    let _ = ctx.run_ui(input, |ui| handle_nudge_keys(app, 0, rect, ui));
}

#[test]
fn a_held_arrow_moves_a_shared_panel_once_per_press_as_one_undo_step() {
    let (mut app, panel, ids) = app_with_texts();
    app.set_page_selection(0, &[ids[0], ids[1]], false);
    let ctx = egui::Context::default();

    run(
        &mut app,
        &ctx,
        key(egui::Key::ArrowRight, true, egui::Modifiers::NONE),
    );
    run(
        &mut app,
        &ctx,
        key(egui::Key::ArrowRight, true, egui::Modifiers::SHIFT),
    );
    let page = &app.doc.canvases[0];
    assert_eq!(page.panel(panel).unwrap().frame.x, 20.0 + 0.5 + 5.0);
    assert_eq!(page.object(ids[2]).unwrap().frame.x, 10.0);
    assert!(!app.can_undo(), "a held key records nothing until released");

    run(
        &mut app,
        &ctx,
        key(egui::Key::ArrowRight, false, egui::Modifiers::NONE),
    );
    assert!(app.can_undo());
    app.undo();
    assert_eq!(app.doc.canvases[0].panel(panel).unwrap().frame.x, 20.0);
    assert!(!app.can_undo());
}

#[test]
fn arrows_skip_locked_items_and_focused_text_fields() {
    let (mut app, _, ids) = app_with_texts();
    app.select_object(0, ids[2]);
    let ctx = egui::Context::default();
    let mut text = String::new();
    let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
        ui.add(egui::TextEdit::singleline(&mut text))
            .request_focus();
    });
    let rect = egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
    let _ = ctx.run_ui(
        key(egui::Key::ArrowDown, true, egui::Modifiers::NONE),
        |ui| {
            ui.add(egui::TextEdit::singleline(&mut text));
            handle_nudge_keys(&mut app, 0, rect, ui);
        },
    );
    assert_eq!(app.doc.canvases[0].object(ids[2]).unwrap().frame.y, 50.0);

    let ctx = egui::Context::default();
    app.doc.canvases[0].object_mut(ids[2]).unwrap().locked = true;
    run(
        &mut app,
        &ctx,
        key(egui::Key::ArrowDown, true, egui::Modifiers::NONE),
    );
    assert_eq!(app.doc.canvases[0].object(ids[2]).unwrap().frame.y, 50.0);
}
