//! Scientific API regression tests over the existing synthetic diffusion fixture.
use super::diffusion_input_tests::direct_input_stack;
use super::pseudo_tests::wait_for_compute;
use super::*;
use crate::automation::*;
use serde_json::json;

fn app() -> PlotiumApp {
    let (mut data, input) = direct_input_stack();
    data.diffusion_input = Some(input);
    let mut app = PlotiumApp::new_with_settings(Default::default());
    app.doc.datasets.push(Dataset::Nmr2D(Box::new(data)));
    app.session.project_present = true;
    app
}
fn request(app: &PlotiumApp, id: &str, parameters: serde_json::Value, field: bool) -> ToolRequest {
    let owner = app.doc.datasets[0].resource_id().to_string();
    let target = if field {
        ProjectResourceProvider::new(app)
            .descriptors()
            .into_iter()
            .find(|d| {
                d.resource.parent_id.as_deref() == Some(&owner)
                    && d.capabilities.iter().any(|c| c.0 == CAP_DIFFUSION_INPUT)
            })
            .unwrap()
            .resource
            .id
    } else {
        owner
    };
    ToolRequest {
        tool_id: id.into(),
        tool_version: 1,
        parameters,
        targets: TargetSelector::Explicit { ids: vec![target] },
        expected_revision: DocumentRevision(app.doc.automation_revision),
        caller: CallerType::Human,
    }
}
#[test]
fn scientific_api_inspects_dataset_and_field_and_never_falls_back_from_explicit_input() {
    let mut app = app();
    let req = request(&app, DIFFUSION_INSPECT, json!({}), true);
    let plan = plan_tool(&app, req).unwrap();
    let result = execute_tool(&mut app, plan, ExecutionAuthority::Read).unwrap();
    assert_eq!(result.value["resolution_source"], "stored_default");
    assert_eq!(result.value["executable"], true);
    let mut input = app.doc.datasets[0]
        .as_nmr2d()
        .unwrap()
        .diffusion_input
        .clone()
        .unwrap();
    input.confirmed = false;
    let req = request(&app, DIFFUSION_INSPECT, json!({"input":input}), false);
    let plan = plan_tool(&app, req).unwrap();
    let result = execute_tool(&mut app, plan, ExecutionAuthority::Read).unwrap();
    assert_eq!(result.value["resolution_source"], "explicit_input");
    assert_eq!(result.value["input"]["status"], "evidence_required");
    assert_eq!(result.value["executable"], false);
}
#[test]
fn scientific_api_background_success_is_undoable_and_replay_only_observes_receipt() {
    let mut app = app();
    let before = app.doc.automation_revision;
    let plan = plan_tool(
        &app,
        request(&app, DOSY_BUILD, json!({"method":"mono_exp"}), true),
    )
    .unwrap();
    let id = plan.task_id.clone().unwrap();
    execute_tool(&mut app, plan.clone(), ExecutionAuthority::ReversibleModify).unwrap();
    assert!(app.session.scientific_tasks.get(&id).is_some());
    assert!(app.doc.datasets[0].as_nmr2d().unwrap().dosy_map.is_none());
    wait_for_compute(&mut app);
    let receipt = app.session.scientific_tasks.get(&id).unwrap();
    assert_eq!(receipt.status, TaskStatus::Succeeded);
    assert_eq!(receipt.end_revision, Some(DocumentRevision(before + 1)));
    let original = app.doc.datasets[0]
        .as_nmr2d()
        .unwrap()
        .dosy_map
        .as_ref()
        .unwrap()
        .d
        .clone();
    assert_eq!(app.session.undo_stack.len(), 1);
    app.undo();
    assert!(app.doc.datasets[0].as_nmr2d().unwrap().dosy_map.is_none());
    app.redo();
    let restored = &app.doc.datasets[0]
        .as_nmr2d()
        .unwrap()
        .dosy_map
        .as_ref()
        .unwrap()
        .d;
    assert!(
        original
            .iter()
            .zip(restored)
            .all(|(a, b)| a.to_bits() == b.to_bits())
    );
    let revision = app.doc.automation_revision;
    execute_tool(&mut app, plan, ExecutionAuthority::Read).unwrap();
    assert_eq!(app.doc.automation_revision, revision);
    assert!(!app.compute_busy());
}
#[test]
fn scientific_api_cancel_wins_against_a_queued_success() {
    let mut app = app();
    let source = app.doc.datasets[0].resource_id();
    let id = TaskId(uuid::Uuid::new_v4().to_string());
    app.start_dosy_task(source, id.clone(), Some("lost-response".into()), None, None)
        .unwrap();
    let first = app.cancel_scientific_task(&id).unwrap();
    let again = app.cancel_scientific_task(&id).unwrap();
    assert_eq!(first, again);
    wait_for_compute(&mut app);
    assert_eq!(
        app.session.scientific_tasks.get(&id).unwrap().status,
        TaskStatus::Cancelled
    );
    assert!(app.doc.datasets[0].as_nmr2d().unwrap().dosy_map.is_none());
    assert!(app.session.undo_stack.is_empty());
}
#[test]
fn scientific_api_document_edit_and_permission_revocation_reject_completed_results() {
    for revoke in [false, true] {
        let mut app = app();
        let source = app.doc.datasets[0].resource_id();
        let id = TaskId(uuid::Uuid::new_v4().to_string());
        app.start_dosy_task(source, id.clone(), None, None, None)
            .unwrap();
        if revoke {
            let permission = TaskCancellation::default();
            app.session
                .scientific_tasks
                .bind_permission(&id, permission.clone());
            permission.cancel();
        } else {
            app.doc.automation_revision += 1;
        }
        wait_for_compute(&mut app);
        let receipt = app.session.scientific_tasks.get(&id).unwrap();
        assert_eq!(
            receipt.failure_code.as_deref(),
            Some(if revoke {
                "permission_denied"
            } else {
                "stale_revision"
            })
        );
        assert!(app.doc.datasets[0].as_nmr2d().unwrap().dosy_map.is_none());
    }
}
#[test]
fn scientific_api_project_replacement_preserves_a_stale_receipt() {
    let mut app = app();
    let source = app.doc.datasets[0].resource_id();
    let id = TaskId(uuid::Uuid::new_v4().to_string());
    app.start_dosy_task(source, id.clone(), None, None, None)
        .unwrap();
    let context = app.session.project_context;
    app.start_new_project();
    let receipt = app.session.scientific_tasks.get(&id).unwrap();
    assert_eq!(receipt.project_context, context);
    assert_eq!(receipt.status, TaskStatus::Stale);
    assert!(app.doc.datasets.is_empty());
}
#[test]
fn scientific_api_pipeline_ids_are_frozen_and_invalid_edits_do_not_mutate_document() {
    let mut app = app();
    let before = crate::actions::DatasetProcessingState::from_dataset(&app.doc.datasets[0]);
    let plan = plan_tool(
        &app,
        request(
            &app,
            PROCESSING_EDIT,
            json!({"edits":[{"op":"add","axis":"f2","kind":"Invert"}]}),
            false,
        ),
    )
    .unwrap();
    let id = plan.processing_edit.as_ref().unwrap()["axes"][0]["pipeline"]["steps"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()["id"]
        .as_u64()
        .unwrap();
    assert_eq!(
        crate::actions::DatasetProcessingState::from_dataset(&app.doc.datasets[0]),
        before
    );
    execute_tool(&mut app, plan, ExecutionAuthority::ReversibleModify).unwrap();
    wait_for_compute(&mut app);
    app.undo();
    wait_for_compute(&mut app);
    assert_eq!(
        crate::actions::DatasetProcessingState::from_dataset(&app.doc.datasets[0]),
        before
    );
    app.redo();
    wait_for_compute(&mut app);
    assert_eq!(
        app.doc.datasets[0]
            .as_nmr2d()
            .unwrap()
            .params
            .f2
            .steps
            .last()
            .unwrap()
            .id
            .get(),
        id
    );
    let revision = app.doc.automation_revision;
    assert!(
        plan_tool(
            &app,
            request(
                &app,
                PROCESSING_EDIT,
                json!({"edits":[{"op":"remove","step":9999999}]}),
                false
            )
        )
        .is_err()
    );
    assert_eq!(app.doc.automation_revision, revision);
}
#[test]
fn scientific_api_scheme_execution_uses_preflight_bytes_after_source_removal() {
    let mut app = app();
    let path = std::env::temp_dir().join(format!(
        "plotium-frozen-{}.plotiumproc",
        uuid::Uuid::new_v4()
    ));
    crate::project::save_scheme(&path, &app.doc.datasets[0]).unwrap();
    let plan = plan_tool(
        &app,
        request(&app, "processing.apply_scheme", json!({"path":path}), false),
    )
    .unwrap();
    let sha = plan.frozen_scheme.as_ref().unwrap().sha256.clone();
    std::fs::remove_file(path).unwrap();
    let result = execute_tool(&mut app, plan, ExecutionAuthority::ReversibleModify).unwrap();
    assert_eq!(result.value["preparation"]["scheme_sha256"], sha);
    wait_for_compute(&mut app);
}

#[test]
fn scientific_api_regions_fit_and_native_result_inspection_share_stable_undo() {
    let mut app = app();
    let plan=plan_tool(&app,request(&app,REGIONS_EXTRACT,json!({"regions":[{"source":"explicit","bounds":[0.8,1.2],"metric":"Height","name":"Signal"}]}),true)).unwrap();
    let id = plan.task_id.clone().unwrap();
    execute_tool(&mut app, plan, ExecutionAuthority::ReversibleModify).unwrap();
    assert_eq!(app.doc.datasets.len(), 1);
    wait_for_compute(&mut app);
    let receipt = app.session.scientific_tasks.get(&id).unwrap();
    assert_eq!(
        receipt.status,
        TaskStatus::Succeeded,
        "{:?}",
        receipt.message
    );
    assert_eq!(app.doc.canvases.len(), 0);
    let table = app.doc.datasets[1].as_table().unwrap();
    let evidence = table.meta.region_extraction.as_ref().unwrap();
    assert!(evidence.source_version.is_some());
    assert_eq!(evidence.input_fingerprint.as_ref().unwrap().len(), 64);
    let table_id = table.resource_id;
    let response = table.series_bindings[0].value_column;
    let req = ToolRequest {
        tool_id: DIFFUSION_FIT.into(),
        tool_version: 1,
        parameters: json!({"response_column":response}),
        targets: TargetSelector::Explicit {
            ids: vec![table_id.to_string()],
        },
        expected_revision: DocumentRevision(app.doc.automation_revision),
        caller: CallerType::Human,
    };
    let plan = plan_tool(&app, req).unwrap();
    let fit_id = plan.task_id.clone().unwrap();
    execute_tool(&mut app, plan.clone(), ExecutionAuthority::ReversibleModify).unwrap();
    wait_for_compute(&mut app);
    let receipt = app.session.scientific_tasks.get(&fit_id).unwrap();
    assert_eq!(
        receipt.status,
        TaskStatus::Succeeded,
        "{:?}",
        receipt.message
    );
    let analysis_resource = receipt.produced[0].clone();
    assert_eq!(analysis_resource.kind.0, KIND_ANALYSIS);
    let table = app.doc.datasets[1].as_table().unwrap();
    let stored = table.curve_fit_analyses.clone();
    let d = stored[0]
        .result
        .parameters
        .iter()
        .find(|p| p.parameter == "D")
        .unwrap()
        .value;
    assert!((d - 1.2e-9).abs() / 1.2e-9 < 0.03, "{d}");
    app.undo();
    assert!(
        app.doc.datasets[1]
            .as_table()
            .unwrap()
            .curve_fit_analyses
            .is_empty()
    );
    app.redo();
    assert!(app.doc.datasets[1].as_table().unwrap().curve_fit_analyses == stored);
    let req = ToolRequest {
        tool_id: ANALYSIS_RESULTS_INSPECT.into(),
        tool_version: 1,
        parameters: json!({"limit":2}),
        targets: TargetSelector::Explicit {
            ids: vec![analysis_resource.id],
        },
        expected_revision: DocumentRevision(app.doc.automation_revision),
        caller: CallerType::Human,
    };
    let plan = plan_tool(&app, req).unwrap();
    let result = execute_tool(&mut app, plan, ExecutionAuthority::Read).unwrap();
    assert_eq!(result.value["analyses"][0]["valid"], true);
    assert_eq!(result.value["valid"], true);
    let descriptor = ToolRegistry::built_in()
        .get(ANALYSIS_RESULTS_INSPECT)
        .unwrap()
        .clone();
    for required in descriptor.result_schema["properties"]["value"]["required"]
        .as_array()
        .unwrap()
    {
        assert!(
            result.value.get(required.as_str().unwrap()).is_some(),
            "Missing {required}"
        );
    }
    assert_eq!(
        result.value["analyses"][0]["diagnostics"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let inspect = |app: &mut PlotiumApp| {
        let request = ToolRequest {
            tool_id: ANALYSIS_RESULTS_INSPECT.into(),
            tool_version: 1,
            parameters: json!({}),
            targets: TargetSelector::Explicit {
                ids: vec![table_id.to_string()],
            },
            expected_revision: DocumentRevision(app.doc.automation_revision),
            caller: CallerType::Human,
        };
        let plan = plan_tool(app, request).unwrap();
        execute_tool(app, plan, ExecutionAuthority::Read)
            .unwrap()
            .value
    };
    app.doc.datasets[1]
        .as_table_mut()
        .unwrap()
        .curve_fit_analyses[0]
        .selection
        .as_mut()
        .unwrap()
        .source_revision = plotium_data::RevisionId::new();
    let stale = inspect(&mut app);
    assert_eq!(stale["valid"], false);
    assert_eq!(stale["analyses"][0]["valid"], false);
}

#[test]
fn scientific_api_desktop_extraction_preserves_native_values_and_stable_history() {
    let mut app = app();
    let data = app.doc.datasets[0].as_nmr2d_mut().unwrap();
    let id = data.region_analysis.allocate_region_id().unwrap();
    data.region_analysis.regions.push(Region {
        id,
        lo: 0.8,
        hi: 1.2,
        name: "Signal".into(),
        metric: Some(RegionMetric::Height),
        label_position: None,
        color: [31, 111, 235],
    });
    let native = app
        .build_region_table(0, None)
        .unwrap()
        .fit_analysis_view()
        .unwrap();
    app.request_region_table_operation(0, true);
    assert!(app.compute_busy(), "{}", app.session.status);
    assert_eq!(app.doc.datasets.len(), 1);
    wait_for_compute(&mut app);
    assert_eq!(app.doc.datasets.len(), 2, "{}", app.session.status);
    assert_eq!(app.doc.canvases.len(), 1);
    let table = app.doc.datasets[1].as_table().unwrap();
    let view = table.fit_analysis_view().unwrap();
    assert_eq!(view.x.values, native.x.values);
    assert_eq!(view.series[0].value.values, native.series[0].value.values);
    assert!(table.provenance.is_some());
    let table_id = table.resource_id;
    let column = table.series_bindings[0].value_column;
    app.request_diffusion_curve_fit(1, column, Default::default());
    wait_for_compute(&mut app);
    assert!(
        !app.doc.datasets[1]
            .as_table()
            .unwrap()
            .curve_fit_analyses
            .is_empty(),
        "{}",
        app.session.status
    );
    app.undo();
    app.undo();
    assert_eq!(app.doc.datasets.len(), 1);
    assert!(app.doc.canvases.is_empty());
    app.redo();
    assert_eq!(app.doc.datasets[1].resource_id(), table_id);
    assert_eq!(
        app.doc.datasets[1].as_table().unwrap().series_bindings[0].value_column,
        column
    );
    assert!(!app.compute_busy());
}

#[test]
fn scientific_api_field_version_change_is_stale_even_without_document_edit() {
    let mut app = app();
    let plan = plan_tool(
        &app,
        request(&app, DOSY_BUILD, json!({"method":"mono_exp"}), true),
    )
    .unwrap();
    let id = plan.task_id.clone().unwrap();
    execute_tool(&mut app, plan, ExecutionAuthority::ReversibleModify).unwrap();
    let receipt = app.session.scientific_tasks.get(&id).unwrap();
    let source = receipt.sources[0].clone();
    let revision = app.doc.automation_revision;
    let version = app.session.compute.reserve_field_version().unwrap();
    app.session.compute.promote_field_version(
        VersionedFieldRef {
            field: FieldRef {
                resource: source.dataset,
                field: source.field,
            },
            version,
        },
        None,
    );
    wait_for_compute(&mut app);
    let receipt = app.session.scientific_tasks.get(&id).unwrap();
    assert_eq!(receipt.status, TaskStatus::Stale);
    assert_eq!(
        receipt.failure_code.as_deref(),
        Some("source_version_changed")
    );
    assert_eq!(app.doc.automation_revision, revision);
    assert!(app.doc.datasets[0].as_nmr2d().unwrap().dosy_map.is_none());
}

#[test]
fn scientific_api_worker_panic_becomes_a_terminal_receipt_without_commit() {
    let mut app = app();
    let dataset = app.doc.datasets[0].resource_id();
    let id = TaskId(uuid::Uuid::new_v4().to_string());
    let receipt = TaskReceipt {
        task_id: id.clone(),
        plan_id: None,
        operation: "analysis.regions.extract".into(),
        sources: vec![],
        project_context: app.session.project_context,
        targets: vec![ResourceRef::from(dataset)],
        start_revision: DocumentRevision(app.doc.automation_revision),
        end_revision: None,
        status: TaskStatus::Queued,
        phase: "queued".into(),
        completed: None,
        total: None,
        failure_code: None,
        message: None,
        produced: vec![],
        modified: vec![],
        summary: serde_json::Value::Null,
    };
    app.start_prepared_scientific(
        dataset,
        receipt,
        Box::new(|_| panic!("synthetic worker failure")),
    )
    .unwrap();
    wait_for_compute(&mut app);
    let receipt = app.session.scientific_tasks.get(&id).unwrap();
    assert_eq!(receipt.status, TaskStatus::Failed);
    assert_eq!(receipt.failure_code.as_deref(), Some("worker_failed"));
    assert!(app.session.undo_stack.is_empty());
    assert_eq!(app.doc.datasets.len(), 1);
}

#[test]
fn synchronous_scheme_worker_failure_stops_dependent_nodes() {
    let mut app = app();
    let folder = tempfile::tempdir().unwrap();
    let scheme = folder.path().join("routine.plotiumproc");
    crate::project::save_scheme(&scheme, &app.doc.datasets[0]).unwrap();
    let source = app.doc.datasets[0].resource_id();
    let before = crate::actions::DatasetProcessingState::from_dataset(&app.doc.datasets[0]);
    let name = app.doc.datasets[0].display_name().to_owned();
    app.session.compute = ComputeService::inactive();
    let workflow: WorkflowDefinition = serde_json::from_value(json!({
        "schema":"plotium.workflow.v1", "nodes":[
            {"id":"process","tool_id":"processing.apply_scheme",
             "parameters":{"path":scheme},"targets":{"kind":"explicit","ids":[source]}},
            {"id":"rename","tool_id":"resource.rename",
             "parameters":{"name":"Unexpected"},"targets":{"kind":"explicit","ids":[source]},
             "dependencies":["process"]}
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
    assert_eq!(manifest.nodes.len(), 1);
    assert_eq!(manifest.errors.len(), 1);
    assert_eq!(
        app.session.scientific_tasks.list(0, 128)[0].status,
        TaskStatus::Failed
    );
    assert_eq!(app.doc.datasets[0].display_name(), name);
    assert_eq!(
        crate::actions::DatasetProcessingState::from_dataset(&app.doc.datasets[0]),
        before
    );
}
