use crate::guide_fixture::*;
use plotium_core::{project::*, properties::*, state::*};
use serde_json::{Value, json};

#[test]
fn guide_edit_preserves_series_and_other_plot_with_one_native_undo_and_roundtrip() {
    let (mut a, mut s, ids) = fixture();
    let bindings: Vec<_> = a.doc.canvases[0]
        .objects
        .iter()
        .map(|o| o.plot().unwrap().binding.clone())
        .collect();
    let other = snapshot(&a)[1].clone();
    let mut expected = a.doc.canvases[0].objects[0]
        .plot()
        .unwrap()
        .axis_overrides
        .clone();
    let base = a.session.undo_stack.len();
    write(
        &mut a,
        &mut s,
        &ids[..1],
        guide::POSITION.as_str(),
        json!([0., 1.]),
    );
    expected.set_legend_position([0., 1.], None);
    assert_eq!(
        a.doc.canvases[0].objects[0].plot().unwrap().axis_overrides,
        expected
    );
    assert_eq!(a.session.undo_stack.len(), base + 1);
    for (o, b) in a.doc.canvases[0].objects.iter().zip(&bindings) {
        assert_eq!(&o.plot().unwrap().binding, b);
    }
    assert_eq!(snapshot(&a)[1], other);
    let r = inspect(&mut a, &mut s, &ids[0], guide::POSITION.as_str());
    assert_eq!(r["value"]["value"]["value"], json!([0., 1.]));
    assert_eq!(r["guide_geometry"]["entry_count"], 6);
    assert_eq!(r["guide_geometry"]["coordinate_space"], "object_local_pt");
    a.undo();
    assert!(
        a.doc.canvases[0].objects[0]
            .plot()
            .unwrap()
            .axis_overrides
            .legend_position
            .is_none()
    );
    a.redo();
    assert_eq!(
        a.doc.canvases[0].objects[0].plot().unwrap().axis_overrides,
        expected
    );
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("guides.plotium");
    save_project(&a, &path, false).unwrap();
    let mut reopened = load_project(&path).unwrap();
    while reopened.compute_busy() {
        reopened.poll_compute();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(
        reopened.doc.canvases[0].objects[0]
            .plot()
            .unwrap()
            .axis_overrides,
        expected
    );
    for (o, b) in reopened.doc.canvases[0].objects.iter().zip(&bindings) {
        assert_eq!(&o.plot().unwrap().binding, b);
    }
}

#[test]
fn guide_modes_reset_only_the_required_coupling_and_hidden_position_remains_editable() {
    let (mut a, mut s, ids) = fixture();
    write(
        &mut a,
        &mut s,
        &ids[..1],
        guide::TITLE.as_str(),
        json!("Samples"),
    );
    write(
        &mut a,
        &mut s,
        &ids[..1],
        guide::LAYOUT.as_str(),
        json!("horizontal"),
    );
    write(&mut a, &mut s, &ids[..1], guide::COLUMNS.as_str(), json!(3));
    write(
        &mut a,
        &mut s,
        &ids[..1],
        guide::VISIBILITY.as_str(),
        json!("hide"),
    );
    write(
        &mut a,
        &mut s,
        &ids[..1],
        guide::POSITION.as_str(),
        json!([1., 0.]),
    );
    let r = inspect(&mut a, &mut s, &ids[0], guide::POSITION.as_str());
    assert_eq!(r["guide_geometry"]["not_rendered_reason"], "hidden");
    assert!(r["guide_geometry"]["legend_rect"].is_null());
    for mode in ["inside", "outside_right", "outside_bottom", "auto"] {
        write(
            &mut a,
            &mut s,
            &ids[..1],
            guide::PLACEMENT.as_str(),
            json!(mode),
        );
        let o = &a.doc.canvases[0].objects[0].plot().unwrap().axis_overrides;
        assert!(o.legend_position.is_none());
        assert_eq!(o.guide_title.as_deref(), Some("Samples"));
        assert_eq!(
            o.guide_layout,
            serde_json::from_value(json!("horizontal")).unwrap()
        );
        assert_eq!(o.guide_columns, Some(3));
        write(
            &mut a,
            &mut s,
            &ids[..1],
            guide::PLACEMENT.as_str(),
            json!("custom"),
        );
        assert_eq!(
            a.doc.canvases[0].objects[0]
                .plot()
                .unwrap()
                .axis_overrides
                .legend_position,
            Some([1., 0.])
        );
    }
    write(
        &mut a,
        &mut s,
        &ids[..1],
        guide::POSITION.as_str(),
        Value::Null,
    );
    let o = &a.doc.canvases[0].objects[0].plot().unwrap().axis_overrides;
    assert!(o.legend_position.is_none() && o.guide_placement.is_none());
    assert_eq!(o.guide_title.as_deref(), Some("Samples"));
    assert_eq!(
        o.guide_visibility,
        serde_json::from_value(json!("hide")).unwrap()
    );
    // Multi-target writes are a single Undo, without binding edits.
    let before = a.session.undo_stack.len();
    write(
        &mut a,
        &mut s,
        &ids,
        guide::POSITION.as_str(),
        json!([0.2, 0.8]),
    );
    assert_eq!(a.session.undo_stack.len(), before + 1);
    a.undo();
    assert!(
        a.doc.canvases[0].objects.iter().all(|o| o
            .plot()
            .unwrap()
            .axis_overrides
            .legend_position
            .is_none())
    );
    a.redo();
    assert!(
        a.doc.canvases[0]
            .objects
            .iter()
            .all(|o| o.plot().unwrap().axis_overrides.legend_position == Some([0.2, 0.8]))
    );
}

#[test]
fn guide_invalid_inputs_targets_limits_context_revision_and_revocation_fail_without_partial_edits()
{
    let (mut a, mut s, ids) = fixture();
    let original = snapshot(&a);
    let revision = a.doc.automation_revision;
    for value in [
        json!([-0.1, 0.]),
        json!([0., 1.01]),
        json!([0.]),
        json!([0., 1., 0.]),
        json!(["NaN", 0.]),
        json!({"x":0,"y":1}),
    ] {
        let p = args(&a, &ids, guide::POSITION.as_str(), value);
        assert_eq!(
            s.call(&mut a, "plotium_plan", p).unwrap_err().code,
            "invalid_parameters"
        );
    }
    for targets in [
        vec![ids[0].clone(), "missing".into()],
        vec![ids[0].clone(), a.doc.canvases[0].resource_id.to_string()],
        vec![ids[0].clone(); 201],
    ] {
        let p = args(&a, &targets, guide::POSITION.as_str(), json!([0., 0.]));
        assert_eq!(
            s.call(&mut a, "plotium_plan", p).unwrap_err().code,
            "invalid_parameters"
        );
    }
    let p = args(&a, &ids, guide::TITLE.as_str(), json!("x".repeat(16385)));
    assert_eq!(
        s.call(&mut a, "plotium_plan", p).unwrap_err().code,
        "invalid_parameters"
    );
    let target = a.object_target(0, a.doc.canvases[0].objects[0].id).unwrap();
    for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.01, 1.01] {
        assert!(
            a.plan_property_write(
                guide::POSITION,
                std::slice::from_ref(&target),
                &PropertyValue::NormalizedPosition(Some([v, 0.]))
            )
            .is_err()
        );
    }
    assert_eq!(snapshot(&a), original);
    assert_eq!(a.doc.automation_revision, revision);
    let p = args(&a, &ids, guide::POSITION.as_str(), json!([0., 0.]));
    let plan = s.call(&mut a, "plotium_plan", p.clone()).unwrap();
    s.allow_modify = false;
    let e = json!({"plan_id":plan["plan_id"]});
    assert_eq!(
        s.call(&mut a, "plotium_execute", e.clone())
            .unwrap_err()
            .code,
        "permission_denied"
    );
    assert_eq!(
        s.call(&mut a, "plotium_execute", e).unwrap_err().code,
        "expired_plan"
    );
    s.allow_modify = true;
    let plan = s.call(&mut a, "plotium_plan", p.clone()).unwrap();
    write(
        &mut a,
        &mut s,
        &ids[..1],
        guide::TITLE.as_str(),
        json!("Revision change"),
    );
    assert_eq!(
        s.call(
            &mut a,
            "plotium_execute",
            json!({"plan_id":plan["plan_id"]})
        )
        .unwrap_err()
        .code,
        "stale_revision"
    );
    let p = args(&a, &ids, guide::POSITION.as_str(), json!([0., 0.]));
    let plan = s.call(&mut a, "plotium_plan", p.clone()).unwrap();
    let rev = a.doc.automation_revision;
    a.start_new_project();
    a.doc.automation_revision = rev;
    assert_eq!(
        s.call(&mut a, "plotium_plan", p).unwrap_err().code,
        "project_changed"
    );
    assert_eq!(
        s.call(
            &mut a,
            "plotium_execute",
            json!({"plan_id":plan["plan_id"]})
        )
        .unwrap_err()
        .code,
        "project_changed"
    );
    assert!(a.doc.canvases.is_empty());
}

