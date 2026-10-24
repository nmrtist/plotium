use super::*;
use crate::automation::*;
use plotium_figure::{ReferenceLabelPosition, ReferenceLabelSide};
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
    commit(
        &mut a,
        json!([
            {"op":"annotation_create","target":target,"text":"peak","at":[1.0,2.0],"font_pt":7},
            {"op":"annotation_create","target":target,"kind":"reference_line","orientation":"vertical","value":1.5,"label":"onset"}
        ]),
    );
    (a, target)
}

fn try_plan(a: &PlotiumApp, edits: Value) -> Result<FigureEditPlan, String> {
    let canvas = TargetRef::resource(a.doc.canvases[0].resource_id.into());
    let params = serde_json::from_value(json!({"canvas":canvas,"edits":edits}))
        .map_err(|e| e.to_string())?;
    a.plan_figure_edit(DocumentRevision(a.doc.automation_revision), &params)
}

fn commit(a: &mut PlotiumApp, edits: Value) -> Value {
    let plan = try_plan(a, edits).unwrap();
    let result = plan.result.clone();
    a.commit_figure_edit(plan).unwrap();
    result
}

fn object(a: &PlotiumApp) -> ObjectId {
    a.doc.canvases[0]
        .objects
        .iter()
        .find(|o| o.plot().is_some())
        .unwrap()
        .id
}

fn annotations(a: &PlotiumApp) -> &PlotAnnotations {
    &a.doc.canvases[0]
        .object(object(a))
        .and_then(ContentItem::plot)
        .unwrap()
        .axis_overrides
        .annotations
}

/// Place both labels by hand, as a canvas drag does.
fn place_by_hand(a: &mut PlotiumApp) {
    let id = object(a);
    let mut overrides = a.doc.canvases[0]
        .object(id)
        .and_then(ContentItem::plot)
        .unwrap()
        .axis_overrides
        .clone();
    overrides.annotations.items[0].placement = LabelPlacement::Hand([4.0, 6.0]);
    overrides.annotations.reference_lines[0].label_placement =
        LabelPlacement::Hand(ReferenceLabelPosition {
            along: 0.25,
            side: ReferenceLabelSide::Before,
        });
    a.set_axis_overrides_value(0, id, &overrides);
}

#[test]
fn agents_pin_a_text_offset_and_return_it_to_auto() {
    let (mut a, target) = fixture();
    let result = commit(
        &mut a,
        json!([{"op":"annotation_set","target":target,"annotation_id":0,"offset_pt":[0,0]}]),
    );
    let reading = &result["annotation_edits"][0]["annotation"];
    assert_eq!(reading["placement"], "fixed");
    assert_eq!(reading["offset_pt"], json!([0.0, 0.0]));
    let figure = a.doc.canvases[0]
        .object(object(&a))
        .unwrap()
        .plot()
        .unwrap()
        .figure();
    assert_eq!(figure.author_annotations[0].offset_pt, Some([0.0, 0.0]));
    commit(
        &mut a,
        json!([{"op":"annotation_set","target":target,"annotation_id":0,"placement":"auto"}]),
    );
    assert_eq!(annotations(&a).items[0].placement, LabelPlacement::Auto);
}

#[test]
fn inspection_reports_drawn_placement() {
    let (a, _) = fixture();
    let readings = annotation_readings(&a.doc.canvases[0], object(&a));
    assert_eq!(readings[0]["placement"], "auto");
    assert_eq!(readings[0]["offset_pt"].as_array().map(Vec::len), Some(2));
    assert_eq!(readings[1]["label_placement"], "auto");
    assert_eq!(readings[1]["label_position"]["along"], 1.0);
    assert!(matches!(
        readings[1]["label_position"]["side"].as_str(),
        Some("left" | "right")
    ));
}

#[test]
fn hand_placed_labels_survive_agent_edits_unless_overridden() {
    let (mut a, target) = fixture();
    place_by_hand(&mut a);
    commit(
        &mut a,
        json!([
            {"op":"annotation_set","target":target,"annotation_id":0,"text":"moved","at":[1.0,2.0]},
            {"op":"annotation_set","target":target,"annotation_id":1,"label":"start"}
        ]),
    );
    assert_eq!(
        annotations(&a).items[0].placement,
        LabelPlacement::Hand([4.0, 6.0])
    );
    assert!(annotations(&a).reference_lines[0].label_placement.is_hand());
    for edit in [
        // A new anchor would carry the hand-placed text along with it.
        json!({"op":"annotation_set","target":target,"annotation_id":0,"at":[1.5,2.0]}),
        json!({"op":"annotation_set","target":target,"annotation_id":0,"offset_pt":[0,0]}),
        json!({"op":"annotation_set","target":target,"annotation_id":0,"placement":"auto"}),
        json!({"op":"annotation_set","target":target,"annotation_id":1,"placement":"auto"}),
    ] {
        let error = try_plan(&a, json!([edit])).map(|_| ()).unwrap_err();
        assert!(error.contains("override_hand_edits"), "{error}");
    }
    commit(
        &mut a,
        json!([
            {"op":"annotation_set","target":target,"annotation_id":0,"at":[1.5,2.0],"placement":"auto","override_hand_edits":true},
            {"op":"annotation_set","target":target,"annotation_id":1,"placement":"auto","override_hand_edits":true}
        ]),
    );
    assert_eq!(annotations(&a).items[0].at, [1.5, 2.0]);
    assert_eq!(annotations(&a).items[0].placement, LabelPlacement::Auto);
    assert_eq!(
        annotations(&a).reference_lines[0].label_placement,
        LabelPlacement::Auto
    );
}

