use super::*;
use serde_json::json;

fn saved_state(app: &PlotiumApp) -> (bool, u64, u64, usize, usize, usize) {
    (
        app.doc.dirty,
        app.doc.edit_generation,
        app.doc.automation_revision,
        app.doc.automation_runs.len(),
        app.session.undo_stack.len(),
        app.session.redo_stack.len(),
    )
}

#[test]
fn repeated_queries_on_a_saved_project_preserve_persistent_state() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("queries.plotium");
    crate::project::save_project(&app_with_table_and_canvas(), &path, false).unwrap();
    let mut app = crate::project::load_project(&path).unwrap();
    assert!(!app.doc.dirty);
    let before = saved_state(&app);
    let context = app.session.project_context;
    let dataset = app.doc.datasets[0].resource_id().to_string();
    let table = app.doc.datasets[0].as_table().unwrap();
    let address =
        json!({"dataset_id":dataset,"table_revision":table.typed_state.envelope.revision.id});
    let canvas = app.doc.canvases[0].resource_id.to_string();
    let target = TargetRef::resource(
        ProjectResourceProvider::new(&app)
            .inspect(&canvas)
            .unwrap()
            .resource,
    );
    let queries = vec![
        ("project.get_blueprint", json!({}), vec![]),
        ("resources.search", json!({"query":{}}), vec![]),
        ("resources.inspect", json!({}), vec![dataset.clone()]),
        ("data.preview", json!({}), vec![dataset.clone()]),
        ("results.compare", json!({}), vec![dataset.clone()]),
        (
            "properties.inspect",
            json!({"key":"document.figure.typography.tick_pt"}),
            vec!["document".into()],
        ),
        (
            DOCUMENT_INSPECT,
            json!({"target":document_root_target(),"view":"children"}),
            vec!["document".into()],
        ),
        (TABLE_COLUMNS, address.clone(), vec![dataset.clone()]),
        (TABLE_READ_ROWS, address, vec![dataset]),
        ("render.preview", json!({}), vec![canvas.clone()]),
        (
            IMAGE_PREVIEW,
            json!({"target":target,"max_edge":64}),
            vec![canvas.clone()],
        ),
        (
            FIGURE_DIAGNOSE,
            json!({"canvas":target,"output":{"size_mm":[120,90]}}),
            vec![canvas],
        ),
    ];
    for _ in 0..3 {
        for (tool, parameters, ids) in &queries {
            let plan = plan_tool(
                &app,
                request(
                    tool,
                    parameters.clone(),
                    ids.clone(),
                    app.doc.automation_revision,
                ),
            )
            .unwrap();
            execute_tool(&mut app, plan, ExecutionAuthority::Read).unwrap();
            assert_eq!(saved_state(&app), before, "{tool}");
            assert_eq!(app.session.project_context, context);
        }
    }
    crate::project::save_project(&app, &path, false).unwrap();
    let restored = crate::project::load_project(&path).unwrap();
    assert!(restored.doc.automation_runs.is_empty());
    assert!(!restored.doc.dirty);
}

#[test]
fn mixed_workflow_retains_query_evidence_after_save_and_reload() {
    let (mut app, _) = crate::properties::tests::contour_app();
    let canvas = &app.doc.canvases[0];
    let id = format!("{}/{}", canvas.resource_id, canvas.objects[0].id);
    let workflow: WorkflowDefinition = serde_json::from_value(json!({
        "schema":WORKFLOW_SCHEMA,
        "inputs":{"source":{"kind":"resources","ids":[id]}},
        "nodes":[
            {"id":"inspect","tool_id":TOOL_INSPECT,"parameters":{"key":"series.contour.count"},
             "targets":{"kind":"workflow_input","name":"source"}},
            {"id":"edit","tool_id":TOOL_SET,"parameters":{"key":"series.contour.count","value":7},
             "targets":{"kind":"workflow_input","name":"source"},"dependencies":["inspect"],
             "condition":{"kind":"if_succeeded","node":"inspect"}}
        ]
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
    assert_eq!(manifest.nodes.len(), 2);
    assert_eq!(manifest.nodes[0].tool_id, TOOL_INSPECT);
    assert!(
        !manifest.nodes[0].result.value["readings"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(manifest.nodes[1].parameters["value"], 7);
    assert!(manifest.end_revision > manifest.start_revision);
    assert_eq!(app.doc.automation_runs.len(), 1);
    assert_eq!(app.session.undo_stack.len(), 1);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workflow.plotium");
    crate::project::save_project(&app, &path, false).unwrap();
    let restored = crate::project::load_project(&path).unwrap();
    assert_eq!(
        serde_json::to_value(&restored.doc.automation_runs).unwrap(),
        json!([manifest])
    );
}

#[test]
fn scientific_inspection_does_not_append_runs() {
    let (mut app, _) = crate::properties::tests::contour_app();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("science.plotium");
    crate::project::save_project(&app, &path, false).unwrap();
    app = crate::project::load_project(&path).unwrap();
    let before = saved_state(&app);
    for tool in [
        PROCESSING_INSPECT,
        DIFFUSION_INSPECT,
        ANALYSIS_RESULTS_INSPECT,
    ] {
        let plan = plan_tool(
            &app,
            request(
                tool,
                json!({}),
                vec![app.doc.datasets[0].resource_id().to_string()],
                app.doc.automation_revision,
            ),
        )
        .unwrap();
        let outcome = execute_tool(&mut app, plan, ExecutionAuthority::Read);
        if tool == PROCESSING_INSPECT {
            outcome.unwrap();
        } else {
            assert!(
                outcome.is_err(),
                "the contour fixture has no diffusion capability"
            );
        }
        assert_eq!(saved_state(&app), before, "{tool}");
    }
}
