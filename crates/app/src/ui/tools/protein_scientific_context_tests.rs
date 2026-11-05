use super::*;

#[test]
fn draft_conflicts_after_edits_undo_redo_and_reloads_without_overwriting() {
    let mut app = PlotiumApp::new();
    let mut draft = ContextDraft::new(&app, None);
    draft.context.samples.push(SampleCondition::default());
    let mut concurrent = ScientificContext::default();
    concurrent.samples.push(SampleCondition {
        name: "Concurrent edit".into(),
        ..Default::default()
    });
    app.edit_scientific_context(concurrent.clone()).unwrap();
    assert!(draft.conflicted(&app));
    assert!(draft.save(&mut app).unwrap_err().contains("changed"));
    assert_eq!(app.doc.protein.context, concurrent);
    let current_draft = ContextDraft::new(&app, None);
    app.undo();
    assert!(current_draft.conflicted(&app));
    assert!(current_draft.save(&mut app).is_err());
    assert!(app.doc.protein.context.is_empty());
    let undo_draft = ContextDraft::new(&app, None);
    app.redo();
    assert!(undo_draft.save(&mut app).is_err());
    let mut reloaded = ContextDraft::new(&app, current_draft.dataset);
    assert!(!reloaded.conflicted(&app));
    reloaded.context.samples[0].name = "Reviewed".into();
    reloaded.save(&mut app).unwrap();
    assert_eq!(app.doc.protein.context.samples[0].name, "Reviewed");
}

#[test]
fn form_creates_and_reuses_samples_without_changing_document_before_save() {
    let mut app = PlotiumApp::new();
    let ctx = egui::Context::default();
    let mut draft = ContextDraft::new(&app, None);
    let sample = SampleCondition::default();
    let id = sample.id;
    draft.context.samples.push(sample);
    draft.sample = Some(id);
    let _output = ctx.run_ui(Default::default(), |ui| form(ui, &mut draft));
    assert_eq!(draft.context.samples.len(), 1);
    assert_eq!(draft.sample, Some(id));
    assert!(app.doc.protein.context.is_empty());
    draft.context.samples[0].ph = Some(14.01);
    assert!(draft.save(&mut app).is_err());
    assert!(app.doc.protein.context.is_empty());
}

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
        |ui| scientific_context(app, ui, panel, active),
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
fn deleted_unsaved_spectrum_offers_recovery_preserving_sample_edits() {
    let mut app = PlotiumApp::new();
    let dataset = spectrum(&mut app);
    let ctx = egui::Context::default();
    let mut panel = Panel::default();
    click(
        &mut app,
        &mut panel,
        &ctx,
        Some(dataset),
        "Scientific context",
    );
    click(
        &mut app,
        &mut panel,
        &ctx,
        Some(dataset),
        "Edit sample conditions and experiments",
    );
    click(
        &mut app,
        &mut panel,
        &ctx,
        Some(dataset),
        "New sample conditions",
    );
    let samples = panel
        .context_draft
        .as_ref()
        .unwrap()
        .context
        .samples
        .clone();
    let baseline = app.doc.protein.context.clone();
    app.try_execute_action(app.delete_dataset_action(0).unwrap())
        .unwrap();
    assert_eq!(app.doc.protein.context, baseline);
    assert!(panel.context_draft.as_ref().unwrap().conflicted(&app));
    assert!(
        panel
            .context_draft
            .as_ref()
            .unwrap()
            .save(&mut app)
            .unwrap_err()
            .contains("deleted")
    );
    click(&mut app, &mut panel, &ctx, None, "Save scientific context");
    assert_eq!(app.doc.protein.context, baseline);
    click(
        &mut app,
        &mut panel,
        &ctx,
        None,
        "Keep sample edits (sample-only)",
    );
    assert!(panel.context_draft.as_ref().unwrap().dataset.is_none());
    click(&mut app, &mut panel, &ctx, None, "Save scientific context");
    assert_eq!(app.doc.protein.context.samples, samples);
    assert!(app.doc.protein.context.experiments.is_empty());
}

