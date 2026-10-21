//! Borrow a frozen edit into a disposable native execution context. Never replan
//! figure edits: doing so would reread files and mint different asset identities.
use super::*;
use crate::{actions::Action, properties::guide, state::*};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub const CANDIDATE_SNAPSHOT_BYTES: usize = 64 * 1024 * 1024;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanPreviewParams {
    pub plan_id: String,
    pub expected_revision: DocumentRevision,
    #[serde(flatten)]
    pub image: ImagePreviewParams,
}
#[derive(Clone)]
pub(crate) struct CandidateIdentity {
    pub plan_id: String,
    pub project_context: ProjectContextId,
}
pub fn plan_preview_schema() -> Value {
    let mut schema = image_preview_schema();
    schema["properties"]["plan_id"] = json!({"type":"string","minLength":1});
    schema["properties"]["expected_revision"] = json!({"type":"integer","minimum":0});
    schema["required"] = json!(["plan_id", "expected_revision", "target"]);
    schema
}
pub fn plan_preview_contract() -> Value {
    json!({"properties":["properties.set","properties.reset"],"property_keys":[guide::VISIBILITY.as_str(),guide::PLACEMENT.as_str(),guide::LAYOUT.as_str(),guide::TITLE.as_str(),guide::POSITION.as_str()],
        "figure_edits":["content_frame","image_replace","text_set","content_delete","panel_delete","series_delete"],
        "batch":"Full native batch; deletion-only rules unchanged. Multi-canvas guide plans may be previewed one affected canvas at a time.",
        "rendering":"Ordinary line/scatter plots, native text, raster and shapes. No background scientific computation.",
        "snapshot_estimate_bytes":CANDIDATE_SNAPSHOT_BYTES,"snapshot_estimate":"Native canvas geometry plus source field dimensions at 64 bytes/element, metadata and frozen assets; not peak process memory.",
        "lifecycle":"Borrow only; no consumption, TTL extension or new authority. No candidate cache. Same single preview flight and image limits.",
        "audit":"Queries do not append project audit records or change dirty state or automation revision."})
}
fn invalid(message: impl Into<String>) -> AutomationError {
    AutomationError::InvalidParameters {
        tool_id: "plotium_preview_plan".into(),
        message: message.into(),
    }
}
fn unsupported(message: &str) -> AutomationError {
    AutomationError::UnknownTool(format!("candidate preview: {message}"))
}
fn budget(bytes: usize) -> Result<(), AutomationError> {
    if bytes > CANDIDATE_SNAPSHOT_BYTES {
        Err(AutomationError::PreviewBudget(
            "Candidate snapshot estimate exceeds 64 MiB".into(),
        ))
    } else {
        Ok(())
    }
}

/// Checks which *existing* plans are previewable, without changing their execute contract.
pub fn validate_plan_preview(
    app: &PlotiumApp,
    plan: &ToolPlan,
    target: &TargetRef,
) -> Result<Vec<CanvasId>, AutomationError> {
    if plan.frozen_targets.revision.0 != app.doc.automation_revision {
        return Err(AutomationError::StaleRevision {
            expected: plan.frozen_targets.revision.0,
            actual: app.doc.automation_revision,
        });
    }
    let mut canvases = BTreeSet::new();
    match plan.request.tool_id.as_str() {
        FIGURE_EDIT => {
            let params: FigureEditParams =
                registry::parse(FIGURE_EDIT, plan.request.parameters.clone())?;
            if params.edits.iter().any(|op| {
                !matches!(
                    op,
                    FigureEdit::ContentFrame { .. }
                        | FigureEdit::ImageReplace { .. }
                        | FigureEdit::TextSet { .. }
                        | FigureEdit::ContentDelete { .. }
                        | FigureEdit::PanelDelete { .. }
                        | FigureEdit::SeriesDelete { .. }
                )
            }) {
                return Err(unsupported(
                    "this figure.edit batch contains an unsupported operation",
                ));
            }
            let prepared = plan
                .figure_plan
                .as_ref()
                .ok_or_else(|| invalid("Frozen figure plan is unavailable"))?;
            app.validate_figure_edit_preview(prepared)
                .map_err(invalid)?;
            let page = app
                .doc
                .canvases
                .iter()
                .find(|c| TargetRef::resource(c.resource_id.into()) == prepared.canvas)
                .ok_or_else(|| invalid("Canvas disappeared"))?;
            canvases.insert(page.resource_id);
        }
        TOOL_SET | TOOL_RESET => {
            let key = plan
                .request
                .parameters
                .get("key")
                .and_then(Value::as_str)
                .and_then(crate::properties::definition_by_key);
            if !key.is_some_and(|k| guide::is_guide_property(k.id)) {
                return Err(unsupported(
                    "only the five native guide properties are supported",
                ));
            }
            for t in &plan.targets {
                if t.status != TargetCompatibility::Compatible {
                    return Err(invalid("All guide targets must remain compatible"));
                }
                let page = app
                    .doc
                    .canvases
                    .iter()
                    .find(|c| {
                        c.objects.iter().any(|o| {
                            TargetRef::resource(canvas_object_ref(c.resource_id, o.id)) == t.target
                        })
                    })
                    .ok_or_else(|| invalid("Guide target disappeared"))?;
                canvases.insert(page.resource_id);
            }
        }
        _ => {
            return Err(unsupported(
                "this operation is not a supported reversible local edit",
            ));
        }
    }
    if !canvases
        .iter()
        .any(|id| TargetRef::resource((*id).into()) == *target)
    {
        return Err(invalid(
            "Preview target must be a whole canvas affected by this plan",
        ));
    }
    Ok(canvases.into_iter().collect())
}

