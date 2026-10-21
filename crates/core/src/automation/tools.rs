//! Planning, dispatch and audit for the registered tools.
//!
//! What each tool *does* lives in [`super::tool_executors`]; this module decides
//! which targets a tool may touch, routes an approved plan to its executor, and
//! records the evidence a run leaves behind.

use super::*;
use crate::state::{Document, PlotiumApp};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;

use super::registry::*;
use super::tool_executors::*;

pub fn plan_tool(app: &PlotiumApp, request: ToolRequest) -> Result<ToolPlan, AutomationError> {
    let registry = ToolRegistry::built_in();
    let descriptor = registry
        .get(&request.tool_id)
        .ok_or_else(|| AutomationError::UnknownTool(request.tool_id.clone()))?;
    if request.tool_version != descriptor.version {
        return Err(AutomationError::ToolVersion {
            tool_id: request.tool_id.clone(),
            version: request.tool_version,
        });
    }
    validate_parameters(&request.tool_id, request.parameters.clone())?;
    if request.tool_id == "data.import"
        && request.targets != (TargetSelector::Explicit { ids: Vec::new() })
    {
        return Err(AutomationError::InvalidSelector(
            "Import requires an explicitly empty project target; paths identify its inputs.".into(),
        ));
    }
    if request.tool_id == "data.transform" && is_table_transform(&request.parameters) {
        TableTransformSnapshot::capture(app, &request)?;
    }
    if matches!(request.tool_id.as_str(), TABLE_PATCH | TABLE_REFRESH) {
        TableUpdateSnapshot::capture(app, &request)?;
    }
    let provider = ProjectResourceProvider::new(app);
    if request.expected_revision != provider.revision() {
        return Err(AutomationError::StaleRevision {
            expected: request.expected_revision.0,
            actual: provider.revision().0,
        });
    }
    if matches!(request.targets, TargetSelector::Components { .. })
        && !properties::is_property_tool(&request.tool_id)
        && request.tool_id != DOCUMENT_INSPECT
    {
        return Err(AutomationError::InvalidSelector(
            "components are only supported by property and document inspection tools".into(),
        ));
    }
    let binding_plan = if request.tool_id == PLOT_CREATE {
        Some(super::plot_creation::prepare(app, &request)?)
    } else if is_curve_binding_tool(&request.tool_id) {
        Some(super::curve_bindings::prepare(app, &request)?)
    } else {
        None
    };
    let figure_plan = if request.tool_id == FIGURE_EDIT {
        Some(super::figure_edit::prepare(app, &request)?)
    } else {
        None
    };
    let scientific_plot = if is_scientific_plot_tool(&request.tool_id) {
        Some(super::scientific_plot::prepare(app, &request)?)
    } else {
        None
    };
    let frozen_targets = if is_scientific_plot_tool(&request.tool_id) {
        super::scientific_plot::freeze(&request)?
    } else if request.tool_id == FIGURE_EDIT {
        super::figure_edit::freeze(&request)?
    } else if request.tool_id == PLOT_CREATE {
        super::plot_creation::freeze(&request)?
    } else if is_curve_binding_tool(&request.tool_id) {
        super::curve_bindings::freeze(&request)?
    } else if matches!(request.tool_id.as_str(), TABLE_COLUMNS | TABLE_READ_ROWS) {
        super::table_read::freeze_table(app, &request)?
    } else if request.tool_id == FIGURE_DIAGNOSE {
        super::diagnostics::freeze(app, &request)?
    } else if request.tool_id == IMAGE_PREVIEW {
        super::preview::freeze(app, &request)?
    } else if request.tool_id == DOCUMENT_INSPECT {
        super::document::freeze(app, &request)?
    } else if request.tool_id == super::property_batch::TOOL_SET_MANY {
        super::property_batch::freeze(&provider, &request)?
    } else {
        freeze_targets(&provider, &request.targets)?
    };
    let targets = if request.tool_id == DOCUMENT_INSPECT {
        let params: DocumentInspectParams = parse(DOCUMENT_INSPECT, request.parameters.clone())?;
        vec![PlannedTarget {
            target: params.target,
            status: TargetCompatibility::Compatible,
            reason: "validated document identity".into(),
        }]
    } else if frozen_targets.component_targets.is_empty() {
        declared_targets(&provider, descriptor, &frozen_targets)
    } else {
        frozen_targets
            .component_targets
            .iter()
            .map(|target| PlannedTarget {
                target: target.clone(),
                status: TargetCompatibility::Compatible,
                reason: String::new(),
            })
            .collect()
    };
    // The one per-tool planning seam. The uniform gate above answers "may this
    // tool touch this resource at all", which is the same question for every
    // tool; a property tool additionally has to expand one resource into the
    // components it addresses, and its applicability comes from a property
    // definition rather than from the tool descriptor. Refining an already
    // gated list keeps that entirely additive: a tool with no refinement is
    // planned by the same code as before, byte for byte.
    let targets = if request.tool_id == super::property_batch::TOOL_SET_MANY {
        super::property_batch::refine_plan(app, &request)?
    } else {
        properties::refine_plan(app, &request, targets)?
    };
    let compatible = targets
        .iter()
        .filter(|target| target.status == TargetCompatibility::Compatible)
        .count();
    let frozen_scheme = if request.tool_id == "processing.apply_scheme" {
        let params: SchemeParams = parse(&request.tool_id, request.parameters.clone())?;
        Some(FrozenProcessingScheme::capture(&params.path)?)
    } else {
        None
    };
    let mut plan = ToolPlan {
        scientific_plot,
        desktop_region: None,
        task_id: None,
        submitted_plan_id: None,
        scientific_input: None,
        scientific_ilt_params: None,
        processing_edit: None,
        scientific_preparation: None,
        frozen_scheme,
        figure_plan,
        binding_plan,
        request,
        frozen_targets,
        targets,
        estimated_changes: match descriptor.effect {
            EffectLevel::ReadOnly => Vec::new(),
            _ => vec![format!("{} compatible resource(s)", compatible)],
        },
        outputs: tool_outputs(&descriptor.id),
        required_authority: descriptor.effect.required_authority(),
    };
    if plan.request.tool_id == DIFFUSION_SET_INPUT {
        super::science::validate_input_update(app, &plan)?;
    }
    if plan.request.tool_id == PROCESSING_EDIT {
        plan.processing_edit = Some(super::processing_edit::prepare(app, &plan)?);
        plan.task_id = Some(TaskId(uuid::Uuid::new_v4().to_string()));
    }
    if plan.request.tool_id == "processing.apply_scheme" {
        plan.scientific_preparation = Some(super::prepared_processing::prepare_scheme(app, &plan)?);
        plan.task_id = Some(TaskId(uuid::Uuid::new_v4().to_string()));
    }
    if matches!(
        plan.request.tool_id.as_str(),
        REGIONS_EXTRACT | DIFFUSION_FIT
    ) {
        plan.scientific_preparation = Some(super::region_fit::prepare(app, &plan)?);
        plan.task_id = Some(TaskId(uuid::Uuid::new_v4().to_string()));
    }
    if plan.request.tool_id == DOSY_BUILD {
        plan.scientific_input = Some(super::dosy::prepare(app, &plan)?);
        plan.scientific_ilt_params = super::dosy::resolved_ilt_params(app, &plan)?;
        plan.task_id = Some(TaskId(uuid::Uuid::new_v4().to_string()));
    }
    Ok(plan)
}

