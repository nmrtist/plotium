//! The same plane and phase operations used by the desktop workspace.
use super::*;
use serde::Deserialize;
use serde_json::json;

pub const NMR_VOLUME_INSPECT: &str = "nmr.volume.inspect";
pub const NMR_VOLUME_EDIT: &str = "nmr.volume.edit";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VolumeEditParams {
    pub fixed_axis: usize,
    pub index: usize,
    #[serde(default)]
    pub phases: Option<[plotium_processing::nmr_volume::VolumePhase; 3]>,
}
pub(super) fn execute(
    app: &mut crate::state::PlotiumApp,
    plan: &ToolPlan,
) -> Result<ToolResult, AutomationError> {
    let [target] = plan.frozen_targets.targets.as_slice() else {
        return Err(AutomationError::InvalidSelector(
            "Select one three-dimensional spectrum".into(),
        ));
    };
    let dataset = app
        .doc
        .datasets
        .iter()
        .find(|d| d.resource_id().to_string() == target.id)
        .ok_or_else(|| AutomationError::InvalidSelector("Select an existing spectrum".into()))?;
    let id = dataset.resource_id();
    if dataset.as_nmr2d().and_then(|n| n.volume.as_ref()).is_none() {
        return Err(AutomationError::InvalidSelector(
            "Open a three-dimensional spectrum first".into(),
        ));
    }
    if plan.request.tool_id == NMR_VOLUME_EDIT {
        let p: VolumeEditParams =
            super::registry::parse(NMR_VOLUME_EDIT, plan.request.parameters.clone())?;
        // Validate the requested plane before making either edit.
        let n = dataset.as_nmr2d().expect("validated above");
        n.volume
            .as_ref()
            .expect("validated above")
            .processed
            .plane_source(p.fixed_axis, p.index)
            .map_err(AutomationError::Execution)?;
        if p.phases.is_some() {
            return start_phase_task(app, plan, id, p);
        }
        app.select_nmr_volume_plane(id, p.fixed_axis, p.index)
            .map_err(AutomationError::Execution)?;
    }
    let n = app
        .doc
        .datasets
        .iter()
        .find(|d| d.resource_id() == id)
        .and_then(|d| d.as_nmr2d())
        .ok_or_else(|| AutomationError::Execution("Spectrum disappeared".into()))?;
    let v = n
        .volume
        .as_ref()
        .ok_or_else(|| AutomationError::Execution("Volume disappeared".into()))?;
    let axes=v.processed.axes().iter().enumerate().map(|(axis,a)|json!({"axis":axis,"nucleus":a.nucleus(),"points":v.processed.shape()[axis],"first_ppm":a.coordinate(0).ok(),"last_ppm":a.coordinate(v.processed.shape()[axis]-1).ok()})).collect::<Vec<_>>();
    let mut result = super::tool_executors::readonly_value(
        &plan.request.tool_id,
        plan.frozen_targets.revision,
        json!({"shape":v.processed.shape(),"axes":axes,"fixed_axis":v.fixed_axis,"index":v.index,"fixed_ppm":v.processed.axes()[v.fixed_axis].coordinate(v.index).map_err(|e|AutomationError::Execution(e.to_string()))?,"phases":v.phases,"phase_status":"user-adjustable; not independently calibrated"}),
    );
    if plan.request.tool_id == NMR_VOLUME_EDIT {
        result.modified.push(target.clone());
    }
    result.after_revision = DocumentRevision(app.doc.automation_revision);
    Ok(result)
}
fn start_phase_task(
    app: &mut crate::state::PlotiumApp,
    plan: &ToolPlan,
    id: crate::state::DatasetId,
    p: VolumeEditParams,
) -> Result<ToolResult, AutomationError> {
    use crate::state::{ComputeService, PlotiumApp, ScientificOutput, ScientificWork};
    let snapshots = app.doc.datasets.clone();
    let canvases = app.doc.canvases.clone();
    let settings = app.settings.clone();
    let phases = p.phases.expect("phase task requires phases");
    let work: ScientificWork = Box::new(move |token| {
        let mut worker = PlotiumApp::with_compute(settings, ComputeService::inactive());
        worker.doc.datasets = snapshots;
        worker.doc.canvases = canvases;
        let di = worker.doc.dataset_index(id).ok_or("Spectrum was removed")?;
        let mut after = worker.doc.datasets[di].clone();
        let n = after
            .as_nmr2d_mut()
            .ok_or("Select a three-dimensional spectrum")?;
        let cancellation = nmr::CancellationToken::new();
        if token.is_cancelled() {
            return Err("Processing cancelled".into());
        }
        let mut progress = |_| {
            if token.is_cancelled() {
                cancellation.cancel();
            }
        };
        let mut context = nmr::ExecutionContext::default()
            .with_cancellation(cancellation.clone())
            .with_progress(&mut progress);
        n.set_volume_phases_with_context(phases, &mut context)?;
        n.select_volume_plane(p.fixed_axis, p.index)?;
        let action = worker.volume_install_action(id, after)?;
        Ok(ScientificOutput {
            create_canvas: false,
            action: Box::new(action),
            summary: json!({"fixed_axis":p.fixed_axis,"index":p.index,"phases":phases}),
            produced: vec![],
        })
    });
    let receipt = TaskReceipt {
        sources: vec![],
        task_id: plan
            .task_id
            .clone()
            .ok_or_else(|| AutomationError::Execution("Task identity missing".into()))?,
        plan_id: plan.submitted_plan_id.clone(),
        operation: NMR_VOLUME_EDIT.into(),
        project_context: app.session.project_context,
        targets: plan.frozen_targets.targets.clone(),
        start_revision: plan.frozen_targets.revision,
        end_revision: None,
        status: TaskStatus::Queued,
        phase: "queued".into(),
        completed: None,
        total: None,
        failure_code: None,
        message: None,
        produced: vec![],
        modified: vec![ResourceRef::from(id)],
        summary: serde_json::Value::Null,
    };
    let receipt = app
        .start_prepared_scientific(id, receipt, work)
        .map_err(AutomationError::Execution)?;
    Ok(super::tool_executors::readonly_value(
        NMR_VOLUME_EDIT,
        plan.frozen_targets.revision,
        json!({"task":receipt}),
    ))
}
pub(super) fn descriptors() -> Vec<ToolDescriptor> {
    [(NMR_VOLUME_INSPECT,false),(NMR_VOLUME_EDIT,true)].into_iter().map(|(id,write)|ToolDescriptor{
        id:id.into(),version:1,title:if write{"Set 3D spectrum plane and phase"}else{"Inspect 3D spectrum"}.into(),description:"Inspect or navigate an acquisition's three-dimensional frequency spectrum. Acquisition axes are zero based; no Fourier transform runs when changing only the plane.".into(),
        parameter_schema:if write{json!({"type":"object","additionalProperties":false,"required":["fixed_axis","index"],"properties":{"fixed_axis":{"type":"integer","minimum":0,"maximum":2},"index":{"type":"integer","minimum":0},"phases":{"type":"array","minItems":3,"maxItems":3,"items":{"type":"object","additionalProperties":false,"required":["p0_degrees","p1_degrees","pivot_fraction"],"properties":{"p0_degrees":{"type":"number"},"p1_degrees":{"type":"number"},"pivot_fraction":{"type":"number","minimum":0,"maximum":1}}}}}})}else{json!({"type":"object","additionalProperties":false})},
        result_schema:json!({"type":"object"}),target_kinds:vec![ResourceKindId::new(KIND_DATASET)],required_capabilities:vec![CapabilityId::new(CAP_PROCESSING_SCHEME)],effect:if write{EffectLevel::Reversible}else{EffectLevel::ReadOnly},undoable:write,deterministic:true,task_kind:if write{"background"}else{"synchronous"}.into()
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn run(
        app: &mut crate::state::PlotiumApp,
        tool: &str,
        parameters: serde_json::Value,
    ) -> Result<ToolResult, AutomationError> {
        let request = ToolRequest {
            tool_id: tool.into(),
            tool_version: 1,
            parameters,
            targets: TargetSelector::Explicit {
                ids: vec![app.doc.datasets[0].resource_id().to_string()],
            },
            expected_revision: DocumentRevision(app.doc.automation_revision),
            caller: CallerType::Agent,
        };
        let plan = plan_tool(app, request)?;
        execute_tool(app, plan, ExecutionAuthority::ReversibleModify)
    }
    #[test]
    fn automation_uses_shared_plane_and_rejects_invalid_coordinates() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../io/tests/fixtures/nmr/bruker-3d-qf");
        let loaded = plotium_io::load_path(&path).unwrap();
        let (dataset, _) = crate::workflow::dataset_from_acquisition(loaded.acquisition).unwrap();
        let mut app = crate::state::PlotiumApp::new();
        app.doc.datasets.push(dataset);
        let initial = run(&mut app, NMR_VOLUME_INSPECT, json!({})).unwrap();
        assert_eq!(initial.value["shape"], json!([5, 3, 4]));
        let changed = run(&mut app, NMR_VOLUME_EDIT, json!({"fixed_axis":2,"index":1})).unwrap();
        assert_eq!(changed.value["index"], 1);
        assert_eq!(changed.value["fixed_axis"], 2);
        let revision = app.doc.automation_revision;
        assert!(run(&mut app, NMR_VOLUME_EDIT, json!({"fixed_axis":3,"index":0})).is_err());
        assert_eq!(revision, app.doc.automation_revision);
        app.undo();
        let restored = run(&mut app, NMR_VOLUME_INSPECT, json!({})).unwrap();
        assert_eq!(initial.value, restored.value);
        let phase = json!({"p0_degrees":30.0,"p1_degrees":0.0,"pivot_fraction":0.5});
        let queued = run(
            &mut app,
            NMR_VOLUME_EDIT,
            json!({"fixed_axis":1,"index":2,"phases":[phase.clone(),phase.clone(),phase]}),
        )
        .unwrap();
        assert!(queued.value.get("task").is_some());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while app.compute_busy() && std::time::Instant::now() < deadline {
            app.poll_compute();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        app.poll_compute();
        let changed = run(&mut app, NMR_VOLUME_INSPECT, json!({})).unwrap();
        assert_eq!(changed.value["fixed_axis"], 1);
        assert_eq!(changed.value["phases"][0]["p0_degrees"], 30.0);
        app.undo();
        let restored = run(&mut app, NMR_VOLUME_INSPECT, json!({})).unwrap();
        assert_eq!(initial.value, restored.value);
    }
}
