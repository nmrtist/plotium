use super::*;
use plotium_core::state::{
    BoardFrameId, CanvasObject, CanvasObjectKind, ObjectFrame, SelectionScope, TextBox,
};

/// Select the second of three pages from the sidebar list, as a click does.
fn select_sidebar_page(app: &mut PlotiumApp) {
    app.session.ui.frame_selection = vec![BoardFrameId::Page(app.doc.canvases[1].resource_id)];
    app.session.ui.selection_scope = SelectionScope::CanvasList;
}

#[test]
fn delete_on_a_focused_page_row_deletes_the_selected_page() {
    let (mut app, _) = app_with_pages(3);
    let removed = app.doc.canvases[1].resource_id;
    select_sidebar_page(&mut app);
    let ctx = egui::Context::default();
    let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
        let row = ui.selectable_label(true, "Page");
        super::super::primary_sidebar::selection::claim_list_keyboard_focus(ui, &row);
    });
    let _ = ctx.run_ui(backspace(), |ui| {
        let _ = ui.selectable_label(true, "Page");
        handle_delete_shortcut(&mut app, ui.ctx());
    });

    assert_eq!(app.doc.canvases.len(), 2, "{}", app.session.status);
    assert!(app.doc.canvases.iter().all(|c| c.resource_id != removed));
}

#[test]
fn a_page_selection_deletes_the_page_not_a_remembered_object() {
    let (mut app, _) = app_with_pages(3);
    let id = app.doc.canvases[0].allocate_object_id();
    app.execute_action(Action::insert_object(
        0,
        CanvasObject {
            id,
            name: "Note".into(),
            frame: ObjectFrame::new(10., 10., 50., 20.),
            locked: false,
            visible: true,
            kind: CanvasObjectKind::Text(TextBox::label("note".into())),
        },
        Selection::None,
    ));
    app.session.ui.selection = Selection::Objects(vec![id]);
    select_sidebar_page(&mut app);

    delete_selection(&mut app, &egui::Context::default());

    assert_eq!(app.doc.canvases.len(), 2, "{}", app.session.status);
    assert!(app.doc.canvases[0].object(id).is_some());
}