/// The declared kind/capability gate every tool shares.
fn declared_targets(
    provider: &ProjectResourceProvider<'_>,
    descriptor: &ToolDescriptor,
    frozen_targets: &FrozenTargetSet,
) -> Vec<PlannedTarget> {
    frozen_targets
        .targets
        .iter()
        .map(|target| {
            let target_descriptor = provider.inspect(&target.id);
            let compatibility = target_descriptor
                .as_ref()
                .map(|target_descriptor| {
                    (descriptor.target_kinds.is_empty()
                        || descriptor
                            .target_kinds
                            .contains(&target_descriptor.resource.kind))
                        && descriptor
                            .required_capabilities
                            .iter()
                            .all(|required| target_descriptor.capabilities.contains(required))
                })
                .unwrap_or(false);
            PlannedTarget {
                target: TargetRef::resource(target.clone()),
                status: if compatibility {
                    TargetCompatibility::Compatible
                } else {
                    TargetCompatibility::Skipped
                },
                reason: if compatibility {
                    "target satisfies the declared kind and capabilities".to_owned()
                } else {
                    "target lacks a required kind or capability".to_owned()
                },
            }
        })
        .collect()
}

/// Run one tool, retaining project provenance for non-query automation calls.
/// Interactive edits use document state and Undo instead of run manifests.
pub fn execute_tool(
    app: &mut PlotiumApp,
    plan: ToolPlan,
    authority: ExecutionAuthority,
) -> Result<ToolResult, AutomationError> {
    if let Some(id) = plan.task_id.as_ref() {
        if let Some(receipt) = app.session.scientific_tasks.get(id) {
            return Ok(readonly_value(
                &plan.request.tool_id,
                DocumentRevision(app.doc.automation_revision),
                serde_json::json!({"task":receipt}),
            ));
        }
        if app.session.scientific_tasks.was_consumed(id) {
            return Err(AutomationError::Execution(
                "task_not_found: the task receipt was evicted; do not replay it.".into(),
            ));
        }
    }
    let caller = plan.request.caller;
    if caller == CallerType::Human || !persists_tool_run(&plan.request.tool_id) {
        return execute_planned_tool(app, plan, authority);
    }
    let run = PreparedToolRun::begin(app, &plan);
    let outcome = execute_planned_tool(app, plan, authority);
    run.finish(app, &outcome)?;
    outcome
}

