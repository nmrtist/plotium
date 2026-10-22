use super::*;
use plotium_core::state::TextBox;

#[test]
fn aspect_resize_preserves_ratio_and_opposite_corner() {
    let before = ObjectFrame::new(10.0, 20.0, 40.0, 20.0);
    let candidate = ObjectFrame::new(0.0, 5.0, 50.0, 35.0);
    let after = preserve_aspect_frame(
        before,
        candidate,
        ObjectDragKind::Resize(ResizeHandle::TopLeft),
    );
    assert_eq!(after, ObjectFrame::new(-20.0, 5.0, 70.0, 35.0));
}

#[test]
fn panel_hit_includes_empty_panels_and_resize_handles() {
    let mut canvas = CanvasDocument::new("page".to_owned(), [100.0, 80.0]);
    let panel = canvas.create_panel("Panel".to_owned(), ObjectFrame::new(10.0, 12.0, 40.0, 30.0));

    assert_eq!(
        hit_panel(&canvas, Pos2::new(20.0, 20.0), 1.0)
            .unwrap()
            .panel,
        panel
    );
    assert!(matches!(
        hit_panel(&canvas, Pos2::new(10.0, 12.0), 1.0).unwrap().kind,
        ObjectDragKind::Resize(ResizeHandle::TopLeft)
    ));
}

#[test]
fn content_hit_uses_page_position_inside_an_editing_panel() {
    let mut canvas = CanvasDocument::new("page".to_owned(), [100.0, 80.0]);
    let content = ObjectId::new(1);
    canvas.objects.push(CanvasObject {
        id: content,
        name: "content".to_owned(),
        frame: ObjectFrame::new(4.0, 5.0, 16.0, 12.0),
        locked: false,
        visible: true,
        kind: CanvasObjectKind::Text(TextBox::label("text".to_owned())),
    });
    let panel = canvas.create_panel("Panel".to_owned(), ObjectFrame::new(30.0, 20.0, 40.0, 30.0));
    canvas.panel_mut(panel).unwrap().item_order.push(content);

    assert_eq!(
        hit_content_object(&canvas, Pos2::new(36.0, 26.0), 1.0, Some(panel))
            .unwrap()
            .object,
        content
    );
    assert!(hit_content_object(&canvas, Pos2::new(6.0, 11.0), 1.0, Some(panel)).is_none());
}

#[test]
fn plot_geometry_follows_content_that_does_not_fill_its_panel() {
    let (mut app, ids) = crate::ui::properties::fixture::contour_page(1);
    let object = ids[0];
    let canvas = &mut app.doc.canvases[0];
    let panel = canvas.create_panel(
        "Panel".to_owned(),
        ObjectFrame::new(40.0, 30.0, 150.0, 120.0),
    );
    canvas.panel_mut(panel).unwrap().item_order.push(object);
    canvas.object_mut(object).unwrap().frame = ObjectFrame::new(20.0, 10.0, 100.0, 50.0);
    app.session.board = BoardViewport {
        zoom: 1.0,
        world_center: [100.0, 100.0],
    };
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 400.0));

    let content = content_screen_rect(app.session.board, &app.doc.canvases[0], object, screen)
        .expect("content is on the board");
    let plot = plot_inner_rect(&app, 0, object, screen).expect("plot is on the board");
    assert!(plot.left >= content.left && plot.right() <= content.right());
    assert!(plot.top >= content.top && plot.bottom() <= content.bottom());

    let beside = Pos2::new(content.right() + 10.0, content.top + 10.0);
    assert!(plot_under_cursor(&app, 0, screen, beside).is_none());
}
