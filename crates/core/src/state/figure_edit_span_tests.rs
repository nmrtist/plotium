use super::*;
use plotium_figure::SpanAxis;

#[test]
fn created_span_lowers_behind_data_with_defaults() {
    let (mut a, target) = fixture();
    let result = commit(
        &mut a,
        json!([{"op":"annotation_create","target":target,"kind":"span","axis":"x","range":[0.5,1.5]}]),
    );
    let edit = &result["annotation_edits"][0];
    assert_eq!(edit["annotation_id"], 0);
    assert_eq!(
        edit["annotation"],
        json!({"id":0,"kind":"span","axis":"x","range":[0.5,1.5],"color":"#808080","opacity":0.2,"label":"","font_pt":null,"anchor_space":"data"})
    );
    let figure = plot(&a).figure();
    assert_eq!(figure.spans.len(), 1);
    assert_eq!(figure.spans[0].axis, SpanAxis::X);
    let svg = plotium_render::svg::export(figure);
    let span = svg.find(r#"class="span""#).expect("span rect");
    let data = ["<polyline", "<circle", "<polygon"]
        .iter()
        .filter_map(|mark| svg.find(mark))
        .min()
        .expect("series mark");
    assert!(span < data);
}

#[test]
fn spans_edit_by_kind_and_undo_as_one_step() {
    let (mut a, target) = fixture();
    commit(
        &mut a,
        json!([
            {"op":"annotation_create","target":target,"kind":"reference_line","orientation":"vertical","value":1.0,"label":"onset"},
            {"op":"annotation_create","target":target,"kind":"span","axis":"y","range":[1.0,2.0],"label":"window","color":"#3366CC","opacity":0.4,"font_pt":7}
        ]),
    );
    let result = commit(
        &mut a,
        json!([{"op":"annotation_set","target":target,"annotation_id":1,"axis":"x","range":[2.0,0.5],"label":"  ","opacity":0.1}]),
    );
    assert_eq!(result["annotation_edits"][0]["annotation"]["axis"], "x");
    let span = &plot(&a).axis_overrides.annotations.spans[0];
    assert_eq!((span.axis, span.range), (SpanAxis::X, [2.0, 0.5]));
    assert_eq!(span.label, "");
    assert_eq!(span.opacity, 0.1);
    assert_eq!(span.font_size, Some(7.0));
    assert_eq!(span.color, plotium_figure::Color::rgb(0x33, 0x66, 0xcc));
    assert_eq!(
        plot(&a).axis_overrides.annotations.reference_lines[0].label,
        "onset"
    );
    a.undo();
    assert_eq!(plot(&a).figure().spans[0].axis, SpanAxis::Y);
    commit(
        &mut a,
        json!([{"op":"annotation_delete","target":target,"annotation_id":1}]),
    );
    assert!(plot(&a).figure().spans.is_empty());
    assert_eq!(plot(&a).figure().reference_lines.len(), 1);
}

#[test]
fn hand_moved_spans_keep_their_range_unless_overridden() {
    let (mut a, target) = fixture();
    commit(
        &mut a,
        json!([{"op":"annotation_create","target":target,"kind":"span","axis":"x","range":[0.0,1.0]}]),
    );
    let object = a.doc.canvases[0]
        .objects
        .iter()
        .find(|o| o.plot().is_some())
        .unwrap()
        .id;
    let mut overrides = plot(&a).axis_overrides.clone();
    overrides.annotations.spans[0].hand_moved = true;
    a.set_axis_overrides_value(0, object, &overrides);
    let readings = hand_edit_readings(&a, 0, 16);
    assert_eq!(readings["items"][0]["key"], HAND_MOVED_ANNOTATION_KEY);
    assert_eq!(
        readings["items"][0]["value"],
        json!({"annotation_id":0,"axis":"x","range":[0.0,1.0]})
    );
    a.select_object(0, object);
    a.select_plot_part(0, object, PlotPart::Annotation(AnnotationId::new(0)));
    let part = crate::state::plot_part_reading(&a).expect("selected span");
    assert_eq!(part["kind"], "span");
    assert_eq!(part["hand_moved"], true);
    let moved = json!({"op":"annotation_set","target":target,"annotation_id":0,"range":[0.0,2.0]});
    assert!(try_plan(&a, json!([moved])).is_err());
    let recolored =
        json!({"op":"annotation_set","target":target,"annotation_id":0,"color":"#112233"});
    assert!(try_plan(&a, json!([recolored])).is_ok());
    let mut forced = moved.clone();
    forced["override_hand_edits"] = json!(true);
    commit(&mut a, json!([forced]));
    let span = &plot(&a).axis_overrides.annotations.spans[0];
    assert_eq!(span.range, [0.0, 2.0]);
    assert!(!span.hand_moved);
}

#[test]
fn invalid_span_edits_reject_the_whole_batch() {
    let (mut a, target) = fixture();
    commit(
        &mut a,
        json!([
            {"op":"annotation_create","target":target,"text":"t","at":[0.0,0.0],"font_pt":8},
            {"op":"annotation_create","target":target,"kind":"span","axis":"x","range":[0.0,1.0]}
        ]),
    );
    for edit in [
        json!({"op":"annotation_create","target":target,"kind":"span","range":[0.0,1.0]}),
        json!({"op":"annotation_create","target":target,"kind":"span","axis":"x"}),
        json!({"op":"annotation_create","target":target,"kind":"span","axis":"x","range":[1.0,1.0]}),
        json!({"op":"annotation_create","target":target,"kind":"span","axis":"x","range":[0.0,1.0],"opacity":1.5}),
        json!({"op":"annotation_create","target":target,"kind":"span","axis":"x","range":[0.0,1.0],"color":"blue"}),
        json!({"op":"annotation_create","target":target,"kind":"span","axis":"x","range":[0.0,1.0],"value":1.0}),
        json!({"op":"annotation_create","target":target,"kind":"span","axis":"x","range":[0.0,1.0],"at":[0.0,0.0]}),
        json!({"op":"annotation_create","target":target,"text":"x","at":[0.0,0.0],"font_pt":8,"opacity":0.5}),
        json!({"op":"annotation_create","target":target,"kind":"reference_line","orientation":"vertical","value":1.0,"range":[0.0,1.0]}),
        json!({"op":"annotation_set","target":target,"annotation_id":0,"range":[0.0,1.0]}),
        json!({"op":"annotation_set","target":target,"annotation_id":9,"axis":"y"}),
        json!({"op":"annotation_set","target":target,"annotation_id":1}),
        json!({"op":"annotation_set","target":target,"annotation_id":1,"to":[1.0,1.0]}),
        json!({"op":"annotation_set","target":target,"annotation_id":1,"placement":"auto"}),
        json!({"op":"annotation_set","target":target,"annotation_id":1,"range":[0.0,0.0]}),
    ] {
        assert!(try_plan(&a, json!([edit])).is_err(), "{edit}");
    }
}

#[test]
fn spans_round_trip_through_the_project_file_and_inspection() {
    let (mut a, target) = fixture();
    commit(
        &mut a,
        json!([
            {"op":"annotation_create","target":target,"kind":"span","axis":"y","range":[1.0,2.5],"label":"band","color":"#336699","opacity":0.35,"font_pt":9}
        ]),
    );
    let dir = std::env::temp_dir().join(format!("plotium-spans-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("spans.plotium");
    crate::project::save_project(&a, &path, false).unwrap();
    let mut loaded = crate::project::load_project(&path).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
    assert_eq!(
        plot(&loaded).axis_overrides.annotations,
        plot(&a).axis_overrides.annotations
    );
    assert_eq!(plot(&loaded).figure().spans[0].label, "band");

    let request = ToolRequest {
        tool_id: DOCUMENT_INSPECT.into(),
        tool_version: 1,
        parameters: json!({"target":target,"view":"self"}),
        targets: TargetSelector::Explicit {
            ids: vec![target.resource.id.clone()],
        },
        expected_revision: DocumentRevision(loaded.doc.automation_revision),
        caller: CallerType::Agent,
    };
    let plan = plan_tool(&loaded, request).unwrap();
    let result = execute_tool(&mut loaded, plan, ExecutionAuthority::Read).unwrap();
    assert_eq!(result.value["node"]["annotations"][0]["kind"], "span");
    assert_eq!(result.value["node"]["annotations"][0]["opacity"], 0.35);
}