/// Audit state for a tool whose immutable computation can finish off-thread.
/// The caller must finish on the original project, or discard on project close.
pub struct PreparedToolRun {
    plan: ToolPlan,
    start_table_revisions: BTreeMap<String, TableRevisionRecord>,
    started_unix_ms: u128,
    start_revision: DocumentRevision,
    started: std::time::Instant,
}
impl PreparedToolRun {
    pub fn begin(app: &PlotiumApp, plan: &ToolPlan) -> Self {
        Self {
            plan: plan.clone(),
            start_table_revisions: typed_table_revisions(app, "input"),
            started_unix_ms: unix_ms(),
            start_revision: DocumentRevision(app.doc.automation_revision),
            started: std::time::Instant::now(),
        }
    }
    pub fn finish(
        self,
        app: &mut PlotiumApp,
        outcome: &Result<ToolResult, AutomationError>,
    ) -> Result<(), AutomationError> {
        let Self {
            plan,
            start_table_revisions,
            started_unix_ms,
            start_revision,
            started,
        } = self;
        if plan.request.caller == CallerType::Human || !persists_tool_run(&plan.request.tool_id) {
            return Ok(());
        }
        let caller = plan.request.caller;
        let node = single_node_workflow(&plan.request);
        let frozen_targets = plan.frozen_targets.clone();
        let parameters = plan.request.parameters.clone();
        let tool_id = plan.request.tool_id.clone();
        let tool_version = plan.request.tool_version;
        let end_revision = DocumentRevision(app.doc.automation_revision);
        let (result, errors) = match outcome {
            Ok(result) => {
                // A tool that reports a per-target failure without failing outright
                // is recorded the way `execute_workflow` records the same node, so
                // the two manifests stay one shape (§9, principle 5).
                let errors = if result
                    .targets
                    .iter()
                    .any(|target| target.outcome == TargetOutcome::Failed)
                {
                    vec![format!("{SINGLE_CALL_NODE}: one or more targets failed")]
                } else {
                    Vec::new()
                };
                (result.clone(), errors)
            }
            Err(error) => (
                ToolResult {
                    tool_id: tool_id.clone(),
                    before_revision: start_revision,
                    after_revision: end_revision,
                    targets: Vec::new(),
                    produced: Vec::new(),
                    modified: Vec::new(),
                    diagnostics: vec![error.to_string()],
                    verification: Vec::new(),
                    value: serde_json::Value::Null,
                },
                vec![error.to_string()],
            ),
        };
        let record = NodeRunRecord {
            node_id: SINGLE_CALL_NODE.to_owned(),
            tool_id: tool_id.clone(),
            parameters,
            frozen_targets,
            result,
            duration_ms: started.elapsed().as_millis(),
        };
        let workflow = serde_json::to_value(&node)
            .map_err(|error| AutomationError::Execution(error.to_string()))?;
        let canonical = serde_json::to_vec(&workflow)
            .map_err(|error| AutomationError::Execution(error.to_string()))?;
        // Reuse the workflow executor's extraction rather than a second copy: a
        // `data.transform` called directly by an agent has to leave the same input
        // and output table revisions, plan fingerprint and backend identity that the
        // same transform leaves when a workflow runs it, or the run is not
        // reproducible from its own record.
        let records = vec![record];
        let (table_revisions, table_plans) =
            table_run_records(app, &records, &start_table_revisions);
        record_run(
            &mut app.doc,
            RunManifest {
                schema: RUN_MANIFEST_SCHEMA.to_owned(),
                run_id: uuid::Uuid::new_v4().to_string(),
                caller,
                workflow_hash: format!("{:x}", Sha256::digest(&canonical)),
                workflow,
                application_version: env!("CARGO_PKG_VERSION").to_owned(),
                tool_versions: BTreeMap::from([(tool_id, tool_version)]),
                start_revision,
                end_revision,
                started_unix_ms,
                finished_unix_ms: unix_ms(),
                cancelled: matches!(
                    outcome,
                    Err(AutomationError::DiagnosticCancelled
                        | AutomationError::PreviewCancelled
                        | AutomationError::TableTransformCancelled)
                ),
                nodes: records,
                warnings: Vec::new(),
                errors,
                verification: Vec::new(),
                table_revisions,
                table_plans,
            },
        );
        Ok(())
    }
}

