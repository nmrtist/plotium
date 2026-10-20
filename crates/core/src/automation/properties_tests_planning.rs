//! Property-tool plan refinement tests.

use super::*;

/// A syntactically valid relation plan, for the tools whose parameters carry
/// one. It never executes here — planning only has to decode it — so the ids it
/// names need not exist.
fn relation_plan() -> serde_json::Value {
    let plan = plotium_data::RelPlanV1::new(plotium_data::Relation::SnapshotRead(
        plotium_data::SnapshotRead {
            table: plotium_data::TableId::new(),
            revision: plotium_data::RevisionId::new(),
            fingerprint: plotium_data::ContentHash::of(b"pre-existing-tool planning"),
        },
    ));
    serde_json::to_value(plan).expect("a relation plan serializes")
}

/// The per-tool planning seam is additive. Every tool that existed before it
/// with resource selectors must still use the shared gate alone: one planned target per frozen
/// resource, no component, and the shared reasons.
#[test]
fn the_planning_of_pre_existing_tools_is_unchanged() {
    let (app, _) = contour_app();
    let canvas = app.doc.canvases[0].resource_id.to_string();
    let dataset = app.doc.datasets[0].resource_id().to_string();
    let object = plot_resource_id(&app);
    let transform = serde_json::json!({
        "plan": relation_plan(),
        "name": "Projected",
        "memory_limit_bytes": 16 * 1024 * 1024,
    });
    let scheme_path = std::env::temp_dir().join(format!(
        "plotium-planning-{}.plotiumproc",
        uuid::Uuid::new_v4()
    ));
    crate::project::save_scheme(&scheme_path, &app.doc.datasets[0]).unwrap();
    let cases: &[(&str, serde_json::Value, &str)] = &[
        ("project.get_blueprint", serde_json::json!({}), &canvas),
        (
            "resources.search",
            serde_json::json!({"query": {}}),
            &canvas,
        ),
        ("resources.inspect", serde_json::json!({}), &object),
        ("data.preview", serde_json::json!({}), &dataset),
        ("render.preview", serde_json::json!({}), &canvas),
        ("results.compare", serde_json::json!({}), &canvas),
        ("resource.rename", serde_json::json!({"name": "x"}), &object),
        (
            "figure.apply_theme",
            serde_json::json!({"theme_id": "default"}),
            &canvas,
        ),
        (
            "processing.apply_scheme",
            serde_json::json!({"path": scheme_path}),
            &dataset,
        ),
        ("data.transform", transform, &dataset),
        (
            "figure.export",
            serde_json::json!({"directory": ".", "format": "svg"}),
            &canvas,
        ),
    ];
    assert_eq!(
        cases.len(),
        ToolRegistry::built_in()
            .descriptors()
            .filter(|d| !is_native_curve_tool(&d.id)
                && !is_scientific_plot_tool(&d.id)
                && !matches!(
                    d.id.as_str(),
                    "data.import"
                        | FIGURE_EDIT
                        | TABLE_PATCH
                        | TABLE_REFRESH
                        | FIGURE_DIAGNOSE
                        | DIFFUSION_INSPECT
                        | DIFFUSION_SET_INPUT
                        | PROCESSING_INSPECT
                        | PROCESSING_EDIT
                        | REGIONS_EXTRACT
                        | DIFFUSION_FIT
                        | DOSY_BUILD
                        | ANALYSIS_RESULTS_INSPECT
                        | TOOL_SET_MANY
                ))
            .count()
            - 7,
        "every resource-targeted tool that predates property, document discovery, image preview, table read and explicit curve binding tools is covered here"
    );
    for (tool_id, parameters, target) in cases {
        let plan = plan_tool(
            &app,
            request(
                &app,
                tool_id,
                parameters.clone(),
                vec![(*target).to_owned()],
                CallerType::Agent,
            ),
        )
        .unwrap_or_else(|error| panic!("{tool_id} plans: {error}"));
        assert_eq!(plan.targets.len(), 1, "{tool_id} expands nothing");
        assert!(
            plan.targets[0].target.component.is_none(),
            "{tool_id} names no component"
        );
        assert_eq!(plan.targets[0].target.resource.id, **target);
        assert!(
            plan.targets[0]
                .reason
                .contains("declared kind and capabilities")
                || plan.targets[0]
                    .reason
                    .contains("lacks a required kind or capability"),
            "{tool_id} keeps the shared reason: {}",
            plan.targets[0].reason
        );
    }
    std::fs::remove_file(scheme_path).unwrap();
}
