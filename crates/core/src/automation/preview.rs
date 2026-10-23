//! Whole-canvas image previews from immutable native document snapshots.
use super::*;
use crate::export::{
    MissingImagePolicy, RasterLimits, RasterOptions, prepare_render_document, rasterize_svg,
};
use crate::state::{AssetId, AssetRecord, CanvasDocument, CanvasObjectKind, PlotiumApp};
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const IMAGE_PREVIEW: &str = "render.preview_image";
pub const PREVIEW_DEFAULT_EDGE: u32 = 1440;
pub const PREVIEW_MAX_EDGE: u32 = 2048;
pub const PREVIEW_MAX_PIXELS: u64 = 4_194_304;
pub const PREVIEW_ASSET_BYTES: usize = 16 * 1024 * 1024;
pub const PREVIEW_DECODED_BYTES: u64 = 64 * 1024 * 1024;
pub const PREVIEW_SVG_BYTES: usize = 8 * 1024 * 1024;
pub const PREVIEW_PNG_BYTES: usize = 4 * 1024 * 1024;
pub const PREVIEW_RESPONSE_BYTES: usize = 6 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImagePreviewParams {
    pub target: TargetRef,
    #[serde(default = "default_edge")]
    pub max_edge: u32,
}
fn default_edge() -> u32 {
    PREVIEW_DEFAULT_EDGE
}
pub fn image_preview_schema() -> Value {
    let mut target = document_inspect_schema()["properties"]["target"].clone();
    if let Some(properties) = target["properties"].as_object_mut() {
        properties.remove("component");
    }
    target["properties"]["resource"]["properties"]["kind"] = json!({"const":KIND_CANVAS});
    for key in ["parent_id", "local_id"] {
        target["properties"]["resource"]["properties"][key] = json!({"type":"null"});
    }
    json!({"type":"object","additionalProperties":false,"required":["target"],"properties":{
        "target":target,"max_edge":{"type":"integer","minimum":1,"maximum":PREVIEW_MAX_EDGE,"default":PREVIEW_DEFAULT_EDGE}}})
}
pub fn image_preview_limits() -> Value {
    json!({"default_edge":PREVIEW_DEFAULT_EDGE,"max_edge":PREVIEW_MAX_EDGE,"max_pixels":PREVIEW_MAX_PIXELS,
        "asset_bytes":PREVIEW_ASSET_BYTES,"decoded_asset_bytes":PREVIEW_DECODED_BYTES,
        "svg_bytes":PREVIEW_SVG_BYTES,"png_bytes":PREVIEW_PNG_BYTES,"response_bytes":PREVIEW_RESPONSE_BYTES,"in_flight":1})
}
fn invalid(message: &str) -> AutomationError {
    AutomationError::InvalidParameters {
        tool_id: IMAGE_PREVIEW.into(),
        message: message.into(),
    }
}
fn budget(message: &str) -> AutomationError {
    AutomationError::PreviewBudget(message.into())
}
fn check_budget(actual: u64, limit: u64, message: &str) -> Result<(), AutomationError> {
    if actual > limit {
        Err(budget(message))
    } else {
        Ok(())
    }
}
fn canvas<'a>(
    app: &'a PlotiumApp,
    params: &ImagePreviewParams,
) -> Result<&'a CanvasDocument, AutomationError> {
    if params.max_edge > PREVIEW_MAX_EDGE {
        return Err(budget(
            "max_edge exceeds 2048 px; request a smaller preview",
        ));
    }
    if params.max_edge == 0 {
        return Err(invalid("max_edge must be positive"));
    }
    app.doc
        .canvases
        .iter()
        .find(|c| TargetRef::resource(c.resource_id.into()) == params.target)
        .ok_or_else(|| invalid("target must be an existing whole canvas with its full identity"))
}
pub(super) fn freeze(
    app: &PlotiumApp,
    request: &ToolRequest,
) -> Result<FrozenTargetSet, AutomationError> {
    let params: ImagePreviewParams =
        super::registry::parse(IMAGE_PREVIEW, request.parameters.clone())?;
    canvas(app, &params)?;
    if request.targets
        != (TargetSelector::Explicit {
            ids: vec![params.target.resource.id.clone()],
        })
    {
        return Err(invalid("selector must match the explicit canvas target"));
    }
    Ok(FrozenTargetSet {
        revision: request.expected_revision,
        targets: vec![params.target.resource],
        component_targets: vec![],
        reasons: vec![],
        total_matches: 1,
        truncated: false,
    })
}

