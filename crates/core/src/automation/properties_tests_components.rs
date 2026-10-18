use super::*;

#[test]
fn components_validate_identity_before_applicability_and_reject_other_tools() {
    let (mut app, target) = contour_app();
    let mut req = set_request(&app, contour::COUNT.as_str(), serde_json::json!(12));
    req.targets = TargetSelector::Components {
        targets: vec![target.clone(), target.clone()],
    };
    let plan = plan_tool(&app, req.clone()).unwrap();
    assert_eq!(plan.targets.len(), 1);
    assert_eq!(plan.frozen_targets.total_matches, 1);
    assert_eq!(plan.frozen_targets.component_targets, vec![target.clone()]);
    let mut invalid = target.clone();
    invalid.resource.kind = ResourceKindId::new(KIND_DATASET);
    req.targets = TargetSelector::Components {
        targets: vec![target.clone(), invalid],
    };
    assert!(matches!(
        plan_tool(&app, req.clone()),
        Err(AutomationError::InvalidSelector(_))
    ));
    req.targets = TargetSelector::Components { targets: vec![] };
    assert!(matches!(
        plan_tool(&app, req.clone()),
        Err(AutomationError::InvalidSelector(_))
    ));
    req.targets = TargetSelector::Components {
        targets: vec![TargetRef::resource(target.resource.clone())],
    };
    assert!(matches!(
        plan_tool(&app, req.clone()),
        Err(AutomationError::InvalidSelector(_))
    ));
    req.targets = TargetSelector::Components {
        targets: vec![target.clone()],
    };
    for tool in [
        "resources.inspect",
        "resource.rename",
        "project.get_blueprint",
    ] {
        let mut other = req.clone();
        other.tool_id = tool.into();
        other.parameters = if tool == "resource.rename" {
            serde_json::json!({"name":"x"})
        } else {
            serde_json::json!({})
        };
        assert!(matches!(
            plan_tool(&app, other),
            Err(AutomationError::InvalidSelector(_))
        ));
    }
    app.doc.canvases[0].objects.clear();
    assert!(matches!(
        plan_tool(&app, req),
        Err(AutomationError::InvalidSelector(_))
    ));
}

#[test]
fn core_components_support_processing_steps_without_expanding_siblings() {
    let mut app = PlotiumApp::new();
    app.doc.datasets.push(Dataset::Nmr(Box::new(
        NmrDataset::load(NmrData {
            points: vec![num_complex::Complex64::new(1.0, 0.0); 32],
            domain: Domain::Time,
            spectral_width_hz: 4000.0,
            observe_freq_mhz: 400.0,
            carrier_ppm: 0.0,
            nucleus: "1H".into(),
            source: "component fixture".into(),
            group_delay: 0.0,
        })
        .unwrap(),
    )));
    let resource = ResourceRef::from(app.doc.datasets[0].resource_id());
    let targets = app.resource_processing_step_targets(&resource);
    assert!(!targets.is_empty());
    let mut req = request(
        &app,
        TOOL_INSPECT,
        serde_json::json!({"key":"dataset.processing.step.enabled"}),
        vec![],
        CallerType::Agent,
    );
    req.targets = TargetSelector::Components {
        targets: vec![targets[0].clone()],
    };
    let plan = plan_tool(&app, req).unwrap();
    assert_eq!(plan.targets.len(), 1);
    assert_eq!(plan.targets[0].target, targets[0]);
    execute_tool(&mut app, plan, ExecutionAuthority::Read).unwrap();
}

#[test]
fn workflow_static_components_preserve_full_identity_in_audit() {
    let (mut app, target) = contour_app();
    let workflow: WorkflowDefinition = serde_json::from_value(serde_json::json!({
        "schema":WORKFLOW_SCHEMA, "nodes":[{
            "id":"exact", "tool_id":TOOL_SET,
            "parameters":{"key":contour::COUNT.as_str(), "value":12},
            "targets":{"kind":"components", "targets":[target]}
        }]
    }))
    .unwrap();
    let manifest = execute_workflow(
        &mut app,
        &workflow,
        CallerType::Workflow,
        ExecutionAuthority::ReversibleModify,
        &TaskCancellation::default(),
        &mut |_| {},
    )
    .unwrap();
    assert!(manifest.errors.is_empty(), "{:?}", manifest.errors);
    assert_eq!(
        manifest.nodes[0].frozen_targets.component_targets,
        vec![target]
    );
    app.undo();
    app.redo();
}
