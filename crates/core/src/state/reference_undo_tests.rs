use super::*;
use crate::protein::{AssignmentEdit, AtomTarget, Edit, GroupMeaning};
use crate::protein_reference::ShiftReference;

#[test]
fn reference_undo_redo_restores_coordinates_provenance_assignments_and_viewport() {
    let (mut app, owner) = fixture();
    app.edit_protein(Edit::Import {
        source: "test".into(),
        fasta: ">test\nAA".into(),
        name: None,
        chains: None,
    })
    .unwrap();
    let molecule = app.doc.protein.molecules[0].clone();
    app.edit_protein(Edit::Bind {
        dataset: owner,
        molecule: Some(molecule.id),
    })
    .unwrap();
    let original: [f64; 3] =
        std::array::from_fn(|a| volume(&app).processed.axis_coordinate(a, 1).unwrap());
    let peak = app.add_peak_3d(owner, original).unwrap();
    app.edit_assignment(
        owner,
        peak,
        AssignmentEdit::Add {
            targets: [
                Some(AtomTarget {
                    residue: molecule.chains[0].residues[0].id,
                    members: ["H".into()].into(),
                    group_meaning: GroupMeaning::Unspecified,
                }),
                None,
                None,
            ],
            note: "manual".into(),
        },
    )
    .unwrap();
    let candidate = app.doc.protein.assignment(owner, peak).unwrap().candidates[0].id;
    app.edit_assignment(owner, peak, AssignmentEdit::Confirm { candidate })
        .unwrap();
    let protein = app.doc.protein.clone();
    let mut canvas = CanvasDocument::new("Reference test".into(), [180., 90.]);
    let object = canvas.allocate_object_id();
    canvas.objects.push(app.build_plot_object(
        0,
        ObjectFrame::new(8., 22., 235., 195.),
        object,
        "Spectrum".into(),
    ));
    app.doc.canvases.push(canvas);
    let before_view = app.doc.canvases[0].objects[0]
        .plot()
        .unwrap()
        .viewport
        .clone();
    let before_export = app.peak_table_3d(owner).unwrap();
    let reference = ShiftReference {
        offsets_ppm: [0.1, -0.2, 0.3],
        source: "Standard".into(),
        confirmed: true,
    };
    let pin = CursorPoint {
        canvas: app.doc.canvases[0].resource_id,
        object,
        dataset: owner,
        x: original[2],
        y: Some(original[1]),
        intensity: 12.,
    };
    for step in 0..3 {
        app.session.ui.inspect_cursor_pin = Some(pin);
        app.session.ui.delta_cursor_anchor = Some(pin);
        app.session.ui.delta_cursor_pin = Some(CursorDelta {
            first: pin,
            second: pin,
        });
        match step {
            0 => app.set_shift_reference(owner, reference.clone()).unwrap(),
            1 => app.undo(),
            _ => app.redo(),
        }
        assert!(app.session.ui.inspect_cursor_pin.is_none());
        assert!(app.session.ui.delta_cursor_anchor.is_none());
        assert!(app.session.ui.delta_cursor_pin.is_none());
        let corrected = step != 1;
        let v = volume(&app);
        for (axis, original) in original.into_iter().enumerate() {
            let expected = original
                + if corrected {
                    reference.offsets_ppm[axis]
                } else {
                    0.
                };
            assert!((v.processed.axis_coordinate(axis, 1).unwrap() - expected).abs() < 1e-10);
            assert!((v.peaks.marks[0].ppm[axis] - expected).abs() < 1e-10);
        }
        assert_eq!(v.peaks.marks[0].id, peak);
        assert_eq!(app.doc.protein, protein);
        assert_eq!(
            v.peaks.reference,
            if corrected {
                reference.clone()
            } else {
                ShiftReference::default()
            }
        );
        let export = app.peak_table_3d(owner).unwrap();
        assert_eq!(
            export["reference"],
            serde_json::to_value(&v.peaks.reference).unwrap()
        );
        assert_eq!(
            export["peaks"],
            serde_json::to_value(&v.peaks.marks).unwrap()
        );
        let view = &app.doc.canvases[0].objects[0].plot().unwrap().viewport;
        if corrected {
            assert!((view.view_x.min - before_view.view_x.min - 0.3).abs() < 1e-10);
            assert!((view.view_y.min - before_view.view_y.min + 0.2).abs() < 1e-10);
            assert!((view.full_x.min - before_view.full_x.min - 0.3).abs() < 1e-10);
            assert!((view.full_y.min - before_view.full_y.min + 0.2).abs() < 1e-10);
        } else {
            assert_eq!(*view, before_view);
            assert_eq!(export, before_export);
        }
    }
}
