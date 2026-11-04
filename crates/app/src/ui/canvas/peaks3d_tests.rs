use super::*;
use egui::{Event, PointerButton, RawInput};

fn render(app: &mut PlotiumApp, ctx: &egui::Context, events: Vec<Event>) -> egui::FullOutput {
    ctx.run_ui(
        RawInput {
            events,
            ..Default::default()
        },
        |ui| {
            let response =
                ui.allocate_response(egui::vec2(800., 400.), egui::Sense::click_and_drag());
            let plot = PlotRect {
                left: response.rect.left(),
                top: response.rect.top(),
                width: 800.,
                height: 400.,
            };
            let object = app.doc.canvases[0].objects[0].id;
            handle_volume_peaks(app, 0, object, 0, plot, ui, &response);
            paint_volume_peaks(app, 0, object, 0, plot, ui.painter());
        },
    )
}
fn pointer(app: &mut PlotiumApp, ctx: &egui::Context, pos: Pos2, pressed: bool) {
    render(
        app,
        ctx,
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            },
        ],
    );
}
fn peaks(app: &PlotiumApp) -> &plotium_core::state::Peak3DSet {
    &app.doc.datasets[0]
        .as_nmr2d()
        .unwrap()
        .volume
        .as_ref()
        .unwrap()
        .peaks
}
fn locate_from_table(app: &mut PlotiumApp, peak: plotium_core::state::Peak3DId) {
    let ctx = egui::Context::default();
    let frame = |app: &mut PlotiumApp, events| {
        ctx.run_ui(
            RawInput {
                events,
                ..Default::default()
            },
            |ui| crate::ui::peaks3d::panel(app, 0, ui),
        )
    };
    frame(app, vec![]);
    let output = frame(app, vec![]);
    let row = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text().starts_with(&format!("{}  ", peak.0)) => {
                Some(text.pos + egui::vec2(5., 5.))
            }
            _ => None,
        })
        .expect("peak table row");
    for pressed in [true, false] {
        frame(
            app,
            vec![
                Event::PointerMoved(row),
                Event::PointerButton {
                    pos: row,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
}
fn scenario(path: &std::path::Path) {
    let loaded = plotium_io::load_path(path).unwrap();
    let (dataset, _) =
        plotium_core::workflow::dataset_from_acquisition(loaded.acquisition).unwrap();
    let owner = dataset.resource_id();
    let canvas = plotium_core::workflow::build_default_canvas(&dataset, "3D peak acceptance");
    let mut app = PlotiumApp::new();
    app.doc.datasets.push(dataset);
    app.doc.canvases.push(canvas);
    app.focus_single(0);
    app.set_tool(Tool::Peaks);
    let ctx = egui::Context::default();
    render(&mut app, &ctx, vec![]);
    // Drive the actual canvas event handler, including hit-testing and drag release.
    for pos in [
        Pos2::new(250., 160.),
        Pos2::new(450., 260.),
        Pos2::new(650., 120.),
    ] {
        pointer(&mut app, &ctx, pos, true);
        pointer(&mut app, &ctx, pos, false);
    }
    assert_eq!(peaks(&app).marks.len(), 3, "{}", app.session.status);
    let original = peaks(&app).clone();
    let start = Pos2::new(450., 260.);
    let end = Pos2::new(490., 290.);
    pointer(&mut app, &ctx, start, true);
    render(&mut app, &ctx, vec![Event::PointerMoved(end)]);
    pointer(&mut app, &ctx, end, false);
    let moved = peaks(&app).clone();
    assert_eq!(
        app.session.ui.selected_peak_3d,
        Some((owner, original.marks[1].id))
    );
    assert_eq!(moved.marks[1].id, original.marks[1].id);
    assert_ne!(moved.marks[1].ppm, original.marks[1].ppm);
    assert_eq!(moved.marks[1].ppm[0], original.marks[1].ppm[0]);
    app.undo();
    assert_eq!(peaks(&app), &original);
    app.redo();
    assert_eq!(peaks(&app), &moved);
    for axis in 0..3 {
        app.select_nmr_volume_plane(owner, axis, 0).unwrap();
        for mark in &moved.marks {
            locate_from_table(&mut app, mark.id);
            let v = app.doc.datasets[0]
                .as_nmr2d()
                .unwrap()
                .volume
                .as_ref()
                .unwrap();
            assert_eq!(v.index, v.peak_indices(mark.ppm).unwrap()[axis]);
            assert_eq!(app.session.ui.selected_peak_3d, Some((owner, mark.id)));
        }
        assert_eq!(peaks(&app), &moved);
    }
    let selected = app.session.ui.selected_peak_3d.unwrap().1;
    render(
        &mut app,
        &ctx,
        vec![Event::Key {
            key: egui::Key::Delete,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    assert!(peaks(&app).marks.iter().all(|m| m.id != selected));
    app.undo();
    assert_eq!(peaks(&app), &moved);
    let root = std::env::temp_dir().join(format!("plotium-ui-peaks-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let project = root.join("peaks.plotium");
    plotium_core::project::save_project(&app, &project, false).unwrap();
    let reopened = plotium_core::project::load_project(&project).unwrap();
    assert_eq!(peaks(&reopened), &moved);
    let table = reopened.peak_table_3d(owner).unwrap();
    let export = root.join("peaks.json");
    std::fs::write(&export, serde_json::to_vec_pretty(&table).unwrap()).unwrap();
    let exported: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&export).unwrap()).unwrap();
    assert_eq!(exported, app.peak_table_3d(owner).unwrap());
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn canvas_peak_gestures_navigation_and_project_export_roundtrip() {
    scenario(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../io/tests/fixtures/nmr/bruker-3d-qf"),
    );
}
#[test]
#[ignore = "requires local ubiquitin data; set PLOTIUM_UBIQUITIN"]
fn host_ubiquitin_peak_ui_acceptance() {
    let root = std::path::PathBuf::from(
        std::env::var_os("PLOTIUM_UBIQUITIN").expect("Set PLOTIUM_UBIQUITIN"),
    );
    for experiment in [2, 14, 16] {
        scenario(&root.join(experiment.to_string()));
        println!(
            "experiment={experiment}: canvas clicks/drag/delete, three orientations, undo/redo, project and JSON export passed"
        );
    }
}
