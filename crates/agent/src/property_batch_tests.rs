use crate::session::Session;
use plotium_core::{
    automation::*,
    properties::{self, AggregateValue, PropertyAddress, PropertyValue},
};
use serde_json::{Value, json};

fn plan(
    session: &mut Session,
    app: &mut plotium_core::state::PlotiumApp,
    mut args: Value,
) -> Result<Value, crate::Failure> {
    args["tool_id"] = json!(TOOL_SET_MANY);
    args["expected_revision"] = json!(app.doc.automation_revision);
    args["expected_project_context"] = json!(app.session.project_context);
    session.call(app, "plotium_plan", args)
}

#[test]
fn one_plan_writes_distinct_values_to_several_series() {
    let (mut app, targets) = crate::component_tests::fixture();
    let (a, b) = (&targets[0], &targets[1]);
    let object = TargetRef::resource(a.resource.clone());
    let mut session = Session::default();
    session.allow_modify = true;
    let undo_before = app.session.undo_stack.len();
    let preflight = plan(
        &mut session,
        &mut app,
        json!({"parameters": {"writes": [
            {"target": a, "key": "series.line.color", "value": "#0072b2"},
            {"target": a, "key": "series.label", "value": "Sample 1"},
            {"target": b, "key": "series.line.color", "value": "#d55e00"},
            {"target": b, "key": "series.label", "value": "Sample 2"},
            {"target": object, "key": "object.axes.y_label", "value": "Mass (%)"},
        ]}}),
    )
    .unwrap();
    assert_eq!(preflight["preflight"]["summary"]["compatible"], 5);
    assert_eq!(preflight["preflight"]["targets"], json!([]));
    assert!(preflight["preflight"].get("request").is_none());
    assert!(preflight["preflight"].get("frozen_targets").is_none());
    let result = session
        .call(
            &mut app,
            "plotium_execute",
            json!({"plan_id": preflight["plan_id"]}),
        )
        .unwrap();
    assert_eq!(result["summary"]["succeeded"], 5, "{result}");
    assert_eq!(result["targets"], json!([]));
    assert_eq!(result["value"]["writes"], json!([]));
    assert_eq!(app.session.undo_stack.len(), undo_before + 1);
    let label = |target: &TargetRef| {
        app.resolve_property(&PropertyAddress::new(
            target.clone(),
            properties::line_display::LABEL,
        ))
        .unwrap()
        .value
    };
    assert_eq!(
        label(a),
        AggregateValue::Uniform(PropertyValue::Text("Sample 1".into()))
    );
    assert_eq!(
        label(b),
        AggregateValue::Uniform(PropertyValue::Text("Sample 2".into()))
    );
}

#[test]
fn batches_stay_inside_the_connection_boundary() {
    let (mut app, targets) = crate::component_tests::fixture();
    let a = &targets[0];
    let mut session = Session::default();
    session.allow_modify = true;
    let write = json!({"target": a, "key": "series.label", "value": "x"});
    let selector = plan(
        &mut session,
        &mut app,
        json!({"ids": [a.resource.id], "parameters": {"writes": [write]}}),
    )
    .unwrap_err();
    assert_eq!(selector.code, "invalid_parameters");
    let outside = plan(
        &mut session,
        &mut app,
        json!({"parameters": {"writes": [
            write,
            {"target": a, "key": "series.contour.ratio", "value": 2.0},
        ]}}),
    )
    .unwrap_err();
    assert_eq!(outside.code, "unsupported_property");
    assert_eq!(outside.details.as_ref().unwrap()["write_index"], 1);
    assert_eq!(
        outside.details.as_ref().unwrap()["key"],
        "series.contour.ratio"
    );
    assert_eq!(
        outside.value()["error"]["recovery"]["next_action"]["tool"],
        "plotium_properties_list"
    );
    assert_eq!(outside.value()["error"]["recovery"]["committed"], false);
    assert!(outside.message.contains("writes[1]"), "{}", outside.message);
}

#[test]
fn batch_results_list_only_the_writes_that_did_not_apply() {
    let (mut app, targets) = crate::component_tests::fixture();
    let a = &targets[0];
    let mut session = Session::default();
    session.allow_modify = true;
    let color = json!({"target": a, "key": "series.line.color", "value": "#0072b2"});
    for writes in [
        json!([color]),
        json!([color, {"target": a, "key": "series.label", "value": "S"}]),
    ] {
        let p = plan(
            &mut session,
            &mut app,
            json!({"parameters": {"writes": writes}}),
        )
        .unwrap();
        let result = session
            .call(
                &mut app,
                "plotium_execute",
                json!({"plan_id": p["plan_id"]}),
            )
            .unwrap();
        if writes.as_array().unwrap().len() == 1 {
            continue;
        }
        assert_eq!(result["summary"]["succeeded"], 1, "{result}");
        assert_eq!(result["summary"]["by_reason"]["already_at_value"], 1);
        assert_eq!(result["targets"].as_array().unwrap().len(), 1);
        assert_eq!(
            result["value"]["writes"],
            json!([{"index": 0, "key": "series.line.color", "outcome": "skipped", "skip_reason": "already_at_value"}])
        );
    }
}
