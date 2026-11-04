use super::*;
use egui::{Event, PointerButton, Pos2, RawInput};
use plotium_core::state::Tool;

fn fixture() -> PlotiumApp {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/bruker-3d-qf");
    let loaded = plotium_io::load_path(path).unwrap();
    let (dataset, _) =
        plotium_core::workflow::dataset_from_acquisition(loaded.acquisition).unwrap();
    let mut app = PlotiumApp::new();
    app.doc.datasets.push(dataset);
    app.focus_single(0);
    app
}
fn frame(app: &mut PlotiumApp, ctx: &egui::Context, events: Vec<Event>) -> egui::FullOutput {
    ctx.run_ui(
        RawInput {
            events,
            ..Default::default()
        },
        |ui| panel(app, 0, ui),
    )
}
fn text_position(output: &egui::FullOutput, text: &str) -> Pos2 {
    output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text().starts_with(text) => {
                Some(t.pos + egui::vec2(5., 5.))
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("Missing UI text: {text}"))
}
fn click(app: &mut PlotiumApp, ctx: &egui::Context, position: Pos2) {
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            vec![
                Event::PointerMoved(position),
                Event::PointerButton {
                    pos: position,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
}
#[test]
fn empty_peak_panel_button_toggles_the_tool_and_single_click_locates_a_peak() {
    let mut app = fixture();
    let ctx = egui::Context::default();
    frame(&mut app, &ctx, vec![]);
    let output = frame(&mut app, &ctx, vec![]);
    let pick = text_position(&output, "Pick peaks");
    click(&mut app, &ctx, pick);
    assert_eq!(app.session.tool, Tool::Peaks);
    assert_eq!(
        commands::describe(&app, CommandId::Tool(Tool::Peaks)).checked,
        Some(true)
    );
    click(&mut app, &ctx, pick);
    assert_ne!(app.session.tool, Tool::Peaks);
    let dataset = app.doc.datasets[0].resource_id();
    let volume = app.doc.datasets[0]
        .as_nmr2d()
        .unwrap()
        .volume
        .as_ref()
        .unwrap();
    let ppm = std::array::from_fn(|a| volume.processed.axis_coordinate(a, 1).unwrap());
    let peak = app.add_peak_3d(dataset, ppm).unwrap();
    app.select_nmr_volume_plane(dataset, 0, 0).unwrap();
    frame(&mut app, &ctx, vec![]);
    let output = frame(&mut app, &ctx, vec![]);
    click(
        &mut app,
        &ctx,
        text_position(&output, &format!("{}  ", peak.0)),
    );
    assert_eq!(app.session.ui.selected_peak_3d, Some((dataset, peak)));
    assert_eq!(
        app.doc.datasets[0]
            .as_nmr2d()
            .unwrap()
            .volume
            .as_ref()
            .unwrap()
            .index,
        1
    );
}

#[test]
fn note_typing_commits_once_and_undo_redo_restore_the_whole_edit() {
    let mut app = fixture();
    let owner = app.doc.datasets[0].resource_id();
    let volume = app.doc.datasets[0]
        .as_nmr2d()
        .unwrap()
        .volume
        .as_ref()
        .unwrap();
    let ppm = std::array::from_fn(|a| volume.processed.axis_coordinate(a, 1).unwrap());
    let id = app.add_peak_3d(owner, ppm).unwrap();
    app.locate_peak_3d(owner, id).unwrap();
    let ctx = egui::Context::default();
    frame(&mut app, &ctx, vec![]);
    let output = frame(&mut app, &ctx, vec![]);
    click(
        &mut app,
        &ctx,
        text_position(&output, "Note") + egui::vec2(10., 22.),
    );
    let revision = app.doc.automation_revision;
    for text in ["over", "lap"] {
        frame(&mut app, &ctx, vec![Event::Text(text.into())]);
        assert_eq!(app.doc.automation_revision, revision);
    }
    click(&mut app, &ctx, text_position(&output, "Pick peaks"));
    let note = |app: &PlotiumApp| {
        app.doc.datasets[0]
            .as_nmr2d()
            .unwrap()
            .volume
            .as_ref()
            .unwrap()
            .peaks
            .marks[0]
            .note
            .clone()
    };
    assert_eq!(note(&app), "overlap");
    app.undo();
    assert_eq!(note(&app), "");
    app.redo();
    assert_eq!(note(&app), "overlap");
}

#[test]
fn coordinate_drag_commits_once_and_undo_restores_all_coordinates() {
    let mut app = fixture();
    let owner = app.doc.datasets[0].resource_id();
    let volume = app.doc.datasets[0]
        .as_nmr2d()
        .unwrap()
        .volume
        .as_ref()
        .unwrap();
    let ppm = std::array::from_fn(|a| volume.processed.axis_coordinate(a, 1).unwrap());
    let id = app.add_peak_3d(owner, ppm).unwrap();
    app.locate_peak_3d(owner, id).unwrap();
    let ctx = egui::Context::default();
    frame(&mut app, &ctx, vec![]);
    let output = frame(&mut app, &ctx, vec![]);
    let label = text_position(&output, "F1 ") - egui::vec2(5., 5.);
    let start = output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t)
                if t.galley.text().parse::<f64>().is_ok()
                    && (t.pos.y - label.y).abs() < 4.
                    && t.pos.x > label.x =>
            {
                Some(t.pos + egui::vec2(5., 5.))
            }
            _ => None,
        })
        .expect("coordinate input");
    let revision = app.doc.automation_revision;
    frame(
        &mut app,
        &ctx,
        vec![
            Event::PointerMoved(start),
            Event::PointerButton {
                pos: start,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
    );
    for distance in [15., 30., 45.] {
        frame(
            &mut app,
            &ctx,
            vec![Event::PointerMoved(start + egui::vec2(distance, 0.))],
        );
        assert_eq!(app.doc.automation_revision, revision);
    }
    let end = start + egui::vec2(45., 0.);
    frame(
        &mut app,
        &ctx,
        vec![Event::PointerButton {
            pos: end,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );
    let position = |app: &PlotiumApp| {
        app.doc.datasets[0]
            .as_nmr2d()
            .unwrap()
            .volume
            .as_ref()
            .unwrap()
            .peaks
            .marks[0]
            .ppm
    };
    assert_ne!(position(&app), ppm);
    let moved = position(&app);
    app.undo();
    assert_eq!(position(&app), ppm);
    app.redo();
    assert_eq!(position(&app), moved);
}
