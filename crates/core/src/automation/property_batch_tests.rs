use super::*;
use crate::properties::tests::contour_app;
use crate::properties::{AggregateValue, PropertyAddress, axis, contour, line_display};
use crate::state::SeriesBinding;

/// The contour fixture plus two line series, returning their targets.
fn two_line_series() -> (PlotiumApp, TargetRef, TargetRef) {
    let (mut app, contour_target) = contour_app();
    let plot = app.doc.canvases[0].objects[0]
        .plot_mut()
        .expect("the fixture holds a plot");
    let mut ids = Vec::new();
    for _ in 0..2 {
        let mut series = plot.binding.series[0].clone();
        series.encoding =
            plotium_figure::SeriesEncoding::Line(plotium_figure::LineEncoding::default());
        let id = plot.allocate_series_id();
        plot.binding.series.push(SeriesBinding { id, ..series });
        ids.push(id);
    }
    let target = |id| TargetRef {
        resource: contour_target.resource.clone(),
        component: Some(ComponentRef::Series(id)),
    };
    (app, target(ids[0]), target(ids[1]))
}

fn batch(app: &PlotiumApp, writes: serde_json::Value) -> ToolRequest {
    ToolRequest {
        tool_id: TOOL_SET_MANY.to_owned(),
        tool_version: 1,
        parameters: serde_json::json!({ "writes": writes }),
        targets: TargetSelector::Explicit { ids: Vec::new() },
        expected_revision: DocumentRevision(app.doc.automation_revision),
        caller: CallerType::Agent,
    }
}

fn run(app: &mut PlotiumApp, writes: serde_json::Value) -> Result<ToolResult, AutomationError> {
    let plan = plan_tool(app, batch(app, writes))?;
    let authority = plan.required_authority;
    execute_tool(app, plan, authority)
}

fn read(
    app: &PlotiumApp,
    target: &TargetRef,
    property: PropertyId,
) -> AggregateValue<PropertyValue> {
    app.resolve_property(&PropertyAddress::new(target.clone(), property))
        .unwrap()
        .value
}

fn text(value: &str) -> AggregateValue<PropertyValue> {
    AggregateValue::Uniform(PropertyValue::Text(value.to_owned()))
}

#[test]
fn distinct_values_commit_atomically_as_one_undo_step() {
    let (mut app, a, b) = two_line_series();
    let object = TargetRef::resource(a.resource.clone());
    let undo_before = app.session.undo_stack.len();
    let result = run(
        &mut app,
        serde_json::json!([
            {"target": a, "key": "series.line.color", "value": "#0072b2"},
            {"target": a, "key": "series.label", "value": "In-house AEM, I⁻ form"},
            {"target": b, "key": "series.line.color", "value": "#d55e00"},
            {"target": b, "key": "series.label", "value": "Resin"},
            {"target": object, "key": "object.axes.x_label", "value": "Temperature (°C)"},
        ]),
    )
    .unwrap();
    assert_eq!(result.targets.len(), 5);
    assert!(
        result
            .targets
            .iter()
            .all(|t| t.outcome == TargetOutcome::Succeeded)
    );
    assert_eq!(result.value["writes"][1]["key"], "series.label");
    assert_eq!(result.modified, vec![a.resource.clone()]);
    assert_eq!(app.session.undo_stack.len(), undo_before + 1);
    assert_eq!(
        read(&app, &a, line_display::LABEL),
        text("In-house AEM, I⁻ form")
    );
    assert_eq!(read(&app, &b, line_display::LABEL), text("Resin"));
    assert_eq!(read(&app, &object, axis::X_LABEL), text("Temperature (°C)"));
    let colour = |target| match read(&app, target, line_display::COLOR) {
        AggregateValue::Uniform(PropertyValue::Color(c)) => c.to_hex(),
        other => panic!("{other:?}"),
    };
    assert_eq!(colour(&a), "#0072b2");
    assert_eq!(colour(&b), "#d55e00");

    app.undo();
    assert_ne!(
        read(&app, &a, line_display::LABEL),
        text("In-house AEM, I⁻ form")
    );
    assert_ne!(read(&app, &b, line_display::LABEL), text("Resin"));
    assert_ne!(read(&app, &object, axis::X_LABEL), text("Temperature (°C)"));
}

#[test]
fn an_inapplicable_or_ambiguous_write_refuses_the_whole_batch() {
    let (mut app, a, b) = two_line_series();
    let revision = app.doc.automation_revision;
    let inapplicable = serde_json::json!([
            {"target": a, "key": "series.label", "value": "kept out"},
            {"target": b, "key": contour::COUNT.as_str(), "value": 12},
    ]);
    let error = run(&mut app, inapplicable).unwrap_err().to_string();
    assert!(error.contains("writes[1]"), "{error}");
    let duplicate = batch(
        &app,
        serde_json::json!([
            {"target": a, "key": "series.label", "value": "one"},
            {"target": a, "key": "series.label", "value": "two"},
        ]),
    );
    assert!(matches!(
        plan_tool(&app, duplicate),
        Err(AutomationError::InvalidParameters { .. })
    ));
    let mut selected = batch(
        &app,
        serde_json::json!([{"target": a, "key": "series.label", "value": "x"}]),
    );
    selected.targets = TargetSelector::Explicit {
        ids: vec![a.resource.id.clone()],
    };
    assert!(matches!(
        plan_tool(&app, selected),
        Err(AutomationError::InvalidSelector(_))
    ));
    assert!(matches!(
        plan_tool(&app, batch(&app, serde_json::json!([]))),
        Err(AutomationError::InvalidParameters { .. })
    ));
    assert_eq!(app.doc.automation_revision, revision);
    assert_ne!(read(&app, &a, line_display::LABEL), text("kept out"));
}

#[test]
fn a_write_already_at_its_value_is_skipped_while_the_rest_apply() {
    let (mut app, a, b) = two_line_series();
    run(
        &mut app,
        serde_json::json!([{"target": a, "key": "series.label", "value": "same"}]),
    )
    .unwrap();
    let result = run(
        &mut app,
        serde_json::json!([
            {"target": a, "key": "series.label", "value": "same"},
            {"target": b, "key": "series.label", "value": "new"},
        ]),
    )
    .unwrap();
    assert_eq!(result.targets[0].outcome, TargetOutcome::Skipped);
    assert_eq!(
        result.targets[0].skip_reason.as_deref(),
        Some(SkipReason::AlreadyAtValue.as_str())
    );
    assert_eq!(result.targets[1].outcome, TargetOutcome::Succeeded);
    assert_eq!(result.value["writes"][0]["outcome"], "skipped");
    assert_eq!(read(&app, &b, line_display::LABEL), text("new"));
}
