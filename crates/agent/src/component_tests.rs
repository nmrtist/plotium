use crate::session::Session;
use plotium_core::{
    automation::*,
    properties::{self, PropertyAddress},
    state::*,
};
use serde_json::{Value, json};

pub(super) fn fixture() -> (PlotiumApp, Vec<TargetRef>) {
    let (mut app, _) = crate::display_tests::fixture(false);
    let plot = app.doc.canvases[0].objects[0].plot_mut().unwrap();
    let mut second = plot.binding.series[0].clone();
    second.id = plot.allocate_series_id();
    plot.binding.series.push(second);
    let mut page = app.doc.canvases[0].clone();
    page.resource_id = CanvasId::new();
    app.doc.canvases.push(page);
    let targets = app
        .doc
        .canvases
        .iter()
        .enumerate()
        .flat_map(|(index, page)| app.series_targets(index, page.objects[0].id))
        .collect();
    (app, targets)
}
pub(super) fn values(
    app: &PlotiumApp,
    targets: &[TargetRef],
) -> Vec<properties::AggregateValue<properties::PropertyValue>> {
    targets
        .iter()
        .map(|target| {
            app.resolve_property(&PropertyAddress::new(
                target.clone(),
                properties::line::STROKE_WIDTH,
            ))
            .unwrap()
            .value
        })
        .collect()
}
fn call(
    session: &mut Session,
    app: &mut PlotiumApp,
    name: &str,
    mut args: Value,
) -> Result<Value, crate::Failure> {
    args["expected_revision"] = json!(app.doc.automation_revision);
    args["expected_project_context"] = json!(app.session.project_context);
    session.call(app, name, args)
}
fn edit(session: &mut Session, app: &mut PlotiumApp, targets: Value, reset: bool) -> Value {
    let parameters = if reset {
        json!({"key":"series.line.stroke_width"})
    } else {
        json!({"key":"series.line.stroke_width","value":2.5})
    };
    let plan = call(
        session,
        app,
        "plotium_plan",
        json!({"targets":targets,
        "tool_id":if reset {"properties.reset"} else {"properties.set"}, "parameters":parameters}),
    )
    .unwrap();
    session
        .call(app, "plotium_execute", json!({"plan_id":plan["plan_id"]}))
        .unwrap()
}
#[test]
fn exact_edits_deduplicate_preserve_owners_and_replay_after_page_reorder() {
    let (mut app, targets) = fixture();
    assert_eq!(targets[0].component, targets[2].component);
    let before = values(&app, &targets);
    let mut session = Session::default();
    session.allow_modify = true;
    let inspect = call(
        &mut session,
        &mut app,
        "plotium_properties_inspect",
        json!({
        "ids":[targets[0].resource.id],"key":"series.line.stroke_width"}),
    )
    .unwrap();
    let target = inspect["value"]["readings"][0]["target"].clone();
    assert_eq!(target, json!(targets[0]));
    let result = edit(&mut session, &mut app, json!([target, target]), false);
    assert_eq!(result["targets"].as_array().unwrap().len(), 1);
    let after = values(&app, &targets);
    assert_ne!(before[0], after[0]);
    assert_eq!(before[1..], after[1..]);
    let revision = app.doc.automation_revision;
    let history = app.session.undo_stack.len();
    let result = edit(&mut session, &mut app, json!([target]), false);
    assert_eq!(result["targets"][0]["skip_reason"], "already_at_value");
    assert_eq!(app.doc.automation_revision, revision);
    assert_eq!(app.session.undo_stack.len(), history);
    let frozen = &app.doc.automation_runs.last().unwrap().nodes[0].frozen_targets;
    assert_eq!(frozen.component_targets, vec![targets[0].clone()]);
    assert_eq!(frozen.total_matches, 1);
    assert_eq!(
        serde_json::from_value::<FrozenTargetSet>(json!(frozen)).unwrap(),
        *frozen
    );
    app.doc.canvases.swap(0, 1);
    app.undo();
    assert_eq!(values(&app, &targets), before);
    app.redo();
    assert_eq!(values(&app, &targets), after);
    edit(&mut session, &mut app, json!([target]), true);
    assert_eq!(values(&app, &targets), before);
    app.undo();
    assert_eq!(values(&app, &targets), after);
    let history = app.session.undo_stack.len();
    edit(
        &mut session,
        &mut app,
        json!([targets[1], targets[2]]),
        false,
    );
    assert_eq!(app.session.undo_stack.len(), history + 1);
    app.undo();
    assert_eq!(values(&app, &targets), after);
}
#[test]
fn malformed_addresses_and_unsupported_operations_are_rejected_atomically() {
    let (mut app, targets) = fixture();
    let before = values(&app, &targets);
    let mut session = Session::default();
    let target = json!(targets[0]);
    let mut missing = target.clone();
    missing["component"]["id"] = json!(99999);
    let mut forged = target.clone();
    forged["resource"]["parent_id"] = json!(targets[2].resource.parent_id);
    let mut kind = target.clone();
    kind["resource"]["kind"] = json!(KIND_DATASET);
    let mut local = target.clone();
    local["resource"]["local_id"] = json!("99999");
    for addressing in [
        json!({}),
        json!({"ids":[]}),
        json!({"targets":[]}),
        json!({"ids":[targets[0].resource.id],"targets":[target]}),
        json!({"ids":null,"targets":[target]}),
        json!({"targets":[{"resource":target["resource"]}]}),
        json!({"targets":[target,missing]}),
        json!({"targets":[forged]}),
        json!({"targets":[kind]}),
        json!({"targets":[local]}),
        json!({"targets":[{"resource":target["resource"],"component":{"kind":"processing_step","id":0}}]}),
    ] {
        let mut plan_args = addressing.clone();
        plan_args["tool_id"] = json!("properties.set");
        plan_args["parameters"] = json!({"key":"series.line.stroke_width","value":2.5});
        assert_eq!(
            call(&mut session, &mut app, "plotium_plan", plan_args)
                .unwrap_err()
                .code,
            "invalid_parameters"
        );
        let mut args = addressing;
        args["key"] = json!("series.line.stroke_width");
        assert_eq!(
            call(&mut session, &mut app, "plotium_properties_inspect", args)
                .unwrap_err()
                .code,
            "invalid_parameters"
        );
    }
    let skipped = call(
        &mut session,
        &mut app,
        "plotium_properties_inspect",
        json!({"targets":[target],"key":"series.contour.count"}),
    )
    .unwrap();
    assert_eq!(skipped["targets"][0]["outcome"], "skipped");
    assert_eq!(
        call(
            &mut session,
            &mut app,
            "plotium_plan",
            json!({"targets":[target],"tool_id":"resource.rename","parameters":{"name":"x"}})
        )
        .unwrap_err()
        .code,
        "invalid_parameters"
    );
    assert_eq!(
        call(
            &mut session,
            &mut app,
            "plotium_properties_inspect",
            json!({"targets":[target],"key":"processing.phase.p0"})
        )
        .unwrap_err()
        .code,
        "unsupported_property"
    );
    app.doc.canvases[0].objects[0]
        .plot_mut()
        .unwrap()
        .binding
        .series
        .remove(0);
    assert_eq!(
        call(
            &mut session,
            &mut app,
            "plotium_properties_inspect",
            json!({"targets":[target],"key":"series.line.stroke_width"})
        )
        .unwrap_err()
        .code,
        "invalid_parameters"
    );
    assert_eq!(values(&app, &targets[1..]), before[1..]);
}