/// Whether an automation call needs durable project provenance.
pub(super) fn persists_tool_run(tool_id: &str) -> bool {
    ToolRegistry::built_in()
        .get(tool_id)
        .is_none_or(|descriptor| descriptor.effect != EffectLevel::ReadOnly)
}

/// Append a modifying run and mark its provenance as unsaved document content.
///
/// `automation_revision` is deliberately untouched. It is the baseline of the
/// optimistic-concurrency check, and advancing it here would invalidate the plan
/// the caller is still holding — dirtiness says "the file on disk is stale",
/// which is a different question from "has the document the caller planned
/// against moved".
pub(super) fn record_run(document: &mut Document, mut manifest: RunManifest) {
    // A workflow and a direct call share the same persistence boundary. PNG
    // transport data is never project state, including when invoked by workflows.
    for node in &mut manifest.nodes {
        if node.tool_id == FIGURE_DIAGNOSE {
            let value = &node.result.value;
            node.result.value = serde_json::json!({"project_context":value["project_context"],"basis_revision":value["basis_revision"],"canvas":value["canvas"],"output":value["output"],"coverage":value["coverage"],"truncated":value["truncated"],"audit":"summary_only"});
        }
        if node.tool_id == IMAGE_PREVIEW
            && let Some(value) = node.result.value.as_object_mut()
        {
            value.remove("png_base64");
        }
    }
    document.automation_runs.push(manifest);
    document.mark_dirty();
}

/// The node id a single call is recorded under.
pub const SINGLE_CALL_NODE: &str = "single-call";

/// The one-node workflow a single tool call is equivalent to.
fn single_node_workflow(request: &ToolRequest) -> WorkflowDefinition {
    WorkflowDefinition {
        schema: WORKFLOW_SCHEMA.to_owned(),
        inputs: BTreeMap::new(),
        nodes: vec![WorkflowNode {
            id: SINGLE_CALL_NODE.to_owned(),
            tool_id: request.tool_id.clone(),
            tool_version: request.tool_version,
            parameters: request.parameters.clone(),
            targets: request.targets.clone(),
            dependencies: Vec::new(),
            bindings: Vec::new(),
            condition: NodeCondition::Always,
            failure_policy: NodeFailurePolicy::Inherit,
        }],
        failure_policy: WorkflowFailurePolicy::Strict,
    }
}

use super::workflow::{table_run_records, typed_table_revisions, unix_ms};