/// The source is borrowed, including runtime provenance. Only disposable native
/// state receives the shared action; live history and identity allocators are never touched.
pub fn capture_plan_preview(
    app: &PlotiumApp,
    plan: &ToolPlan,
    params: &PlanPreviewParams,
) -> Result<ImagePreviewSnapshot, AutomationError> {
    if params.expected_revision != plan.frozen_targets.revision {
        return Err(AutomationError::StaleRevision {
            expected: params.expected_revision.0,
            actual: plan.frozen_targets.revision.0,
        });
    }
    let ids = validate_plan_preview(app, plan, &params.image.target)?;
    let pages: Vec<_> = app
        .doc
        .canvases
        .iter()
        .filter(|c| ids.contains(&c.resource_id))
        .collect();
    let mut bytes = 0usize;
    let mut sources = BTreeSet::new();
    let mut assets = BTreeSet::new();
    for page in &pages {
        bytes = bytes.saturating_add(page.panels.len().saturating_mul(2048));
        for item in &page.objects {
            bytes = bytes.saturating_add(2048).saturating_add(item.name.len());
            match &item.kind {
                ContentKind::Plot(p) => {
                    if p.binding
                        .series
                        .iter()
                        .any(|s| !matches!(s.encoding, plotium_figure::SeriesEncoding::Line(_)))
                    {
                        return Err(unsupported(
                            "candidate canvas requires a non-line/scatter renderer context",
                        ));
                    }
                    bytes =
                        bytes.saturating_add(super::diagnostics::candidate_figure_cost(p.figure()));
                    sources.extend(p.binding.series.iter().map(|s| s.source.resource));
                    sources.extend(p.display_owner);
                }
                ContentKind::Text(t) => bytes = bytes.saturating_add(t.text.len()),
                ContentKind::RasterImage(i) => {
                    assets.insert(i.asset);
                }
                ContentKind::Shape(_) => {}
            }
            budget(bytes)?;
        }
    }
    for id in &sources {
        let source = app
            .doc
            .dataset_by_id(*id)
            .ok_or_else(|| invalid("Native source data is unavailable"))?;
        for field in source.field_descriptors() {
            let elements = field
                .dimensions
                .iter()
                .fold(1usize, |a, b| a.saturating_mul(*b));
            bytes = bytes
                .saturating_add(elements.saturating_mul(64))
                .saturating_add(4096);
            budget(bytes)?;
        }
    }
    for id in &assets {
        if let Some(asset) = app.doc.assets.get(id) {
            bytes = bytes.saturating_add(asset.bytes.len());
        }
    }
    // The frozen authoring payload can contain an additional canvas and assets.
    if let Some(p) = &plan.figure_plan {
        bytes = bytes.saturating_add(p.preview_payload_bytes());
    }
    budget(bytes)?;
    let mut candidate = PlotiumApp::with_compute(app.settings.clone(), ComputeService::inactive());
    candidate.doc.automation_revision = app.doc.automation_revision;
    candidate.doc.style_library = app.doc.style_library.clone();
    candidate.doc.canvases = pages.into_iter().cloned().collect();
    candidate.doc.datasets = app
        .doc
        .datasets
        .iter()
        .filter(|d| sources.contains(&d.resource_id()))
        .cloned()
        .collect();
    candidate.doc.assets = assets
        .into_iter()
        .filter_map(|id| app.doc.assets.get(&id).map(|a| (id, a.clone())))
        .collect();
    candidate.session.project_context = app.session.project_context;
    candidate.session.project_present = true;
    let active = app
        .session
        .active_canvas
        .and_then(|i| app.doc.canvases.get(i))
        .map(|c| c.resource_id);
    candidate.session.active_canvas = active.and_then(|id| candidate.doc.canvas_index(id));
    // Deletion checks are scoped to the selection effect, not unrelated canvases.
    candidate.session.ui.selection = app.session.ui.selection.clone();
    candidate.session.ui.hierarchical_selection = app.session.ui.hierarchical_selection.clone();
    candidate.session.ui.selection_anchors = app.session.ui.selection_anchors;
    candidate.session.ui.panel_label_selection =
        app.session.ui.panel_label_selection.and_then(|(ci, id)| {
            app.doc
                .canvases
                .get(ci)
                .and_then(|c| candidate.doc.canvas_index(c.resource_id))
                .map(|ci| (ci, id))
        });
    let action = if let Some(p) = &plan.figure_plan {
        // Deletion uses stable CanvasIds, including selection effects whose active
        // canvas may be outside this snapshot. Recheck in the original context;
        // only apply the resulting action to the disposable candidate.
        let context = if p.is_deletion() { app } else { &candidate };
        context.figure_edit_action(p.clone()).map_err(invalid)?
    } else {
        let targets: Vec<_> = plan.targets.iter().map(|t| t.target.clone()).collect();
        let commit = super::properties::prepare_commit(&candidate, plan, &targets)?;
        commit.document_action.unwrap_or(Action::Composite(vec![]))
    };
    candidate
        .apply_preview_action(&action)
        .map_err(|e| invalid(e.to_string()))?;
    candidate.reconcile_page_fit();
    let preview = plan_tool(
        &candidate,
        ToolRequest {
            tool_id: IMAGE_PREVIEW.into(),
            tool_version: 1,
            parameters: json!(params.image),
            targets: TargetSelector::Explicit {
                ids: vec![params.image.target.resource.id.clone()],
            },
            expected_revision: params.expected_revision,
            caller: CallerType::Human,
        },
    )?;
    ImagePreviewSnapshot::capture(&candidate, &preview).map(|s| {
        s.with_candidate(CandidateIdentity {
            plan_id: params.plan_id.clone(),
            project_context: app.session.project_context,
        })
    })
}