#[test]
fn exact_plans_recheck_permissions_revision_busy_and_single_use() {
    let (mut app, targets) = fixture();
    let before = values(&app, &targets);
    let mut session = Session::default();
    for failure in ["permission_denied", "stale_revision", "busy"] {
        session.allow_modify = true;
        let plan = call(
            &mut session,
            &mut app,
            "plotium_plan",
            json!({
            "targets":[targets[0]],"tool_id":"properties.set",
            "parameters":{"key":"series.line.stroke_width","value":2.5}}),
        )
        .unwrap();
        match failure {
            "permission_denied" => session.allow_modify = false,
            "stale_revision" => app.doc.automation_revision += 1,
            "busy" => app.session.ui.phase_edit_active = true,
            _ => unreachable!(),
        }
        assert_eq!(
            session
                .call(
                    &mut app,
                    "plotium_execute",
                    json!({"plan_id":plan["plan_id"]})
                )
                .unwrap_err()
                .code,
            failure
        );
        app.session.ui.phase_edit_active = false;
        assert_eq!(
            session
                .call(
                    &mut app,
                    "plotium_execute",
                    json!({"plan_id":plan["plan_id"]})
                )
                .unwrap_err()
                .code,
            "expired_plan"
        );
        assert_eq!(values(&app, &targets), before);
    }
}

#[test]
fn presentation_history_reports_missing_page_or_object() {
    for remove_page in [true, false] {
        let (mut app, targets) = fixture();
        let mut session = Session::default();
        session.allow_modify = true;
        edit(&mut session, &mut app, json!([targets[0]]), false);
        let action = app.session.undo_stack.last().unwrap().clone();
        if remove_page {
            app.doc.canvases.remove(0);
        } else {
            app.doc.canvases[0].objects.clear();
        }
        let revision = app.doc.automation_revision;
        let history = app.session.undo_stack.len();
        app.undo();
        assert_eq!(app.doc.automation_revision, revision);
        assert_eq!(app.session.undo_stack.len(), history);
        assert!(
            app.session.status.contains("action target is stale"),
            "{}",
            app.session.status
        );
        assert!(app.try_execute_action(action).is_err());
    }
}
