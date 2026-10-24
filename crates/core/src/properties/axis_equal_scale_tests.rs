use super::axis_scale_tests::{figure, line_app, write};
use super::*;
use crate::automation::TargetRef;
use crate::state::{CanvasDocument, Dataset, ObjectFrame, ObjectId, PlotiumApp};

fn equal_scale(app: &PlotiumApp, target: &TargetRef) -> ResolvedProperty {
    app.resolve_property(&PropertyAddress::new(target.clone(), axis::EQUAL_SCALE))
        .expect("equal scale resolves")
}

fn afm_app() -> (PlotiumApp, TargetRef, ObjectId) {
    let channel = plotium_io::AfmImageChannel {
        name: "Height".to_owned(),
        width: 4,
        height: 4,
        scan_size_x: 3.0,
        scan_size_y: 3.0,
        lateral_unit: "nm".to_owned(),
        scale: plotium_io::AfmScale {
            multiplier: 1.0,
            offset: 0.0,
            unit: "nm".to_owned(),
        },
        raw: std::sync::Arc::from((0..16).collect::<Vec<i32>>()),
        frame_direction: plotium_io::AfmFrameDirection::Trace,
    };
    let mut app = PlotiumApp::new();
    app.doc
        .datasets
        .push(Dataset::Afm(Box::new(crate::state::AfmDataset::load(
            plotium_io::AfmData {
                images: vec![channel],
                forces: None,
                source: "equal scale".to_owned(),
                import_warnings: Vec::new(),
            },
        ))));
    let mut canvas = CanvasDocument::new("page".to_owned(), [120.0, 80.0]);
    let id = canvas.allocate_object_id();
    canvas.objects.push(app.build_plot_object(
        0,
        ObjectFrame::new(0.0, 0.0, 100.0, 80.0),
        id,
        "Map".to_owned(),
    ));
    app.doc.canvases.push(canvas);
    app.session.active_canvas = Some(0);
    let target = app.object_target(0, id).expect("plot target");
    (app, target, id)
}

#[test]
fn an_xy_plot_turns_equal_scale_on_and_resets_to_its_derived_default() {
    let (mut app, target, id) = line_app();
    let resolved = equal_scale(&app, &target);
    assert_eq!(resolved.availability, Availability::Editable);
    assert_eq!(
        resolved.value,
        AggregateValue::Uniform(PropertyValue::Bool(false))
    );
    assert_eq!(resolved.default_value, Some(PropertyValue::Bool(false)));
    assert_eq!(resolved.modified, Some(false));

    write(
        &mut app,
        &target,
        axis::EQUAL_SCALE,
        PropertyValue::Bool(true),
    );
    assert!(figure(&app, id).lock_aspect);
    assert_eq!(equal_scale(&app, &target).modified, Some(true));
    let log = app
        .resolve_property(&PropertyAddress::new(target.clone(), axis::Y_SCALE))
        .expect("scale resolves");
    assert!(matches!(log.availability, Availability::Disabled(reason) if reason.contains("equal")));

    let commit = app
        .plan_property_reset(axis::EQUAL_SCALE, std::slice::from_ref(&target))
        .expect("reset plans");
    app.commit_property(commit);
    assert!(!figure(&app, id).lock_aspect);
    assert_eq!(equal_scale(&app, &target).modified, Some(false));
}

#[test]
fn a_log_axis_disables_equal_scale() {
    let (mut app, target, id) = line_app();
    write(
        &mut app,
        &target,
        axis::Y_SCALE,
        PropertyValue::Enum(axis::LOG),
    );
    assert!(matches!(
        equal_scale(&app, &target).availability,
        Availability::Disabled(_)
    ));
    let commit = app
        .plan_property_write(
            axis::EQUAL_SCALE,
            std::slice::from_ref(&target),
            &PropertyValue::Bool(true),
        )
        .expect("write plans with a skip");
    assert!(commit.applied.is_empty());
    assert!(!figure(&app, id).lock_aspect);
}

#[test]
fn an_afm_map_starts_locked_and_can_turn_equal_scale_off() {
    let (mut app, target, id) = afm_app();
    let resolved = equal_scale(&app, &target);
    assert_eq!(resolved.availability, Availability::Editable);
    assert_eq!(resolved.default_value, Some(PropertyValue::Bool(true)));
    assert!(figure(&app, id).lock_aspect);

    write(
        &mut app,
        &target,
        axis::EQUAL_SCALE,
        PropertyValue::Bool(false),
    );
    assert!(!figure(&app, id).lock_aspect);
    app.undo();
    assert!(figure(&app, id).lock_aspect);
}

#[test]
fn a_shared_x_stack_member_cannot_turn_equal_scale_on() {
    let (mut app, _, first) = line_app();
    let dataset = app.doc.datasets[0].clone();
    app.doc.datasets.push(dataset);
    let second = app.doc.canvases[0].allocate_object_id();
    let object = app.build_plot_object(
        1,
        ObjectFrame::new(0.0, 80.0, 100.0, 70.0),
        second,
        "Second".to_owned(),
    );
    app.doc.canvases[0].objects.push(object);
    let (panel, action) = app
        .compose_panel_action(0, "Stack".to_owned(), &[first, second], 0.0)
        .expect("compose");
    app.execute_action(action);
    let action = app
        .set_shared_x_stack_action(0, panel, true, None)
        .expect("stack");
    app.execute_action(action);

    let target = app.object_target(0, first).expect("plot target");
    assert!(matches!(
        equal_scale(&app, &target).availability,
        Availability::Disabled(reason) if reason.contains("shared-x stack")
    ));
    let commit = app
        .plan_property_write(
            axis::EQUAL_SCALE,
            std::slice::from_ref(&target),
            &PropertyValue::Bool(true),
        )
        .expect("write plans with a skip");
    assert!(commit.applied.is_empty());
}