#[test]
fn widgets_create_switch_share_and_save_samples_and_roundtrip_advanced_json() {
    let mut app = PlotiumApp::new();
    let first = spectrum(&mut app);
    let second = spectrum(&mut app);
    let ctx = egui::Context::default();
    let mut panel = Panel::default();
    click(
        &mut app,
        &mut panel,
        &ctx,
        Some(first),
        "Scientific context",
    );
    click(
        &mut app,
        &mut panel,
        &ctx,
        Some(first),
        "Edit sample conditions and experiments",
    );
    click(
        &mut app,
        &mut panel,
        &ctx,
        Some(first),
        "New sample conditions",
    );
    let shared = panel.context_draft.as_ref().unwrap().sample.unwrap();
    click(
        &mut app,
        &mut panel,
        &ctx,
        Some(first),
        "New sample conditions",
    );
    click(
        &mut app,
        &mut panel,
        &ctx,
        Some(first),
        "Sample conditions 2",
    );
    click(
        &mut app,
        &mut panel,
        &ctx,
        Some(first),
        "Sample conditions 1 (1)",
    );
    click(
        &mut app,
        &mut panel,
        &ctx,
        Some(first),
        "Unknown experiment",
    );
    click(&mut app, &mut panel, &ctx, Some(first), "NOESY");
    click(&mut app, &mut panel, &ctx, Some(first), "F1: Unknown role");
    click(&mut app, &mut panel, &ctx, Some(first), "Indirect proton");
    click(
        &mut app,
        &mut panel,
        &ctx,
        Some(first),
        "Save scientific context",
    );
    assert!(panel.context_draft.is_none());
    assert_eq!(app.doc.protein.context.experiments[&first].sample, shared);
    assert_eq!(
        app.doc.protein.context.experiments[&first].roles[0],
        ExperimentAxisRole::IndirectProton
    );
    click(
        &mut app,
        &mut panel,
        &ctx,
        Some(second),
        "Edit sample conditions and experiments",
    );
    click(
        &mut app,
        &mut panel,
        &ctx,
        Some(second),
        "Sample conditions 1",
    );
    click(
        &mut app,
        &mut panel,
        &ctx,
        Some(second),
        "Sample conditions 2 (2)",
    );
    click(
        &mut app,
        &mut panel,
        &ctx,
        Some(second),
        "Sample conditions 2",
    );
    click(
        &mut app,
        &mut panel,
        &ctx,
        Some(second),
        "Sample conditions 1 (1)",
    );
    click(&mut app, &mut panel, &ctx, Some(second), "Advanced JSON");
    let before = app.doc.protein.context.clone();
    // Replace the focused JSON text through keyboard events, then use the real Apply button.
    let output = frame(&mut app, &mut panel, &ctx, Some(second), vec![]);
    let pos = output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text().starts_with('{') => {
                Some(t.pos + egui::vec2(4., 5.))
            }
            _ => None,
        })
        .unwrap();
    for pressed in [true, false] {
        frame(
            &mut app,
            &mut panel,
            &ctx,
            Some(second),
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
    let mut edited = panel.context_draft.as_ref().unwrap().context.clone();
    edited.samples[0].solvent = "D2O".into();
    let modifiers = egui::Modifiers {
        ctrl: true,
        command: true,
        ..Default::default()
    };
    frame(
        &mut app,
        &mut panel,
        &ctx,
        Some(second),
        vec![
            egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            },
            egui::Event::Text(serde_json::to_string_pretty(&edited).unwrap()),
        ],
    );
    click(
        &mut app,
        &mut panel,
        &ctx,
        Some(second),
        "Save scientific context",
    );
    assert_eq!(app.doc.protein.context, before);
    click(
        &mut app,
        &mut panel,
        &ctx,
        Some(second),
        "Apply JSON to draft",
    );
    assert!(
        panel.context_draft.as_ref().unwrap().json.is_none(),
        "{}",
        panel.feedback
    );
    assert_eq!(
        panel.context_draft.as_ref().unwrap().context.samples[0].solvent,
        "D2O"
    );
    assert_eq!(app.doc.protein.context, before);
    click(&mut app, &mut panel, &ctx, Some(second), "Advanced JSON");
    click(
        &mut app,
        &mut panel,
        &ctx,
        Some(second),
        "Cancel JSON edits",
    );
    click(
        &mut app,
        &mut panel,
        &ctx,
        Some(second),
        "Save scientific context",
    );
    assert!(panel.context_draft.is_none());
    assert_eq!(app.doc.protein.context.experiments[&second].sample, shared);
    assert_eq!(app.doc.protein.context.samples.len(), 2);
    assert_eq!(app.doc.protein.context.samples[0].solvent, "D2O");
}