pub struct ImagePreviewSnapshot {
    canvas: CanvasDocument,
    assets: BTreeMap<AssetId, AssetRecord>,
    target: TargetRef,
    revision: DocumentRevision,
    max_edge: u32,
    candidate: Option<super::candidate_preview::CandidateIdentity>,
}
impl ImagePreviewSnapshot {
    /// Call on the desktop before releasing access to live state. Only referenced
    /// assets are copied; no project path or external image is ever resolved.
    pub fn capture(app: &PlotiumApp, plan: &ToolPlan) -> Result<Self, AutomationError> {
        if plan.request.tool_id != IMAGE_PREVIEW {
            return Err(invalid("expected an image preview plan"));
        }
        let fresh = plan_tool(app, plan.request.clone())?;
        if fresh != *plan {
            return Err(invalid("preview plan does not match current targets"));
        }
        let params: ImagePreviewParams =
            super::registry::parse(IMAGE_PREVIEW, plan.request.parameters.clone())?;
        let c = canvas(app, &params)?;
        for object in &c.objects {
            if let Some(plot) = object.plot()
                && plot.binding.coordinates.is_some()
            {
                app.build_explicit_binding_figure(
                    &plot.binding,
                    &plot.stack,
                    [
                        object.frame.width / crate::state::MM_TO_PT,
                        object.frame.height / crate::state::MM_TO_PT,
                    ],
                )
                .map_err(|e| invalid(&format!("Explicit curve unavailable: {e}")))?;
            }
        }
        Self::capture_canvas(c, &app.doc.assets, params, plan.frozen_targets.revision)
    }