#[test]
fn guide_inspection_reports_oversize_and_discovery_without_mutating_revision() {
    let (mut a, mut s, ids) = fixture();
    let catalog = crate::session::property_catalog();
    let p = catalog["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == guide::POSITION.as_str())
        .unwrap();
    assert_eq!(p["schema"]["type"], "normalized_position");
    assert_eq!(p["schema"]["input_schema"]["oneOf"][1]["maxItems"], 2);
    write(
        &mut a,
        &mut s,
        &ids[..1],
        guide::TITLE.as_str(),
        json!("Long ".repeat(60)),
    );
    write(
        &mut a,
        &mut s,
        &ids[..1],
        guide::POSITION.as_str(),
        json!([1., 1.]),
    );
    let rev = a.doc.automation_revision;
    let undo = a.session.undo_stack.len();
    let r = inspect(&mut a, &mut s, &ids[0], guide::POSITION.as_str());
    assert_eq!(r["guide_geometry"]["fits_plot"][0], false);
    assert_eq!(r["guide_geometry"]["available_travel_pt"][0], 0.);
    assert_eq!(r["guide_geometry"]["effective_position"][0], 0.);
    assert_eq!(r["value"]["value"]["value"], json!([1., 1.]));
    assert_eq!(a.doc.automation_revision, rev);
    assert_eq!(a.session.undo_stack.len(), undo);
}

#[test]
#[ignore = "explicit manual fixture output path required"]
fn write_guide_acceptance_fixture() {
    let (a, _, _) = fixture();
    let path = std::path::PathBuf::from(
        std::env::var_os("PLOTIUM_GUIDE_FIXTURE").expect("set fixture output path"),
    );
    assert!(path.is_absolute() && !path.exists());
    save_project(&a, &path, false).unwrap();
}

fn snapshot(a: &PlotiumApp) -> Vec<(DataBinding, AxisOverrides, ObjectFrame, bool, bool, String)> {
    a.doc.canvases[0]
        .objects
        .iter()
        .map(|o| {
            let p = o.plot().unwrap();
            (
                p.binding.clone(),
                p.axis_overrides.clone(),
                o.frame,
                o.visible,
                o.locked,
                o.name.clone(),
            )
        })
        .collect()
}

#[test]
fn guide_duplicates_reset_and_response_budget_are_bounded_without_changing_old_skips() {
    let (mut a, mut s, ids) = fixture();
    let before = a.session.undo_stack.len();
    write(
        &mut a,
        &mut s,
        &[ids[0].clone(), ids[0].clone()],
        guide::POSITION.as_str(),
        json!([0.5, 0.5]),
    );
    assert_eq!(a.session.undo_stack.len(), before + 1);
    for key in [guide::POSITION, guide::PLACEMENT] {
        let mut p = args(&a, &ids[..1], key.as_str(), Value::Null);
        p["tool_id"] = json!("properties.reset");
        p["parameters"] = json!({"key":key.as_str()});
        let plan = s.call(&mut a, "plotium_plan", p).unwrap();
        s.call(
            &mut a,
            "plotium_execute",
            json!({"plan_id":plan["plan_id"]}),
        )
        .unwrap();
        let o = &a.doc.canvases[0].objects[0].plot().unwrap().axis_overrides;
        assert!(o.legend_position.is_none() && o.guide_placement.is_none());
        write(
            &mut a,
            &mut s,
            &ids[..1],
            guide::POSITION.as_str(),
            json!([0.5, 0.5]),
        );
    }
    // Older property calls still skip existing inapplicable objects.
    let canvas = a.doc.canvases[0].resource_id.to_string();
    let p = args(
        &a,
        &[ids[0].clone(), canvas],
        plot_display::LEGEND_PT.as_str(),
        json!(8),
    );
    let plan = s.call(&mut a, "plotium_plan", p).unwrap();
    assert!(
        plan["preflight"]["targets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["status"] == "skipped")
    );
    // Native GUI titles have no new MCP input prerequisite; reading remains bounded.
    let id = a.doc.canvases[0].objects[0].id;
    let mut o = a.doc.canvases[0].objects[0]
        .plot()
        .unwrap()
        .axis_overrides
        .clone();
    o.guide_title = Some("x".repeat(150_000));
    a.set_axis_overrides_value(0, id, &o);
    let rev = a.doc.automation_revision;
    let undo = a.session.undo_stack.len();
    let p = json!({"expected_project_context":a.session.project_context,"expected_revision":rev,"ids":[ids[0]],"key":guide::TITLE.as_str()});
    assert_eq!(
        s.call(&mut a, "plotium_properties_inspect", p)
            .unwrap_err()
            .code,
        "response_too_large"
    );
    assert_eq!(a.doc.automation_revision, rev);
    assert_eq!(a.session.undo_stack.len(), undo);
}

#[test]
fn retained_origin_without_legend_is_readable_and_clearable_but_not_free_colorbar_position() {
    let (mut a, id) = crate::display_tests::fixture(true);
    let mut s = crate::Session::default();
    s.allow_modify = true;
    assert_eq!(
        plotium_core::automation::property_definition_description(guide::POSITION).unwrap()["scope"],
        "Object"
    );
    write(
        &mut a,
        &mut s,
        std::slice::from_ref(&id),
        guide::PLACEMENT.as_str(),
        json!("custom"),
    );
    let p = args(
        &a,
        std::slice::from_ref(&id),
        guide::POSITION.as_str(),
        json!([0.5, 0.5]),
    );
    assert_eq!(
        s.call(&mut a, "plotium_plan", p).unwrap_err().code,
        "invalid_parameters"
    );
    let reading = inspect(&mut a, &mut s, &id, guide::POSITION.as_str());
    assert_eq!(reading["value"]["value"]["value"], json!([1., 0.]));
    write(&mut a, &mut s, &[id], guide::POSITION.as_str(), Value::Null);
    assert!(
        a.doc.canvases[0].objects[0]
            .plot()
            .unwrap()
            .axis_overrides
            .legend_position
            .is_none()
    );
}
