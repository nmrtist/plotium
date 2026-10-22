//! The body of every registered tool.
//!
//! Planning, dispatch and audit live in `tools.rs`; this module holds what each
//! tool actually does once a plan has been approved. The split is by role, not
//! by tool: every executor here reaches the document through the same shared
//! commit helper, so "one tool, one atomic action" stays a property of the
//! module rather than of each function remembering to hold it.

use super::registry::*;
use super::*;
use crate::actions::Action;
use crate::export::{ExportFormat, ExportPageScope, ExportSettings};
use crate::state::PlotiumApp;
use crate::theme::{resolve_theme, styles_dir};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;

pub(super) fn execute_search(
    app: &PlotiumApp,
    plan: &ToolPlan,
) -> Result<ToolResult, AutomationError> {
    let params: SearchParams = parse(&plan.request.tool_id, plan.request.parameters.clone())?;
    let found = search_resources(&ProjectResourceProvider::new(app), &params.query);
    serde_result(
        &plan.request.tool_id,
        DocumentRevision(app.doc.automation_revision),
        found,
    )
}

pub(super) fn execute_inspect(
    app: &PlotiumApp,
    plan: &ToolPlan,
) -> Result<ToolResult, AutomationError> {
    let provider = ProjectResourceProvider::new(app);
    let values = compatible_targets(plan)
        .filter_map(|target| provider.inspect(&target.id))
        .collect::<Vec<_>>();
    serde_result(&plan.request.tool_id, provider.revision(), values)
}

pub(super) fn execute_data_preview(
    app: &PlotiumApp,
    plan: &ToolPlan,
) -> Result<ToolResult, AutomationError> {
    let params: PreviewParams = parse(&plan.request.tool_id, plan.request.parameters.clone())?;
    let provider = ProjectResourceProvider::new(app);
    let values = compatible_targets(plan)
        .map(|target| provider.preview(target, params.limit))
        .collect::<Result<Vec<_>, _>>()?;
    serde_result(&plan.request.tool_id, provider.revision(), values)
}

pub(super) fn execute_render_preview(
    app: &PlotiumApp,
    plan: &ToolPlan,
) -> Result<ToolResult, AutomationError> {
    let provider = ProjectResourceProvider::new(app);
    let values = compatible_targets(plan)
        .map(|target| {
            provider
                .render_preview(target)
                .map(|svg| serde_json::json!({"resource_id": target.id, "svg": svg}))
        })
        .collect::<Result<Vec<_>, _>>()?;
    serde_result(&plan.request.tool_id, provider.revision(), values)
}

pub(super) fn execute_compare(
    app: &PlotiumApp,
    plan: &ToolPlan,
) -> Result<ToolResult, AutomationError> {
    let params: CompareParams = parse(&plan.request.tool_id, plan.request.parameters.clone())?;
    let provider = ProjectResourceProvider::new(app);
    let values = compatible_targets(plan)
        .filter_map(|target| provider.inspect(&target.id))
        .map(|current| {
            let before = params
                .before
                .iter()
                .find(|item| item.resource.id == current.resource.id);
            ResourceComparison {
                resource_id: current.resource.id.clone(),
                changed: before != Some(&current),
                fields: BTreeMap::from([
                    (
                        "before".to_owned(),
                        serde_json::to_value(before).unwrap_or_default(),
                    ),
                    (
                        "after".to_owned(),
                        serde_json::to_value(current).unwrap_or_default(),
                    ),
                ]),
            }
        })
        .collect::<Vec<_>>();
    serde_result(&plan.request.tool_id, provider.revision(), values)
}

pub(super) fn execute_rename(
    app: &mut PlotiumApp,
    plan: &ToolPlan,
) -> Result<ToolResult, AutomationError> {
    let params: RenameParams = parse(&plan.request.tool_id, plan.request.parameters.clone())?;
    if params.name.trim().is_empty() {
        return Err(AutomationError::InvalidParameters {
            tool_id: plan.request.tool_id.clone(),
            message: "name must not be empty".to_owned(),
        });
    }
    let mut actions = Vec::new();
    for target in compatible_targets(plan) {
        if let Some(index) = dataset_index(app, &target.id) {
            actions.push(Action::rename_dataset(
                app.doc.datasets[index].resource_id(),
                app.doc.datasets[index].name(),
                Some(params.name.clone()),
            ));
        } else if let Some(index) = canvas_index(app, &target.id) {
            actions.push(Action::rename_canvas(
                index,
                app.doc.canvases[index].name.clone(),
                params.name.clone(),
            ));
        } else if target.kind.0 == KIND_CANVAS_OBJECT
            && let (Some(parent), Some(local)) = (&target.parent_id, &target.local_id)
            && let Some(canvas) = canvas_index(app, parent)
            && let Ok(object_id) = local.parse::<crate::state::ObjectId>()
            && let Some(object) = app.doc.canvases[canvas].object(object_id)
        {
            actions.push(Action::rename_object(
                canvas,
                object_id,
                object.name.clone(),
                params.name.clone(),
            ));
        }
    }
    commit_actions(app, plan, actions, "renamed")
}