/// Execute a plan without the single-call audit wrapper.
///
/// The workflow executor calls this: a run already writes one manifest for the
/// whole graph, and nesting a per-node manifest inside it would double-record
/// every node.
pub(super) fn execute_planned_tool(
    app: &mut PlotiumApp,
    plan: ToolPlan,
    authority: ExecutionAuthority,
) -> Result<ToolResult, AutomationError> {
    if authority < plan.required_authority {
        return Err(AutomationError::InsufficientAuthority {
            granted: authority,
            required: plan.required_authority,
        });
    }
    if plan.frozen_targets.revision.0 != app.doc.automation_revision {
        return Err(AutomationError::StaleRevision {
            expected: plan.frozen_targets.revision.0,
            actual: app.doc.automation_revision,
        });
    }
    let before = DocumentRevision(app.doc.automation_revision);
    let tool_id = plan.request.tool_id.clone();
    let mut result = match tool_id.as_str() {
        "project.get_blueprint" => readonly_value(
            &tool_id,
            before,
            serde_json::to_value(ProjectResourceProvider::new(app).blueprint())
                .map_err(|error| AutomationError::Execution(error.to_string()))?,
        ),
        "document.inspect" => super::document::execute(app, &plan)?,
        "resources.search" => execute_search(app, &plan)?,
        "resources.inspect" => execute_inspect(app, &plan)?,
        "data.preview" => execute_data_preview(app, &plan)?,
        "render.preview" => execute_render_preview(app, &plan)?,
        FIGURE_DIAGNOSE => {
            let params = parse(FIGURE_DIAGNOSE, plan.request.parameters.clone())?;
            let mut scan = FigureDiagnosticScan::capture(app, before, params)?;
            let mut value = scan.next_page(|| false)?;
            value["continuation_available"] = serde_json::json!(false);
            value["continuation_help"] = serde_json::json!(
                "Use plotium_figure_diagnose for retained cursor pages; direct core calls return one bounded batch."
            );
            readonly_value(FIGURE_DIAGNOSE, before, value)
        }
        IMAGE_PREVIEW => ImagePreviewSnapshot::capture(app, &plan)?.render()?,
        "results.compare" => execute_compare(app, &plan)?,
        "resource.rename" => execute_rename(app, &plan)?,
        "figure.apply_theme" => execute_theme(app, &plan)?,
        FIELD_PLOT_CREATE | DIFFUSION_PLOT_CREATE => super::scientific_plot::execute(app, &plan)?,
        PLOT_CREATE => super::plot_creation::execute(app, &plan)?,
        FIGURE_EDIT => super::figure_edit::execute(app, &plan)?,
        CURVE_BIND | CURVE_MATERIALIZE | CURVE_APPEND => {
            super::curve_bindings::execute(app, &plan)?
        }
        "processing.apply_scheme" => execute_scheme(app, &plan)?,
        REGIONS_EXTRACT | DIFFUSION_FIT => super::region_fit::execute(app, &plan)?,
        DOSY_BUILD => super::dosy::execute(app, &plan)?,
        ANALYSIS_RESULTS_INSPECT => super::dosy::inspect(app, &plan)?,
        PROCESSING_EDIT => super::processing_edit::execute(app, &plan)?,
        DIFFUSION_INSPECT | DIFFUSION_SET_INPUT | PROCESSING_INSPECT => {
            super::science::execute(app, &plan)?
        }
        TABLE_COLUMNS | TABLE_READ_ROWS => super::table_read::execute_table_read(app, &plan)?,
        "data.import" => execute_import(app, &plan)?,
        "data.transform" => execute_transform(app, &plan)?,
        TABLE_PATCH | TABLE_REFRESH => super::table_update::execute(app, &plan)?,
        "figure.export" => execute_export(app, &plan)?,
        super::property_batch::TOOL_SET_MANY => super::property_batch::execute(app, &plan)?,
        id if properties::is_property_tool(id) => {
            let targets = compatible_target_refs(&plan).cloned().collect();
            properties::execute(app, &plan, targets)?
        }
        _ => return Err(AutomationError::UnknownTool(tool_id)),
    };
    result.before_revision = before;
    result.after_revision = DocumentRevision(app.doc.automation_revision);
    record_external_write(app, &plan, &result);
    Ok(result)
}

