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

fn aspect_drag(before: ObjectFrame, dx: f32, dy: f32) -> ObjectFrame {
    let kind = ObjectDragKind::Resize(ResizeHandle::BottomRight);
    preserve_aspect_frame(before, drag_frame(before, kind, dx, dy, true), kind)
}

fn free_drag(before: ObjectFrame, handle: ResizeHandle, dx: f32, dy: f32) -> ObjectFrame {
    drag_frame(before, ObjectDragKind::Resize(handle), dx, dy, false)
}

#[test]
fn aspect_resize_of_a_small_object_does_not_jump_and_drags_back() {
    let natural = ObjectFrame::new(0.0, 0.0, 75.0, 15.0);
    let larger = aspect_drag(natural, 75.0, 15.0);
    assert_eq!(larger, ObjectFrame::new(0.0, 0.0, 150.0, 30.0));
    assert_eq!(aspect_drag(natural, -15.0, -3.0).width, 60.0);
    assert_eq!(aspect_drag(larger, -75.0, -15.0), natural);
}

#[test]
fn aspect_resize_shrinks_from_a_short_edge_drag_alone() {
    let natural = ObjectFrame::new(0.0, 0.0, 75.0, 15.0);
    assert_eq!(aspect_drag(natural, 0.0, -10.0).height, 5.0);
    let larger = ObjectFrame::new(0.0, 0.0, 150.0, 30.0);
    assert_eq!(aspect_drag(larger, 0.0, -15.0), natural);
}

#[test]
fn free_resize_never_grows_a_short_edge() {
    let before = ObjectFrame::new(0.0, 0.0, 100.0, 15.0);
    let handle = ResizeHandle::BottomRight;
    assert_eq!(
        free_drag(before, handle, -20.0, 0.0),
        ObjectFrame::new(0.0, 0.0, 80.0, 15.0)
    );
    assert_eq!(free_drag(before, handle, -200.0, -40.0).height, 15.0);
    assert_eq!(free_drag(before, handle, -200.0, 0.0).width, 24.0);
}

#[test]
fn free_resize_at_the_minimum_holds_the_opposite_edges() {
    let before = ObjectFrame::new(10.0, 20.0, 100.0, 15.0);
    let cases = [
        (
            ResizeHandle::TopLeft,
            ObjectFrame::new(86.0, 20.0, 24.0, 15.0),
        ),
        (
            ResizeHandle::TopRight,
            ObjectFrame::new(10.0, 20.0, 24.0, 15.0),
        ),
        (
            ResizeHandle::BottomLeft,
            ObjectFrame::new(86.0, 20.0, 24.0, 15.0),
        ),
        (
            ResizeHandle::BottomRight,
            ObjectFrame::new(10.0, 20.0, 24.0, 15.0),
        ),
    ];
    for (handle, expected) in cases {
        let moves_left = matches!(handle, ResizeHandle::TopLeft | ResizeHandle::BottomLeft);
        let moves_top = matches!(handle, ResizeHandle::TopLeft | ResizeHandle::TopRight);
        let dx = if moves_left { 200.0 } else { -200.0 };
        let dy = if moves_top { 40.0 } else { -40.0 };
        assert_eq!(free_drag(before, handle, dx, dy), expected);
    }
}

#[test]
fn free_resize_keeps_the_minimum_for_large_objects() {
    let before = ObjectFrame::new(0.0, 0.0, 100.0, 80.0);
    assert_eq!(
        free_drag(before, ResizeHandle::BottomRight, -200.0, -200.0),
        ObjectFrame::new(0.0, 0.0, 24.0, 24.0)
    );
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