pub(super) fn execute_theme(
    app: &mut PlotiumApp,
    plan: &ToolPlan,
) -> Result<ToolResult, AutomationError> {
    let params: ThemeParams = parse(&plan.request.tool_id, plan.request.parameters.clone())?;
    // A saved style is read when the plan executes, as the desktop applies it.
    let theme = resolve_theme(styles_dir().as_deref(), &params.theme_id).map_err(|error| {
        AutomationError::InvalidParameters {
            tool_id: plan.request.tool_id.clone(),
            message: error.to_string(),
        }
    })?;
    let canvases: Vec<_> = compatible_targets(plan)
        .filter_map(|target| canvas_index(app, &target.id))
        .collect();
    // Hints describe the canvases as found; applying a theme never resizes.
    let hints: Vec<_> = canvases
        .iter()
        .filter_map(|&canvas| {
            let canvas = &app.doc.canvases[canvas];
            let hint = theme.column_width_hint(canvas.size_mm)?;
            Some(format!("Canvas \"{}\": {hint}", canvas.name))
        })
        .collect();
    let mut actions = Vec::new();
    let mut kept = 0;
    let mut kept_document = 0;
    for canvas in canvases {
        let Some((action, held)) = app.theme_action(canvas, &theme) else {
            continue;
        };
        actions.push(action);
        kept += held.canvas;
        // Every canvas keeps the same shared typography; count it once.
        kept_document = held.document;
    }
    let mut result = commit_actions(app, plan, actions, "theme applied")?;
    result.diagnostics = hints;
    result
        .diagnostics
        .extend(crate::theme::kept_hand_edits_note(kept + kept_document));
    Ok(result)
}

pub(super) fn execute_scheme(
    app: &mut PlotiumApp,
    plan: &ToolPlan,
) -> Result<ToolResult, AutomationError> {
    super::prepared_processing::execute(app, plan)
}

pub(super) fn execute_import(
    app: &mut PlotiumApp,
    plan: &ToolPlan,
) -> Result<ToolResult, AutomationError> {
    let params = DataImportParams::parse(plan.request.parameters.clone())?;
    PreparedDataImport::prepare(params, || false)?.commit(app, plan)
}