    pub(crate) fn capture_canvas(
        c: &CanvasDocument,
        assets: &BTreeMap<AssetId, AssetRecord>,
        params: ImagePreviewParams,
        revision: DocumentRevision,
    ) -> Result<Self, AutomationError> {
        if params.max_edge == 0 {
            return Err(invalid("max_edge must be positive"));
        }
        check_budget(
            params.max_edge as u64,
            PREVIEW_MAX_EDGE as u64,
            "max_edge exceeds 2048 px",
        )?;
        let [w, h] = c.size_pt();
        if !w.is_finite() || !h.is_finite() || w <= 0.0 || h <= 0.0 {
            return Err(invalid("canvas dimensions must be finite and positive"));
        }
        let ids = c
            .objects
            .iter()
            .filter_map(|o| match &o.kind {
                CanvasObjectKind::RasterImage(i) => Some(i.asset),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let mut bytes = 0usize;
        for id in &ids {
            let a = assets.get(id).ok_or_else(|| {
                AutomationError::PreviewAsset(format!(
                    "asset {id} is missing; replace it before previewing"
                ))
            })?;
            bytes = bytes.saturating_add(a.bytes.len());
            check_budget(
                bytes as u64,
                PREVIEW_ASSET_BYTES as u64,
                "referenced asset bytes exceed 16 MiB",
            )?;
        }
        Ok(Self {
            canvas: {
                let mut c = c.clone();
                c.reconcile_derived_page_size();
                c
            },
            assets: ids
                .into_iter()
                .map(|id| (id, assets[&id].clone()))
                .collect(),
            target: params.target,
            revision,
            candidate: None,
            max_edge: params.max_edge,
        })
    }
    pub(crate) fn with_candidate(
        mut self,
        identity: super::candidate_preview::CandidateIdentity,
    ) -> Self {
        self.candidate = Some(identity);
        self
    }
    /// Name each thinned curve by its plot and, for bound curves, its Series.
    fn downsampled_targets(
        &self,
        records: &[plotium_render::svg::PreviewDownsample],
    ) -> Vec<Value> {
        records
            .iter()
            .filter_map(|record| {
                let object = self.canvas.objects.get(record.item)?;
                let target = TargetRef {
                    resource: canvas_object_ref(self.canvas.resource_id, object.id),
                    component: record
                        .origin
                        .map(|origin| ComponentRef::Series(crate::state::SeriesId::new(origin.0))),
                };
                Some(json!({"target":target,"source_points":record.source_points,"drawn_points":record.drawn_points}))
            })
            .collect()
    }
    pub fn render(self) -> Result<ToolResult, AutomationError> {
        self.render_cancellable(|| false)
    }
    pub fn render_cancellable(
        self,
        cancelled: impl Fn() -> bool,
    ) -> Result<ToolResult, AutomationError> {
        let check = || {
            if cancelled() {
                Err(AutomationError::PreviewCancelled)
            } else {
                Ok(())
            }
        };
        check()?;
        // Probe original headers, not persisted pixel-size hints. This happens
        // before decode and counts each distinct asset page exactly once.
        let mut pages = BTreeSet::new();
        let mut decoded = 0u64;
        for o in &self.canvas.objects {
            check()?;
            if let CanvasObjectKind::RasterImage(i) = &o.kind {
                i.validate().map_err(AutomationError::PreviewAsset)?;
                if !pages.insert((i.asset, i.page_index)) {
                    continue;
                }
                let a = &self.assets[&i.asset];
                let probe = plotium_io::image::probe(&a.bytes).map_err(|e| {
                    AutomationError::PreviewAsset(format!("asset {}: {e}", i.asset))
                })?;
                let [w, h] = if i.page_index == 0 {
                    [probe.width, probe.height]
                } else {
                    plotium_io::image::tiff_page_dimensions(&a.bytes, i.page_index).ok_or_else(
                        || {
                            AutomationError::PreviewAsset(format!(
                                "asset {} page {} is unavailable",
                                i.asset, i.page_index
                            ))
                        },
                    )?
                };
                decoded = decoded
                    .saturating_add(u64::from(w).saturating_mul(u64::from(h)).saturating_mul(4));
                check_budget(
                    decoded,
                    PREVIEW_DECODED_BYTES,
                    "decoded asset RGBA exceeds 64 MiB",
                )?;
            }
        }
        check()?;
        let document =
            prepare_render_document(&self.canvas, &self.assets, MissingImagePolicy::Block)
                .map_err(|e| AutomationError::PreviewAsset(e.to_string()))?;
        check()?;
        let [w, h] = self.canvas.size_pt();
        let preview = plotium_render::svg::export_document_preview(
            &document,
            self.max_edge as f32 / w.max(h),
        );
        let downsampled = self.downsampled_targets(&preview.downsampled);
        let svg = preview.svg;
        check_budget(
            svg.len() as u64,
            PREVIEW_SVG_BYTES as u64,
            "rendered SVG exceeds 8 MiB",
        )?;
        let mut options = RasterOptions::new(72);
        options.target_width_mm = Some(w / w.max(h) * self.max_edge as f32 * 25.4 / 72.0);
        options.limits = RasterLimits {
            max_width: PREVIEW_MAX_EDGE,
            max_height: PREVIEW_MAX_EDGE,
            max_pixels: PREVIEW_MAX_PIXELS,
            max_bytes: PREVIEW_MAX_PIXELS * 4,
        };
        check()?;
        let raster = rasterize_svg(&svg, [w, h], options)
            .map_err(|e| AutomationError::Execution(e.to_string()))?;
        check()?;
        let png = raster
            .to_png()
            .map_err(|e| AutomationError::Execution(e.to_string()))?;
        check_budget(
            png.len() as u64,
            PREVIEW_PNG_BYTES as u64,
            "PNG exceeds 4 MiB; request a smaller max_edge",
        )?;
        check()?;
        let mut value = json!({"target":self.target,"width_px":raster.width(),"height_px":raster.height(),
            "mime_type":"image/png","png_base64":base64::engine::general_purpose::STANDARD.encode(png)});
        if !downsampled.is_empty() {
            value["downsampled"] = json!(downsampled);
        }
        if let Some(identity) = self.candidate {
            value["plan_id"] = json!(identity.plan_id);
            value["basis_project_context"] = json!(identity.project_context);
            value["basis_revision"] = json!(self.revision);
            value["candidate"] = json!(true);
            value["output"] = json!({"size_mm":self.canvas.size_mm,"size_pt":self.canvas.size_pt(),"max_edge":self.max_edge});
        } else {
            value["render_revision"] = json!(self.revision);
        }
        Ok(super::tool_executors::readonly_value(
            IMAGE_PREVIEW,
            self.revision,
            value,
        ))
    }
}
#[cfg(test)]
#[path = "preview_tests.rs"]
mod tests;
