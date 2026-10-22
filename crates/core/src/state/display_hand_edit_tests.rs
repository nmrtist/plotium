//! Values the user set by hand survive themes and automation.
use super::*;
use crate::properties::{guide, line_display, typography};
use crate::theme::Theme;

fn current(app: &PlotiumApp, target: &TargetRef, key: properties::PropertyId) -> PropertyValue {
    let resolved = app
        .resolve_property(&PropertyAddress::new(target.clone(), key))
        .unwrap();
    let properties::AggregateValue::Uniform(value) = resolved.value else {
        panic!()
    };
    value
}

fn agent(
    app: &mut PlotiumApp,
    tool_id: &str,
    parameters: serde_json::Value,
    targets: Vec<TargetRef>,
) -> ToolResult {
    let mut request = request(app, "", json!(null), targets);
    request.tool_id = tool_id.into();
    request.parameters = parameters;
    let plan = plan_tool(app, request).unwrap();
    execute_tool(app, plan, ExecutionAuthority::ReversibleModify).unwrap()
}

fn skip_reason<'a>(result: &'a ToolResult, target: &TargetRef) -> Option<&'a str> {
    result
        .targets
        .iter()
        .find(|t| t.target == *target)
        .and_then(|t| t.skip_reason.as_deref())
}

#[test]
fn hand_edits_are_recorded_with_undo_and_forgotten_on_reset() {
    let mut app = fixture();
    let a = target(&app, 0, 1);
    let b = target(&app, 1, 1);
    write(
        &mut app,
        line_display::MARKER_SIZE,
        PropertyValue::Float(5.0),
        std::slice::from_ref(&a),
    );
    assert!(app.is_hand_set(&a, line_display::MARKER_SIZE));
    app.undo();
    assert!(!app.is_hand_set(&a, line_display::MARKER_SIZE));
    app.redo();
    assert!(app.is_hand_set(&a, line_display::MARKER_SIZE));
    assert_eq!(app.live_hand_edits().len(), 1);

    let path = std::env::temp_dir().join(format!("plotium-hand-{}.plotium", uuid::Uuid::new_v4()));
    crate::project::save_project(&app, &path, false).unwrap();
    let loaded = crate::project::load_project(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(loaded.is_hand_set(&a, line_display::MARKER_SIZE));

    let reset = app
        .plan_property_reset(line_display::MARKER_SIZE, std::slice::from_ref(&a))
        .unwrap();
    app.commit_property(reset);
    assert!(!app.is_hand_set(&a, line_display::MARKER_SIZE));

    // An agent's write is not the user's.
    agent(
        &mut app,
        "properties.set",
        json!({"key": "series.line.marker_size", "value": 4.0}),
        vec![b.clone()],
    );
    assert_eq!(
        current(&app, &b, line_display::MARKER_SIZE),
        PropertyValue::Float(4.0)
    );
    assert!(!app.is_hand_set(&b, line_display::MARKER_SIZE));
}

#[test]
fn themes_keep_hand_set_values_and_say_how_many() {
    let mut app = fixture();
    let hand = target(&app, 0, 1);
    let agent_set = target(&app, 1, 1);
    let document = app.document_target();
    write(
        &mut app,
        line_display::MARKER_SIZE,
        PropertyValue::Float(5.0),
        std::slice::from_ref(&hand),
    );
    write(
        &mut app,
        line_display::MARKER_EDGE_WIDTH,
        PropertyValue::Float(0.5),
        std::slice::from_ref(&hand),
    );
    write(
        &mut app,
        typography::TICK_PT,
        PropertyValue::Float(6.5),
        std::slice::from_ref(&document),
    );
    agent(
        &mut app,
        "properties.set",
        json!({"key": "series.line.marker_edge_width", "value": 0.5}),
        vec![agent_set.clone()],
    );
    let before = plot(&app, 0).binding.series.clone();
    let nature = Theme::by_id("nature").unwrap();

    app.session.active_canvas = Some(0);
    app.apply_theme(&nature);

    assert_eq!(
        current(&app, &hand, line_display::MARKER_SIZE),
        PropertyValue::Float(5.0)
    );
    assert_eq!(
        current(&app, &hand, line_display::MARKER_EDGE_WIDTH),
        PropertyValue::Float(0.5)
    );
    let typography = app.doc.style_library.figure_typography;
    assert_eq!(typography.tick_pt, 6.5);
    assert_eq!(typography.label_pt, nature.figure_typography.label_pt);
    assert!(
        app.session.status.contains("Kept 3 values set by hand"),
        "{}",
        app.session.status
    );
    app.undo();
    assert_eq!(plot(&app, 0).binding.series, before);

    // Nature still clears marker outlines nobody drew by hand.
    app.session.active_canvas = Some(1);
    app.apply_theme(&nature);
    assert_eq!(
        current(&app, &agent_set, line_display::MARKER_EDGE_WIDTH),
        PropertyValue::Float(0.)
    );
}

#[test]
fn automation_skips_hand_set_values_unless_the_user_asked() {
    let mut app = fixture();
    let a = target(&app, 0, 1);
    let b = target(&app, 1, 1);
    write(
        &mut app,
        line_display::MARKER_SIZE,
        PropertyValue::Float(5.0),
        std::slice::from_ref(&a),
    );

    let result = agent(
        &mut app,
        "properties.set",
        json!({"key": "series.line.marker_size", "value": 4.0}),
        vec![a.clone(), b.clone()],
    );
    assert_eq!(skip_reason(&result, &a), Some("hand_set"));
    assert_eq!(
        current(&app, &a, line_display::MARKER_SIZE),
        PropertyValue::Float(5.0)
    );
    assert_eq!(
        current(&app, &b, line_display::MARKER_SIZE),
        PropertyValue::Float(4.0)
    );

    let inspected = agent(
        &mut app,
        "properties.inspect",
        json!({"key": "series.line.marker_size"}),
        vec![a.clone(), b.clone()],
    );
    let readings = inspected.value["readings"].as_array().unwrap();
    assert_eq!(readings[0]["hand_set"], json!(true));
    assert!(readings[1].get("hand_set").is_none());

    let result = agent(
        &mut app,
        "properties.set_many",
        json!({"writes": [
            {"target": a, "key": "series.line.marker_size", "value": 3.0},
            {"target": b, "key": "series.line.marker_size", "value": 3.0},
        ]}),
        Vec::new(),
    );
    assert_eq!(skip_reason(&result, &a), Some("hand_set"));
    assert_eq!(result.value["writes"][1]["outcome"], json!("succeeded"));
    assert_eq!(
        current(&app, &a, line_display::MARKER_SIZE),
        PropertyValue::Float(5.0)
    );

    let result = agent(
        &mut app,
        "properties.reset",
        json!({"key": "series.line.marker_size"}),
        vec![a.clone()],
    );
    assert_eq!(skip_reason(&result, &a), Some("hand_set"));

    // Asked by the user, the agent changes it, and the value stays theirs.
    agent(
        &mut app,
        "properties.set",
        json!({"key": "series.line.marker_size", "value": 4.5, "override_hand_edits": true}),
        vec![a.clone()],
    );
    assert_eq!(
        current(&app, &a, line_display::MARKER_SIZE),
        PropertyValue::Float(4.5)
    );
    assert!(app.is_hand_set(&a, line_display::MARKER_SIZE));
}

#[test]
fn a_dragged_legend_keeps_its_placement() {
    let mut app = fixture();
    let plot_target = object(&app, 0);
    let object_id = app.doc.canvases[0].objects[0].id;
    let before = plot(&app, 0).axis_overrides.clone();
    let mut after = before.clone();
    after.set_legend_position([0.2, 0.8]);
    let position = PropertyAddress::new(plot_target.clone(), guide::POSITION);
    app.execute_hand_edit(
        crate::actions::Action::set_axis_overrides(0, object_id, before, after.clone()),
        std::slice::from_ref(&position),
        true,
    );

    let result = agent(
        &mut app,
        "properties.set",
        json!({"key": "object.guides.placement", "value": "inside"}),
        vec![plot_target.clone()],
    );
    assert_eq!(skip_reason(&result, &plot_target), Some("hand_set"));
    assert_eq!(plot(&app, 0).axis_overrides, after);

    // Restoring automatic placement by hand hands the legend back.
    let mut restored = after.clone();
    restored.reset_guide_placement();
    app.execute_hand_edit(
        crate::actions::Action::set_axis_overrides(0, object_id, after, restored),
        &[position],
        false,
    );
    assert!(!app.is_hand_set(&plot_target, guide::PLACEMENT));
}

#[test]
fn resetting_a_value_already_at_its_default_still_releases_it() {
    let mut app = fixture();
    let a = target(&app, 0, 1);
    let default = current(&app, &a, line_display::MARKER_SIZE);
    write(
        &mut app,
        line_display::MARKER_SIZE,
        PropertyValue::Float(5.0),
        std::slice::from_ref(&a),
    );
    write(
        &mut app,
        line_display::MARKER_SIZE,
        default,
        std::slice::from_ref(&a),
    );
    assert!(app.is_hand_set(&a, line_display::MARKER_SIZE));

    let reset = app
        .plan_property_reset(line_display::MARKER_SIZE, std::slice::from_ref(&a))
        .unwrap();
    app.commit_property(reset);
    assert!(!app.is_hand_set(&a, line_display::MARKER_SIZE));
    app.undo();
    assert!(app.is_hand_set(&a, line_display::MARKER_SIZE));
}

#[test]
fn saved_styles_keep_hand_set_marker_outlines() {
    let mut app = fixture();
    let a = target(&app, 0, 1);
    let mut style = app.theme_from_canvas(0, "Lab").unwrap();
    style.marker_palette = vec![Some(crate::theme::MarkerPaint::default()); 2];
    write(
        &mut app,
        line_display::MARKER_EDGE_WIDTH,
        PropertyValue::Float(0.5),
        std::slice::from_ref(&a),
    );

    app.session.active_canvas = Some(0);
    app.apply_theme(&style);

    assert_eq!(
        current(&app, &a, line_display::MARKER_EDGE_WIDTH),
        PropertyValue::Float(0.5)
    );
    assert!(app.session.status.contains("Kept 1 value set by hand"));
}
