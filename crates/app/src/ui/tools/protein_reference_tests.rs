use super::*;
use crate::ui::tools::protein::Panel;
fn spectrum(app: &mut PlotiumApp) -> DatasetId {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/bruker-3d-qf");
    let (dataset, _) = plotium_core::workflow::dataset_from_acquisition(
        plotium_io::load_path(fixture).unwrap().acquisition,
    )
    .unwrap();
    let id = dataset.resource_id();
    app.doc.datasets.push(dataset);
    id
}

fn frame(
    app: &mut PlotiumApp,
    panel: &mut Panel,
    ctx: &egui::Context,
    active: Option<DatasetId>,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200., 2400.),
            )),
            events,
            ..Default::default()
        },
        |ui| render(app, ui, panel, active),
    )
}

fn click(
    app: &mut PlotiumApp,
    panel: &mut Panel,
    ctx: &egui::Context,
    active: Option<DatasetId>,
    label: &str,
) {
    let output = frame(app, panel, ctx, active, vec![]);
    let pos = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(t) if t.galley.text() == label => Some(t.pos + egui::vec2(3., 5.)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("Missing widget: {label}"));
    for pressed in [true, false] {
        frame(
            app,
            panel,
            ctx,
            active,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
}

#[test]
fn switching_spectra_displays_draft_target_and_saves_only_that_target() {
    let mut app = PlotiumApp::new();
    let a = spectrum(&mut app);
    let b = spectrum(&mut app);
    app.doc.datasets[0].as_nmr2d_mut().unwrap().name = Some("Spectrum A".into());
    app.doc.datasets[1].as_nmr2d_mut().unwrap().name = Some("Spectrum B".into());
    let ctx = egui::Context::default();
    let mut panel = Panel::default();
    click(
        &mut app,
        &mut panel,
        &ctx,
        Some(a),
        "Chemical-shift reference",
    );
    click(&mut app, &mut panel, &ctx, Some(a), "Edit reference");
    let (_, draft) = panel.reference_draft.as_mut().unwrap();
    draft.offsets_ppm = [0.125, 0., 0.];
    draft.source = "External standard".into();
    draft.confirmed = true;
    let output = frame(&mut app, &mut panel, &ctx, Some(b), vec![]);
    let labels: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t) => Some(t.galley.text().to_owned()),
            _ => None,
        })
        .collect();
    assert!(
        labels
            .iter()
            .any(|t| t.contains("Reference draft target: Spectrum A"))
    );
    assert!(labels.iter().any(|t| t.contains("active spectrum differs")));
    click(&mut app, &mut panel, &ctx, Some(b), "Save reference");
    assert!(panel.reference_draft.is_none());
    assert_eq!(
        plotium_core::protein::volume(&app.doc, a)
            .unwrap()
            .peaks
            .reference
            .offsets_ppm,
        [0.125, 0., 0.]
    );
    assert_eq!(
        plotium_core::protein::volume(&app.doc, b)
            .unwrap()
            .peaks
            .reference,
        Default::default()
    );
}
