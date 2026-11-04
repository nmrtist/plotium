use super::*;
use egui::{Event, PointerButton, Pos2, RawInput, Rect, vec2};

fn frame(
    app: &mut PlotiumApp,
    panel: &mut Panel,
    ctx: &egui::Context,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(900., 1000.))),
            events,
            ..Default::default()
        },
        |ui| body(app, ui, panel),
    )
}

fn click(app: &mut PlotiumApp, panel: &mut Panel, ctx: &egui::Context, text: &str) {
    click_label(app, panel, ctx, text, false);
}

fn click_label(
    app: &mut PlotiumApp,
    panel: &mut Panel,
    ctx: &egui::Context,
    text: &str,
    popup: bool,
) {
    let output = frame(app, panel, ctx, vec![]);
    let mut positions = output.shapes.iter().filter_map(|s| match &s.shape {
        egui::Shape::Text(t) if t.galley.text() == text => Some(t.pos + vec2(3., 5.)),
        _ => None,
    });
    let pos = if popup {
        positions.next_back()
    } else {
        positions.next()
    }
    .unwrap_or_else(|| panic!("missing label {text}"));
    for pressed in [true, false] {
        frame(
            app,
            panel,
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
}

#[test]
fn preview_binding_candidate_confirmation_and_residue_navigation_use_widgets() {
    let mut app = PlotiumApp::new();
    let ctx = egui::Context::default();
    let mut panel = Panel {
        import: Some(prepare("synthetic.fasta", ">alpha\nAG\n>beta\nGV".into()).unwrap()),
        ..Default::default()
    };
    click(&mut app, &mut panel, &ctx, "Import sequence");
    let m = app.doc.protein.molecules[0].clone();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/bruker-3d-qf");
    let mut peaks = Vec::new();
    for i in 0..2 {
        let (dataset, _) = plotium_core::workflow::dataset_from_acquisition(
            plotium_io::load_path(&fixture).unwrap().acquisition,
        )
        .unwrap();
        let d = dataset.resource_id();
        let name = dataset.display_name();
        app.doc.datasets.push(dataset);
        app.set_active_dataset(Some(i));
        click(
            &mut app,
            &mut panel,
            &ctx,
            "Bind active spectrum to this molecule",
        );
        assert_eq!(app.doc.protein.bindings.get(&d), Some(&m.id));
        let v = protein::volume(&app.doc, d).unwrap();
        let ppm = std::array::from_fn(|axis| v.processed.axis_coordinate(axis, 1).unwrap());
        let p = app.add_peak_3d(d, ppm).unwrap();
        app.session.ui.selected_peak_3d = Some((d, p));
        click(&mut app, &mut panel, &ctx, "New candidate");
        // The popup has its own layer; open it before selecting the first residue.
        click(&mut app, &mut panel, &ctx, "Unassigned");
        click(&mut app, &mut panel, &ctx, "A:A1");
        click(&mut app, &mut panel, &ctx, "HB1");
        click(&mut app, &mut panel, &ctx, "HB2");
        click(
            &mut app,
            &mut panel,
            &ctx,
            "One of these atoms (identity unresolved)",
        );
        // An unfinished second axis must not silently become unassigned.
        click(&mut app, &mut panel, &ctx, "Unassigned");
        click(&mut app, &mut panel, &ctx, "B:G1");
        click(&mut app, &mut panel, &ctx, "Save candidate");
        assert!(panel.draft.is_some());
        assert!(app.doc.protein.assignment(d, p).is_none());
        click(&mut app, &mut panel, &ctx, "Select atoms");
        click_label(&mut app, &mut panel, &ctx, "Unassigned", true);
        assert!(panel.draft.as_ref().unwrap().residues[1].is_none());

        click(&mut app, &mut panel, &ctx, "Optional note");
        frame(
            &mut app,
            &mut panel,
            &ctx,
            vec![Event::Text("Keyboard note".into())],
        );
        click(&mut app, &mut panel, &ctx, "Save candidate");
        click(&mut app, &mut panel, &ctx, "Confirm");
        let a = app.doc.protein.assignment(d, p).unwrap();
        assert_eq!(a.confirmed, Some(a.candidates[0].id));
        assert_eq!(a.candidates[0].note, "Keyboard note");
        assert_eq!(
            a.candidates[0].targets[0].as_ref().unwrap().members.len(),
            2
        );
        peaks.push((d, p, name));
    }
    click(&mut app, &mut panel, &ctx, "Sequence and associated peaks");
    for _ in 0..8 {
        frame(&mut app, &mut panel, &ctx, vec![]);
    }
    click(&mut app, &mut panel, &ctx, "Chain A · alpha");
    for _ in 0..8 {
        frame(&mut app, &mut panel, &ctx, vec![]);
    }
    click(&mut app, &mut panel, &ctx, "A1");
    let (d, p, name) = &peaks[0];
    click(
        &mut app,
        &mut panel,
        &ctx,
        &format!("{name} · Peak {} · Confirmed", p.0),
    );
    assert_eq!(app.session.ui.selected_peak_3d, Some((*d, *p)));
    assert_eq!(app.active_dataset(), Some(0));
}

fn setup_review_spectra() -> (PlotiumApp, Molecule, Vec<(DatasetId, Peak3DId)>) {
    let mut app = PlotiumApp::new();
    app.edit_protein(Edit::Import {
        source: "review.fasta".into(),
        fasta: ">chain\nAWA".into(),
        name: None,
        chains: None,
    })
    .unwrap();
    let m = app.doc.protein.molecules[0].clone();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/bruker-3d-qf");
    let mut peaks = Vec::new();
    for i in 0..2 {
        let (mut dataset, _) = plotium_core::workflow::dataset_from_acquisition(
            plotium_io::load_path(&fixture).unwrap().acquisition,
        )
        .unwrap();
        dataset.as_nmr2d_mut().unwrap().name = Some(format!("Review spectrum {i}"));
        let d = dataset.resource_id();
        app.doc.datasets.push(dataset);
        app.edit_protein(Edit::Bind {
            dataset: d,
            molecule: Some(m.id),
        })
        .unwrap();
        let v = protein::volume(&app.doc, d).unwrap();
        let ppm = std::array::from_fn(|a| v.processed.axis_coordinate(a, 1).unwrap());
        let p = app.add_peak_3d(d, ppm).unwrap();
        let targets = ["H", "N", "H"].map(|name| {
            Some(AtomTarget {
                residue: m.chains[0].residues[1].id,
                members: [name.into()].into_iter().collect(),
                group_meaning: GroupMeaning::Unspecified,
            })
        });
        app.edit_assignment(
            d,
            p,
            AssignmentEdit::Add {
                targets,
                note: String::new(),
            },
        )
        .unwrap();
        peaks.push((d, p));
    }
    app.set_active_dataset(Some(0));
    app.session.ui.selected_peak_3d = Some(peaks[0]);
    (app, m, peaks)
}

fn real_card_frame(
    app: &mut PlotiumApp,
    ctx: &egui::Context,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000., 700.))),
            events,
            ..Default::default()
        },
        |ui| render(app, ui),
    )
}