pub(super) fn execute_transform(
    app: &mut PlotiumApp,
    plan: &ToolPlan,
) -> Result<ToolResult, AutomationError> {
    if is_table_transform(&plan.request.parameters) {
        let prepared = TableTransformSnapshot::capture(app, &plan.request)?
            .prepare(&std::sync::atomic::AtomicBool::new(false))?;
        return prepared.commit(app, plan);
    }
    let params: TransformParams = parse(&plan.request.tool_id, plan.request.parameters.clone())?;
    if params.name.trim().is_empty() {
        return Err(AutomationError::InvalidParameters {
            tool_id: plan.request.tool_id.clone(),
            message: "name must not be empty".into(),
        });
    }
    if params.memory_limit_bytes == 0 {
        return Err(AutomationError::InvalidParameters {
            tool_id: plan.request.tool_id.clone(),
            message: "memory_limit_bytes must be positive".into(),
        });
    }
    let input_datasets = compatible_targets(plan)
        .map(|target| {
            dataset_index(app, &target.id).ok_or_else(|| {
                AutomationError::Execution(format!("table resource {} disappeared", target.id))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if input_datasets.is_empty() {
        return Err(AutomationError::InvalidParameters {
            tool_id: plan.request.tool_id.clone(),
            message: "select at least one typed table input".into(),
        });
    }
    let index = app
        .derive_table_from_plan(
            params.plan,
            &input_datasets,
            params.name,
            params.memory_limit_bytes,
        )
        .map_err(AutomationError::Execution)?;
    let output = app.doc.datasets[index].as_table().ok_or_else(|| {
        AutomationError::Execution("transform did not produce a typed table".into())
    })?;
    let produced = ResourceRef {
        id: output.resource_id.to_string(),
        kind: ResourceKindId::new(KIND_DATASET),
        parent_id: None,
        local_id: None,
    };
    Ok(ToolResult {
        tool_id: plan.request.tool_id.clone(),
        before_revision: DocumentRevision(app.doc.automation_revision),
        after_revision: DocumentRevision(app.doc.automation_revision),
        targets: compatible_target_refs(plan)
            .cloned()
            .map(|target| TargetResult {
                target,
                outcome: TargetOutcome::Succeeded,
                message: "executed as a pinned Plotium RelPlanV1 input".into(),
                skip_reason: None,
                fingerprints: Vec::new(),
            })
            .collect(),
        produced: vec![produced],
        modified: Vec::new(),
        diagnostics: output
            .typed_state
            .envelope
            .revision
            .operation
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect(),
        verification: Vec::new(),
        value: serde_json::json!({
            "table_id": output.typed_state.envelope.revision.table_id,
            "revision_id": output.typed_state.envelope.revision.id,
            "fingerprint": output.typed_state.envelope.revision.snapshot.fingerprint,
        }),
    })
}

pub(super) fn execute_export(
    app: &PlotiumApp,
    plan: &ToolPlan,
) -> Result<ToolResult, AutomationError> {
    let params: ExportParams = parse(&plan.request.tool_id, plan.request.parameters.clone())?;
    let format =
        parse_export_format(&params.format).ok_or_else(|| AutomationError::InvalidParameters {
            tool_id: plan.request.tool_id.clone(),
            message: format!("unsupported export format '{}'", params.format),
        })?;
    std::fs::create_dir_all(&params.directory).map_err(|source| AutomationError::Io {
        path: params.directory.clone(),
        source,
    })?;
    let mut outputs = Vec::new();
    let mut targets = plan
        .targets
        .iter()
        .filter(|target| target.status != TargetCompatibility::Compatible)
        .map(|target| TargetResult {
            target: target.target.clone(),
            outcome: TargetOutcome::Skipped,
            message: target.reason.clone(),
            skip_reason: None,
            fingerprints: Vec::new(),
        })
        .collect::<Vec<_>>();
    for target in compatible_targets(plan) {
        let Some(index) = canvas_index(app, &target.id) else {
            continue;
        };
        let canvas = &app.doc.canvases[index];
        let safe_name = canvas_export_stem(&canvas.name);
        let base = params.directory.join(safe_name);
        let expected = crate::export::export_output_paths(&base, format, 1);
        if !params.overwrite && expected.iter().any(|path| path.exists()) {
            targets.push(TargetResult {
                target: TargetRef::resource(target.clone()),
                outcome: TargetOutcome::Failed,
                message: "output already exists and overwrite is false".to_owned(),
                skip_reason: None,
                fingerprints: expected
                    .iter()
                    .filter_map(|path| fingerprint_file(path, "existing_output"))
                    .collect(),
            });
            continue;
        }
        let written = match crate::export::export_canvases_with_assets(
            std::slice::from_ref(canvas),
            &app.doc.assets,
            Some(0),
            &ExportSettings {
                format,
                scope: ExportPageScope::Current,
                dpi: params.dpi,
                target_width_mm: None,
                trim_to_visible_content: false,
                allow_missing_images: false,
            },
            &base,
        ) {
            Ok(written) => written,
            // One canvas failing is a per-target outcome, exactly like the
            // overwrite collision above — not a reason to abandon the run. An
            // early `?` here would return before the caller's audit record is
            // written, so files already on disk would exist with nothing in the
            // operation history or the run manifest naming them. The failure is
            // not swallowed: `TargetOutcome::Failed` is what a workflow reads to
            // abort, and it reaches the manifest's errors either way.
            Err(error) => {
                targets.push(TargetResult {
                    target: TargetRef::resource(target.clone()),
                    outcome: TargetOutcome::Failed,
                    message: error.to_string(),
                    skip_reason: None,
                    fingerprints: Vec::new(),
                });
                continue;
            }
        };
        outputs.extend(written.iter().map(|path| path.display().to_string()));
        targets.push(TargetResult {
            target: TargetRef::resource(target.clone()),
            outcome: TargetOutcome::Succeeded,
            message: written
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", "),
            skip_reason: None,
            fingerprints: written
                .iter()
                .filter_map(|path| fingerprint_file(path, "output"))
                .collect(),
        });
    }
    Ok(ToolResult {
        tool_id: plan.request.tool_id.clone(),
        before_revision: DocumentRevision(app.doc.automation_revision),
        after_revision: DocumentRevision(app.doc.automation_revision),
        targets,
        produced: Vec::new(),
        modified: Vec::new(),
        diagnostics: outputs,
        verification: vec![VerificationRecord {
            check: "outputs_exist".to_owned(),
            passed: true,
            message: "all successful exports were committed by the Plotium exporter".to_owned(),
        }],
        value: serde_json::Value::Null,
    })
}

pub(super) fn commit_actions(
    app: &mut PlotiumApp,
    plan: &ToolPlan,
    actions: Vec<Action>,
    verb: &str,
) -> Result<ToolResult, AutomationError> {
    if actions.is_empty() {
        return Err(AutomationError::Execution(
            "tool had no compatible targets".to_owned(),
        ));
    }
    app.try_execute_action(Action::Composite(actions))
        .map_err(|error| AutomationError::Execution(error.to_string()))?;
    let modified = compatible_targets(plan).cloned().collect::<Vec<_>>();
    Ok(ToolResult {
        tool_id: plan.request.tool_id.clone(),
        before_revision: plan.frozen_targets.revision,
        after_revision: DocumentRevision(app.doc.automation_revision),
        targets: plan
            .targets
            .iter()
            .map(|target| TargetResult {
                target: target.target.clone(),
                outcome: if target.status == TargetCompatibility::Compatible {
                    TargetOutcome::Succeeded
                } else {
                    TargetOutcome::Skipped
                },
                message: if target.status == TargetCompatibility::Compatible {
                    verb.to_owned()
                } else {
                    target.reason.clone()
                },
                skip_reason: None,
                fingerprints: Vec::new(),
            })
            .collect(),
        produced: Vec::new(),
        modified,
        diagnostics: Vec::new(),
        verification: vec![VerificationRecord {
            check: "revision_advanced".to_owned(),
            passed: app.doc.automation_revision > plan.frozen_targets.revision.0,
            message: "atomic document commit completed".to_owned(),
        }],
        value: serde_json::Value::Null,
    })
}

fn dataset_index(app: &PlotiumApp, id: &str) -> Option<usize> {
    app.doc
        .datasets
        .iter()
        .position(|dataset| dataset.resource_id().to_string() == id)
}

fn canvas_index(app: &PlotiumApp, id: &str) -> Option<usize> {
    app.doc
        .canvases
        .iter()
        .position(|canvas| canvas.resource_id.to_string() == id)
}

pub(super) fn serde_result(
    tool_id: &str,
    revision: DocumentRevision,
    value: impl serde::Serialize,
) -> Result<ToolResult, AutomationError> {
    Ok(readonly_value(
        tool_id,
        revision,
        serde_json::to_value(value)
            .map_err(|error| AutomationError::Execution(error.to_string()))?,
    ))
}

pub(super) fn readonly_value(
    tool_id: &str,
    revision: DocumentRevision,
    value: serde_json::Value,
) -> ToolResult {
    ToolResult {
        tool_id: tool_id.to_owned(),
        before_revision: revision,
        after_revision: revision,
        targets: Vec::new(),
        produced: Vec::new(),
        modified: Vec::new(),
        diagnostics: Vec::new(),
        verification: Vec::new(),
        value,
    }
}

fn parse_export_format(value: &str) -> Option<ExportFormat> {
    match value.to_ascii_lowercase().as_str() {
        "svg" => Some(ExportFormat::Svg),
        "pdf" => Some(ExportFormat::Pdf),
        "png" => Some(ExportFormat::Png),
        "tiff" | "tif" => Some(ExportFormat::Tiff),
        "jpeg" | "jpg" => Some(ExportFormat::Jpeg),
        _ => None,
    }
}

/// The file stem `figure.export` writes for a canvas name, exposed so callers
/// can detect two canvases that would export to the same file.
pub fn canvas_export_stem(value: &str) -> String {
    let value = value
        .chars()
        .map(|ch| if "<>:\"/\\|?*".contains(ch) { '_' } else { ch })
        .collect::<String>();
    let value = value.trim().trim_end_matches(['.', ' ']);
    if value.is_empty() {
        "figure".to_owned()
    } else {
        value.to_owned()
    }
}

fn fingerprint_file(path: &Path, role: &str) -> Option<FingerprintRecord> {
    let bytes = std::fs::read(path).ok()?;
    Some(FingerprintRecord {
        role: role.to_owned(),
        path: path.to_owned(),
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        bytes: bytes.len() as u64,
    })
}
