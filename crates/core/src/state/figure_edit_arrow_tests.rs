use super::*;

fn tail_px(figure: &plotium_figure::Figure) -> (f32, f32) {
    let arrow = &figure.author_arrows[0];
    let outer = plotium_render::Rect::new(0.0, 0.0, figure.width, figure.height);
    let margins = plotium_render::Margins::for_figure(figure);
    plotium_render::Projector::new(figure, outer, &margins).project(arrow.from)
}

#[test]
fn created_arrow_renders_in_data_space_with_defaults() {
    let (mut a, target) = fixture();
    let result = commit(
        &mut a,
        json!([{"op":"annotation_create","target":target,"kind":"arrow","at":[0.5,1.0],"to":[1.5,3.0]}]),
    );
    let edit = &result["annotation_edits"][0];
    assert_eq!(edit["annotation_id"], 0);
    assert_eq!(
        edit["annotation"],
        json!({"id":0,"kind":"arrow","at":[0.5,1.0],"to":[1.5,3.0],"head":"filled","line_width_pt":1.0,"color":"#000000","anchor_space":"data"})
    );
    let figure = plot(&a).figure();
    assert_eq!(figure.author_arrows.len(), 1);
    assert!(figure.author_annotations.is_empty());
    assert!(plotium_render::svg::export(figure).contains(r#"class="annotation-arrow""#));
    let before = tail_px(figure);

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
    assert_eq!(figure.author_arrows[0].from, [0.5, 1.0]);
    assert!((tail_px(figure).0 - before.0).abs() > 1.0);
}

#[test]
fn arrows_and_text_share_ids_and_edit_by_kind() {
    let (mut a, target) = fixture();
    commit(
        &mut a,
        json!([
            {"op":"annotation_create","target":target,"text":"peak","at":[1.0,3.0],"font_pt":8},
            {"op":"annotation_create","target":target,"kind":"arrow","at":[1.0,3.0],"to":[1.5,2.0]}
        ]),
    );
    let result = commit(
        &mut a,
        json!([{"op":"annotation_set","target":target,"annotation_id":1,"to":[2.0,2.5],"head":"double","line_width_pt":0.5,"color":"#C03020"}]),
    );
    assert_eq!(
        result["annotation_edits"][0]["annotation"]["head"],
        "double"
    );
    let arrow = &plot(&a).axis_overrides.annotations.arrows[0];
    assert_eq!(arrow.to, [2.0, 2.5]);
    assert_eq!(arrow.line_width, 0.5);
    assert_eq!(arrow.color, plotium_figure::Color::rgb(0xc0, 0x30, 0x20));
    let lowered = &plot(&a).figure().author_arrows[0];
    assert_eq!(lowered.head, plotium_figure::ArrowHead::Double);
    assert_eq!(lowered.width, 0.5);

    commit(
        &mut a,
        json!([{"op":"annotation_delete","target":target,"annotation_id":0}]),
    );
    assert!(plot(&a).axis_overrides.annotations.items.is_empty());
    assert_eq!(plot(&a).figure().author_arrows.len(), 1);
    a.undo();
    commit(
        &mut a,
        json!([{"op":"annotation_delete","target":target,"annotation_id":1}]),
    );
    assert!(plot(&a).axis_overrides.annotations.arrows.is_empty());
    assert!(plot(&a).figure().author_arrows.is_empty());
    assert_eq!(plot(&a).axis_overrides.annotations.items.len(), 1);
    a.undo();
    assert_eq!(plot(&a).figure().author_arrows.len(), 1);
    let result = commit(
        &mut a,
        json!([{"op":"annotation_create","target":target,"kind":"arrow","at":[0.0,0.0],"to":[1.0,1.0],"head":"none"}]),
    );
    assert_eq!(result["annotation_edits"][0]["annotation_id"], 2);
}

#[test]
fn invalid_arrow_edits_reject_the_whole_batch() {
    let (mut a, target) = fixture();
    commit(
        &mut a,
        json!([
            {"op":"annotation_create","target":target,"text":"t","at":[0.0,0.0],"font_pt":8},
            {"op":"annotation_create","target":target,"kind":"arrow","at":[0.0,0.0],"to":[1.0,1.0]}
        ]),
    );
    for edit in [
        json!({"op":"annotation_create","target":target,"kind":"arrow","at":[0.0,0.0]}),
        json!({"op":"annotation_create","target":target,"kind":"arrow","at":[1.0,1.0],"to":[1.0,1.0]}),
        json!({"op":"annotation_create","target":target,"kind":"arrow","at":[0.0,0.0],"to":[1.0,1.0],"line_width_pt":0}),
        json!({"op":"annotation_create","target":target,"kind":"arrow","at":[0.0,0.0],"to":[1.0,1.0],"color":"red"}),
        json!({"op":"annotation_create","target":target,"kind":"arrow","at":[0.0,0.0],"to":[1.0,1.0],"text":"x"}),
        json!({"op":"annotation_create","target":target,"text":"x","at":[0.0,0.0],"font_pt":8,"to":[1.0,1.0]}),
        json!({"op":"annotation_create","target":target,"at":[0.0,0.0],"font_pt":8}),
        json!({"op":"annotation_create","target":target,"kind":"arrow","at":[0.0,0.0],"to":[1.0,1.0],"value":1.0}),
        json!({"op":"annotation_create","target":target,"kind":"reference_line","orientation":"vertical","value":1.0,"head":"open"}),
        json!({"op":"annotation_set","target":target,"annotation_id":1,"line":{"width_pt":2.0}}),
        json!({"op":"annotation_set","target":target,"annotation_id":9,"to":[1.0,1.0]}),
        json!({"op":"annotation_set","target":target,"annotation_id":0,"head":"open"}),
        json!({"op":"annotation_set","target":target,"annotation_id":1,"font_pt":9}),
        json!({"op":"annotation_set","target":target,"annotation_id":1}),
        json!({"op":"annotation_set","target":target,"annotation_id":1,"to":[0.0,0.0]}),
    ] {
        assert!(try_plan(&a, json!([edit])).is_err(), "{edit}");
    }
}

#[test]
fn arrows_round_trip_through_the_project_file_and_inspection() {
    let (mut a, target) = fixture();
    commit(
        &mut a,
        json!([
            {"op":"annotation_create","target":target,"text":"label","at":[0.0,1.0],"font_pt":8},
            {"op":"annotation_create","target":target,"kind":"arrow","at":[0.0,1.0],"to":[1.5,2.5],"head":"open","line_width_pt":1.5,"color":"#336699"}
        ]),
    );
    let dir = std::env::temp_dir().join(format!("plotium-arrows-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("arrows.plotium");
    crate::project::save_project(&a, &path, false).unwrap();
    let mut loaded = crate::project::load_project(&path).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
    assert_eq!(
        plot(&loaded).axis_overrides.annotations,
        plot(&a).axis_overrides.annotations
    );
    assert_eq!(
        plot(&loaded).figure().author_arrows[0].head,
        plotium_figure::ArrowHead::Open
    );

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
    let kinds = result.value["node"]["annotations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["kind"].clone())
        .collect::<Vec<_>>();
    assert_eq!(kinds, [json!("text"), json!("arrow")]);
}