fn visible_text(output: &egui::FullOutput, label: &str) -> Option<Pos2> {
    output.shapes.iter().find_map(|s| match &s.shape {
        egui::Shape::Text(t) if t.galley.text() == label => {
            let pos = t.pos + vec2(3., 5.);
            s.clip_rect.contains(pos).then_some(pos)
        }
        _ => None,
    })
}

#[test]
fn real_card_scroll_reaches_save_for_complete_group_candidate() {
    let (mut app, m, peaks) = setup_review_spectra();
    let (d, p) = peaks[0];
    let base = app.doc.protein.assignment(d, p).unwrap().candidates[0].clone();
    let mut draft = Draft::new(d, p, Some(base));
    for axis in [0, 2] {
        draft.targets[axis] = Some(AtomTarget {
            residue: m.chains[0].residues[1].id,
            members: ["HB2".into(), "HB3".into()].into_iter().collect(),
            group_meaning: GroupMeaning::OneOf,
        });
    }
    let ctx = crate::typography::test_context();
    ctx.data_mut(|data| {
        data.insert_temp(
            key(&app),
            Panel {
                draft: Some(draft),
                ..Default::default()
            },
        )
    });
    open(&mut app);
    for _ in 0..8 {
        real_card_frame(&mut app, &ctx, vec![]);
    }
    let initial = real_card_frame(&mut app, &ctx, vec![]);
    assert!(visible_text(&initial, "Save candidate").is_none());
    // Wheel input must scroll the real card, not just an unconstrained body.
    real_card_frame(
        &mut app,
        &ctx,
        vec![
            Event::PointerMoved(Pos2::new(820., 260.)),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                phase: egui::TouchPhase::Move,
                delta: vec2(0., -2500.),
                modifiers: Default::default(),
            },
        ],
    );
    let mut save = None;
    for _ in 0..30 {
        let output = real_card_frame(&mut app, &ctx, vec![]);
        save = visible_text(&output, "Save candidate");
    }
    let pos = save.expect("Save must be reachable by scrolling at 1000×700");
    assert!(pos.y < 700.);
    for pressed in [true, false] {
        real_card_frame(
            &mut app,
            &ctx,
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
    assert!(
        ctx.data(|data| data.get_temp::<Panel>(key(&app)))
            .unwrap()
            .draft
            .is_none()
    );
    assert_eq!(
        app.doc.protein.assignment(d, p).unwrap().candidates[0].targets[0]
            .as_ref()
            .unwrap()
            .group_meaning,
        GroupMeaning::OneOf
    );
}

#[test]
fn clear_review_keeps_its_spectrum_and_requires_review_after_changes() {
    let (mut app, _, peaks) = setup_review_spectra();
    let ctx = egui::Context::default();
    let mut panel = Panel::default();
    let original = app.doc.protein.clone();
    click(
        &mut app,
        &mut panel,
        &ctx,
        "Clear all assignments on active spectrum",
    );
    assert_eq!(app.doc.protein, original);
    click(&mut app, &mut panel, &ctx, "Cancel clearing");
    assert_eq!(app.doc.protein, original);
    click(
        &mut app,
        &mut panel,
        &ctx,
        "Clear all assignments on active spectrum",
    );
    let (d, p) = peaks[0];
    let candidate = app.doc.protein.assignment(d, p).unwrap().candidates[0].id;
    app.edit_assignment(d, p, AssignmentEdit::Confirm { candidate })
        .unwrap();
    app.set_active_dataset(Some(1));
    app.session.ui.selected_peak_3d = Some(peaks[1]);
    let output = frame(&mut app, &mut panel, &ctx, vec![]);
    assert!(
        visible_text(
            &output,
            &format!("Active spectrum: {}", spectrum_name(&app, peaks[1].0))
        )
        .is_some()
    );
    assert!(
        visible_text(
            &output,
            &format!("Clear assignments from {}?", spectrum_name(&app, d))
        )
        .is_some()
    );
    assert!(visible_text(&output,"1 peaks · 1 candidates · 1 confirmed. Peak observations will remain. Undo restores the assignments.").is_some());
    click(
        &mut app,
        &mut panel,
        &ctx,
        "Confirm clear spectrum assignments",
    );
    assert!(app.doc.protein.assignment(d, p).is_some());
    click(&mut app, &mut panel, &ctx, "Review updated counts");
    click(
        &mut app,
        &mut panel,
        &ctx,
        "Confirm clear spectrum assignments",
    );
    assert!(app.doc.protein.assignment(d, p).is_none());
    assert!(app.doc.protein.assignment(peaks[1].0, peaks[1].1).is_some());
    app.undo();
    assert_eq!(
        app.doc.protein.assignment(d, p).unwrap().confirmed,
        Some(candidate)
    );
}