/// Record an operation for any tool that wrote outside the document.
///
/// The test is the declared [`EffectLevel::ExternalWrite`], never a tool id: a
/// file that left the application is the thing worth recording, and every future
/// tool that writes one inherits this audit without touching this function.
/// The record names what was written and carries the fingerprints the tool
/// already computed, so "which bytes did this run produce" is answerable from
/// the operation history alone.
fn record_external_write(app: &mut PlotiumApp, plan: &ToolPlan, result: &ToolResult) {
    let external = ToolRegistry::built_in()
        .get(&plan.request.tool_id)
        .is_some_and(|descriptor| descriptor.effect == EffectLevel::ExternalWrite);
    if !external {
        return;
    }
    let succeeded = result
        .targets
        .iter()
        .filter(|target| target.outcome == TargetOutcome::Succeeded)
        .collect::<Vec<_>>();
    let failed = result
        .targets
        .iter()
        .filter(|target| target.outcome == TargetOutcome::Failed)
        .collect::<Vec<_>>();
    let files = succeeded
        .iter()
        .map(|target| target.fingerprints.len())
        .sum::<usize>();
    let summary = format!(
        "{} wrote {files} file(s) from {} target(s)",
        plan.request.tool_id,
        succeeded.len()
    );
    let mut diagnostics = Vec::new();
    for target in &succeeded {
        for fingerprint in &target.fingerprints {
            diagnostics.push(
                crate::operation::Diagnostic::new(
                    crate::operation::Severity::Info,
                    crate::operation::DiagnosticCode::ExportSucceeded,
                    format!(
                        "wrote {} bytes for {}",
                        fingerprint.bytes,
                        target.target.describe()
                    ),
                )
                .with_source("core.automation")
                .with_context("path", fingerprint.path.display().to_string())
                .with_context("sha256", fingerprint.sha256.clone())
                .with_context("role", fingerprint.role.clone()),
            );
        }
    }
    for target in &failed {
        diagnostics.push(
            crate::operation::Diagnostic::new(
                crate::operation::Severity::Error,
                crate::operation::DiagnosticCode::ExportFailed,
                format!("{}: {}", target.target.describe(), target.message),
            )
            .with_source("core.automation"),
        );
    }
    if succeeded.is_empty() && failed.is_empty() {
        diagnostics.push(
            crate::operation::Diagnostic::new(
                crate::operation::Severity::Warning,
                crate::operation::DiagnosticCode::ExportProducedNoFiles,
                "no target produced an external file".to_owned(),
            )
            .with_source("core.automation"),
        );
    }
    let id = app.session.begin_operation();
    let mut report = if failed.is_empty() {
        crate::operation::OperationReport::success(
            id,
            crate::operation::OperationKind::Export,
            summary,
            (),
        )
    } else {
        crate::operation::OperationReport::warning(
            id,
            crate::operation::OperationKind::Export,
            summary,
            (),
        )
    };
    report.diagnostics = diagnostics;
    app.session.record_operation(report);
}

/// The resources of every compatible planned target.
///
/// Tools that address whole resources keep reading a `ResourceRef` here: their
/// planned targets never carry a component, so projecting it away costs them
/// nothing and keeps the component out of code that has no meaning for it.
/// Tools whose plan expands into components read [`compatible_target_refs`].
pub(super) fn compatible_targets(plan: &ToolPlan) -> impl Iterator<Item = &ResourceRef> {
    compatible_target_refs(plan).map(|target| &target.resource)
}

pub(super) fn compatible_target_refs(plan: &ToolPlan) -> impl Iterator<Item = &TargetRef> {
    plan.targets
        .iter()
        .filter(|target| target.status == TargetCompatibility::Compatible)
        .map(|target| &target.target)
}

pub fn write_run_manifest(path: &Path, manifest: &RunManifest) -> Result<(), AutomationError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|source| AutomationError::Io {
            path: parent.to_owned(),
            source,
        })?;
    }
    let bytes = serde_json::to_vec_pretty(manifest)
        .map_err(|error| AutomationError::Execution(error.to_string()))?;
    let temporary = path.with_file_name(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        uuid::Uuid::new_v4()
    ));
    std::fs::write(&temporary, bytes).map_err(|source| AutomationError::Io {
        path: temporary.clone(),
        source,
    })?;
    crate::project::commit_atomic_file(&temporary, path).map_err(|source| AutomationError::Io {
        path: path.to_owned(),
        source,
    })
}
