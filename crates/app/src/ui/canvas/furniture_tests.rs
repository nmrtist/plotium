use super::*;
use crate::ui::plot_annotations::tests::{annotations, app_with_plot};
use plotium_core::state::{AnnotationId, PlotAnnotations};

const SCREEN: egui::Rect = egui::Rect {
    min: Pos2::ZERO,
    max: Pos2::new(1200.0, 900.0),
};

fn pointer(pos: Pos2, pressed: Option<bool>) -> egui::RawInput {
    let mut events = vec![egui::Event::PointerMoved(pos)];
    if let Some(pressed) = pressed {
        events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    egui::RawInput {
        screen_rect: Some(SCREEN),
        events,
        ..Default::default()
    }
}

fn run(app: &mut PlotiumApp, ctx: &egui::Context, input: egui::RawInput) {
    let _ = ctx.run_ui(input, |ui| {
        handle_furniture_interactions(app, 0, SCREEN, ui);
    });
}

/// The plot with `edit` applied to its annotations, and its plot area on screen.
fn annotated(
    edit: impl FnOnce(&mut PlotAnnotations) -> AnnotationId,
) -> (PlotiumApp, ObjectId, AnnotationId, PlotRect) {
    let (mut app, object) = app_with_plot();
    let mut overrides = app.doc.canvases[0]
        .object(object)
        .and_then(|item| item.plot())
        .expect("plot")
        .axis_overrides
        .clone();
    let id = edit(&mut overrides.annotations);
    app.set_axis_overrides_value(0, object, &overrides);
    app.clear_history();
    let (plot, _) = plot_geometry(&app, 0, object, SCREEN).expect("plot on screen");
    (app, object, id, plot)
}

fn data_at(app: &PlotiumApp, object: ObjectId, plot: PlotRect, fx: f32, fy: f32) -> [f64; 2] {
    let figure = app.doc.canvases[0]
        .object(object)
        .and_then(|item| item.plot())
        .expect("plot")
        .figure();
    [
        from_screen_x(plot.left + fx * plot.width, plot, &figure.x),
        from_screen_y(plot.top + fy * plot.height, plot, &figure.y),
    ]
}

#[test]
fn dragging_an_arrow_moves_it_by_hand_as_one_undo_step() {
    let (app, object) = app_with_plot();
    let (plot, _) = plot_geometry(&app, 0, object, SCREEN).expect("plot on screen");
    let (tail, head) = (
        data_at(&app, object, plot, 0.2, 0.5),
        data_at(&app, object, plot, 0.6, 0.5),
    );
    let (mut app, object, arrow, plot) = annotated(|annotations| {
        annotations
            .add_arrow(
                tail,
                head,
                plotium_figure::ArrowHead::Filled,
                1.0,
                plotium_figure::Color::BLACK,
            )
            .expect("valid arrow")
    });
    let ctx = egui::Context::default();
    let middle = Pos2::new(plot.left + 0.4 * plot.width, plot.top + 0.5 * plot.height);
    run(&mut app, &ctx, pointer(middle, None));
    run(&mut app, &ctx, pointer(middle, Some(true)));
    assert!(matches!(app.interaction(), Interaction::Furniture(_)));
    run(
        &mut app,
        &ctx,
        pointer(middle + egui::vec2(0.0, 0.1 * plot.height), None),
    );
    run(
        &mut app,
        &ctx,
        pointer(middle + egui::vec2(0.0, 0.1 * plot.height), Some(false)),
    );

    let moved = annotations(&app, object).arrow(arrow).expect("arrow");
    assert!(moved.hand_moved);
    assert!(moved.at[1] < tail[1], "{moved:?}");
    assert_eq!(
        app.selected_plot_part().map(|(_, s)| s.part),
        Some(PlotPart::Annotation(arrow))
    );
    app.undo();
    assert_eq!(
        annotations(&app, object).arrow(arrow).expect("arrow").at,
        tail
    );
}

#[test]
fn double_clicking_a_text_annotation_opens_its_text() {
    let (app, object) = app_with_plot();
    let (plot, _) = plot_geometry(&app, 0, object, SCREEN).expect("plot on screen");
    let at = data_at(&app, object, plot, 0.5, 0.5);
    let (mut app, object, text, plot) = annotated(|annotations| {
        let id = annotations
            .add_text(at, "90.1%".into(), 12.0)
            .expect("valid text");
        // Pinned at its anchor, so the click point is known.
        annotations.get_mut(id).expect("text").placement =
            plotium_core::state::LabelPlacement::Fixed([0.0, 0.0]);
        id
    });
    let bounds = super::super::plot_labels::label_bounds(
        app.doc.canvases[0]
            .object(object)
            .and_then(|o| o.plot())
            .expect("plot"),
        plot,
        app.session.board.zoom,
        text,
    )
    .expect("drawn text");
    let centre = Pos2::new(
        bounds.left + 0.5 * bounds.width,
        bounds.top + 0.5 * bounds.height,
    );
    let ctx = egui::Context::default();
    run(&mut app, &ctx, pointer(centre, None));
    for _ in 0..2 {
        run(&mut app, &ctx, pointer(centre, Some(true)));
        run(&mut app, &ctx, pointer(centre, Some(false)));
    }
    let edit = app
        .session
        .ui
        .plot_text_edit
        .as_ref()
        .expect("text editor open");
    assert_eq!((edit.annotation, edit.buffer.as_str()), (text, "90.1%"));
}

#[test]
fn clicking_an_arrow_selects_it_without_moving_it() {
    let (app, object) = app_with_plot();
    let (plot, _) = plot_geometry(&app, 0, object, SCREEN).expect("plot on screen");
    let (tail, head) = (
        data_at(&app, object, plot, 0.2, 0.5),
        data_at(&app, object, plot, 0.6, 0.5),
    );
    let (mut app, object, arrow, plot) = annotated(|annotations| {
        annotations
            .add_arrow(
                tail,
                head,
                plotium_figure::ArrowHead::Filled,
                1.0,
                plotium_figure::Color::BLACK,
            )
            .expect("valid arrow")
    });
    let ctx = egui::Context::default();
    let middle = Pos2::new(plot.left + 0.4 * plot.width, plot.top + 0.5 * plot.height);
    run(&mut app, &ctx, pointer(middle, None));
    run(&mut app, &ctx, pointer(middle, Some(true)));
    run(&mut app, &ctx, pointer(middle, None));
    run(&mut app, &ctx, pointer(middle, Some(false)));
    let clicked = annotations(&app, object).arrow(arrow).expect("arrow");
    assert!(!clicked.hand_moved);
    assert_eq!((clicked.at, clicked.to), (tail, head));
    assert!(!app.can_undo());
    assert_eq!(
        app.selected_plot_part().map(|(_, s)| s.part),
        Some(PlotPart::Annotation(arrow))
    );
}