#[test]
fn hand_placed_labels_are_listed_with_hand_edits() {
    let (mut a, _) = fixture();
    assert_eq!(hand_edit_readings(&a, 0, 16)["total"], 0);
    place_by_hand(&mut a);
    let readings = hand_edit_readings(&a, 0, 16);
    assert_eq!(readings["total"], 2);
    let items = readings["items"].as_array().unwrap();
    assert!(
        items
            .iter()
            .all(|item| item["key"] == HAND_PLACED_LABEL_KEY)
    );
    assert_eq!(items[0]["value"]["offset_pt"], json!([4.0, 6.0]));
    assert_eq!(items[1]["value"]["side"], "left");
}

#[test]
fn placement_fields_reject_the_wrong_kind_or_both_at_once() {
    let (mut a, target) = fixture();
    commit(
        &mut a,
        json!([{"op":"annotation_create","target":target,"kind":"arrow","at":[0.0,0.0],"to":[1.0,1.0]}]),
    );
    for edit in [
        json!({"op":"annotation_set","target":target,"annotation_id":0,"offset_pt":[1,1],"placement":"auto"}),
        json!({"op":"annotation_set","target":target,"annotation_id":1,"offset_pt":[1,1]}),
        json!({"op":"annotation_set","target":target,"annotation_id":2,"placement":"auto"}),
        json!({"op":"annotation_set","target":target,"annotation_id":0,"offset_pt":[1e6,0]}),
    ] {
        assert!(try_plan(&a, json!([edit.clone()])).is_err(), "{edit}");
    }
}

#[test]
fn label_placement_round_trips_through_the_project_file() {
    let (mut a, target) = fixture();
    commit(
        &mut a,
        json!([{"op":"annotation_create","target":target,"text":"pinned","at":[0.5,1.0],"font_pt":7}]),
    );
    commit(
        &mut a,
        json!([{"op":"annotation_set","target":target,"annotation_id":2,"offset_pt":[-3,2]}]),
    );
    place_by_hand(&mut a);
    let dir = std::env::temp_dir().join(format!("plotium-labels-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("labels.plotium");
    crate::project::save_project(&a, &path, false).unwrap();
    let loaded = crate::project::load_project(&path).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
    assert_eq!(annotations(&loaded), annotations(&a));
    assert_eq!(
        annotations(&loaded).items[1].placement,
        LabelPlacement::Fixed([-3.0, 2.0])
    );
}

/// Move the reference line and a new arrow by hand, as a canvas drag does.
fn move_by_hand(a: &mut PlotiumApp, target: &TargetRef) {
    commit(
        a,
        json!([{"op":"annotation_create","target":target,"kind":"arrow","at":[0.0,0.0],"to":[1.0,1.0]}]),
    );
    let id = object(a);
    let mut overrides = annotations_overrides(a);
    overrides.annotations.reference_lines[0].value = 1.25;
    overrides.annotations.reference_lines[0].hand_moved = true;
    overrides.annotations.arrows[0].to = [1.5, 1.0];
    overrides.annotations.arrows[0].hand_moved = true;
    a.set_axis_overrides_value(0, id, &overrides);
}

fn annotations_overrides(a: &PlotiumApp) -> AxisOverrides {
    a.doc.canvases[0]
        .object(object(a))
        .and_then(ContentItem::plot)
        .unwrap()
        .axis_overrides
        .clone()
}

#[test]
fn hand_moved_lines_and_arrows_keep_their_position_unless_overridden() {
    let (mut a, target) = fixture();
    move_by_hand(&mut a, &target);
    // Restyling and unchanged positions pass.
    commit(
        &mut a,
        json!([
            {"op":"annotation_set","target":target,"annotation_id":1,"value":1.25,"line":{"color":"#C03020"}},
            {"op":"annotation_set","target":target,"annotation_id":2,"to":[1.5,1.0],"head":"open"}
        ]),
    );
    for edit in [
        json!({"op":"annotation_set","target":target,"annotation_id":1,"value":2.0}),
        json!({"op":"annotation_set","target":target,"annotation_id":1,"orientation":"horizontal"}),
        json!({"op":"annotation_set","target":target,"annotation_id":2,"at":[0.5,0.0]}),
        json!({"op":"annotation_set","target":target,"annotation_id":2,"to":[2.0,1.0]}),
    ] {
        let error = try_plan(&a, json!([edit.clone()])).map(|_| ()).unwrap_err();
        assert!(error.contains("override_hand_edits"), "{edit}: {error}");
    }
    let readings = hand_edit_readings(&a, 0, 16);
    let moved: Vec<_> = readings["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["key"] == HAND_MOVED_ANNOTATION_KEY)
        .collect();
    assert_eq!(moved.len(), 2);
    commit(
        &mut a,
        json!([
            {"op":"annotation_set","target":target,"annotation_id":1,"value":2.0,"override_hand_edits":true},
            {"op":"annotation_set","target":target,"annotation_id":2,"to":[2.0,1.0],"override_hand_edits":true}
        ]),
    );
    let annotations = annotations(&a);
    assert_eq!(annotations.reference_lines[0].value, 2.0);
    assert_eq!(annotations.arrows[0].to, [2.0, 1.0]);
    // The agent's position is no longer the user's.
    assert!(!annotations.reference_lines[0].hand_moved);
    assert!(!annotations.arrows[0].hand_moved);
}

#[test]
fn hand_moves_round_trip_through_the_project_file() {
    let (mut a, target) = fixture();
    move_by_hand(&mut a, &target);
    let dir = std::env::temp_dir().join(format!("plotium-moves-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("moves.plotium");
    crate::project::save_project(&a, &path, false).unwrap();
    let loaded = crate::project::load_project(&path).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
    assert_eq!(annotations(&loaded), annotations(&a));
    assert!(annotations(&loaded).arrows[0].hand_moved);
}
