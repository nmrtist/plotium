use super::*;
use crate::automation::*;
use serde_json::{Value, json};

fn fixture() -> (PlotiumApp, TargetRef) {
    let mut a = super::curve_tests::fixture(false);
    let page = &a.doc.canvases[0];
    let plot = page
        .objects
        .iter()
        .find(|o| o.plot().is_some())
        .expect("fixture plot");
    let target = TargetRef::resource(canvas_object_ref(page.resource_id, plot.id));
    a.clear_history();
    (a, target)
}

fn try_plan(a: &PlotiumApp, edits: Value) -> Result<FigureEditPlan, String> {
    let canvas = TargetRef::resource(a.doc.canvases[0].resource_id.into());
    a.plan_figure_edit(
        DocumentRevision(a.doc.automation_revision),
        &serde_json::from_value(json!({"canvas":canvas,"edits":edits})).unwrap(),
    )
}

fn commit(a: &mut PlotiumApp, edits: Value) -> Value {
    let plan = try_plan(a, edits).unwrap();
    let result = plan.result.clone();
    a.commit_figure_edit(plan).unwrap();
    result
}

fn plot(a: &PlotiumApp) -> &PlotObject {
    a.doc.canvases[0]
        .objects
        .iter()
        .find_map(ContentItem::plot)
        .unwrap()
}

fn anchor_px(figure: &plotium_figure::Figure) -> f32 {
    let annotation = &figure.author_annotations[0];
    let outer = plotium_render::Rect::new(0.0, 0.0, figure.width, figure.height);
    let margins = plotium_render::Margins::for_figure(figure);
    plotium_render::Projector::new(figure, outer, &margins)
        .project(annotation.at)
        .0
}

#[test]
fn created_annotation_renders_in_data_space_and_follows_axis_range() {
    let (mut a, target) = fixture();
    let result = commit(
        &mut a,
        json!([{"op":"annotation_create","target":target,"text":"4.8 V","at":[1.0,2.0],"font_pt":7}]),
    );
    assert_eq!(result["annotation_edits"][0]["annotation_id"], 0);
    assert_eq!(
        result["annotation_edits"][0]["annotation"]["at"],
        json!([1.0, 2.0])
    );
    let figure = plot(&a).figure();
    assert_eq!(figure.author_annotations.len(), 1);
    assert!(plotium_render::svg::export(figure).contains("4.8 V"));
    let before = anchor_px(figure);

    let object = a.doc.canvases[0]
        .objects
        .iter()
        .find(|o| o.plot().is_some())
        .unwrap()
        .id;
    let mut overrides = plot(&a).axis_overrides.clone();
    overrides.x_range = Some(AxisRange { min: 0.0, max: 4.0 });
    a.set_axis_overrides_value(0, object, &overrides);
    let figure = plot(&a).figure();
    assert_eq!(figure.author_annotations[0].at, [1.0, 2.0]);
    assert!((anchor_px(figure) - before).abs() > 1.0);
}

#[test]
fn set_and_delete_address_stable_ids_and_undo_as_one_step() {
    let (mut a, target) = fixture();
    commit(
        &mut a,
        json!([
            {"op":"annotation_create","target":target,"text":"a","at":[0.0,1.0],"font_pt":8},
            {"op":"annotation_create","target":target,"text":"b","at":[1.0,1.0],"font_pt":8}
        ]),
    );
    commit(
        &mut a,
        json!([
            {"op":"annotation_set","target":target,"annotation_id":1,"text":"b2","at":[2.0,3.0]},
            {"op":"annotation_delete","target":target,"annotation_id":0}
        ]),
    );
    let annotations = &plot(&a).axis_overrides.annotations;
    assert_eq!(annotations.items.len(), 1);
    assert_eq!(annotations.items[0].text, "b2");
    assert_eq!(annotations.items[0].at, [2.0, 3.0]);
    assert_eq!(plot(&a).figure().author_annotations[0].text, "b2");

    let stale = try_plan(
        &a,
        json!([{"op":"annotation_set","target":target,"annotation_id":0,"text":"x"}]),
    );
    assert!(stale.is_err());
    let result = commit(
        &mut a,
        json!([{"op":"annotation_create","target":target,"text":"c","at":[0.0,0.0],"font_pt":8}]),
    );
    assert_eq!(result["annotation_edits"][0]["annotation_id"], 2);

    a.undo();
    a.undo();
    let annotations = &plot(&a).axis_overrides.annotations;
    assert_eq!(annotations.items.len(), 2);
    assert_eq!(plot(&a).figure().author_annotations[1].text, "b");
}

#[test]
fn invalid_annotation_edits_reject_the_whole_batch() {
    let (a, target) = fixture();
    for edit in [
        json!({"op":"annotation_create","target":target,"text":" ","at":[0.0,0.0],"font_pt":8}),
        json!({"op":"annotation_create","target":target,"text":"a","at":[0.0,0.0],"font_pt":0.5}),
        json!({"op":"annotation_set","target":target,"annotation_id":0}),
        json!({"op":"annotation_delete","target":target,"annotation_id":9}),
    ] {
        assert!(try_plan(&a, json!([edit])).is_err());
    }
    let canvas = TargetRef::resource(a.doc.canvases[0].resource_id.into());
    let not_a_plot = try_plan(
        &a,
        json!([{"op":"annotation_create","target":canvas,"text":"a","at":[0.0,0.0],"font_pt":8}]),
    );
    assert!(not_a_plot.is_err());
}

#[test]
fn annotations_round_trip_through_the_project_file_with_their_allocator() {
    let (mut a, target) = fixture();
    commit(
        &mut a,
        json!([
            {"op":"annotation_create","target":target,"text":"gone","at":[0.0,1.0],"font_pt":8},
            {"op":"annotation_create","target":target,"text":"kept","at":[1.5,2.5],"font_pt":9}
        ]),
    );
    commit(
        &mut a,
        json!([{"op":"annotation_delete","target":target,"annotation_id":0}]),
    );
    let dir = std::env::temp_dir().join(format!("plotium-annotations-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("annotations.plotium");
    crate::project::save_project(&a, &path, false).unwrap();
    let loaded = crate::project::load_project(&path).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
    assert_eq!(
        plot(&loaded).axis_overrides.annotations,
        plot(&a).axis_overrides.annotations
    );
    assert_eq!(plot(&loaded).figure().author_annotations[0].text, "kept");
    assert_eq!(
        plot(&loaded).axis_overrides.annotations.next_id,
        AnnotationId::new(2)
    );
}
