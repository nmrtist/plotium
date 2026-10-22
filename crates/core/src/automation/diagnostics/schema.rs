use super::*;
use serde::{Deserialize, Serialize};
pub const FIGURE_DIAGNOSE: &str = "figure.diagnose";
pub const DIAGNOSTIC_SNAPSHOT_BYTES: usize = 64 * 1024 * 1024;
pub const DIAGNOSTIC_POINTS: usize = 1_000_000;
pub const DIAGNOSTIC_OBJECTS: usize = 200;
pub const DIAGNOSTIC_TEXT_BYTES: usize = 1024 * 1024;
pub const DIAGNOSTIC_PAGE_BYTES: usize = 96 * 1024;
pub const DIAGNOSTIC_TTL_SECS: u64 = 300;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCheck {
    Bounds,
    Clipping,
    TextOverflow,
    ContentOverlap,
    LegendOcclusion,
    DataShape,
    LayoutAesthetics,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticView {
    Layout,
    Issues,
    #[default]
    Both,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticOutput {
    pub size_mm: [f32; 2],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FigureDiagnosticParams {
    pub canvas: TargetRef,
    pub output: DiagnosticOutput,
    #[serde(default = "all_checks")]
    pub checks: Vec<DiagnosticCheck>,
    pub targets: Option<Vec<TargetRef>>,
    #[serde(default)]
    pub overlap_targets: Vec<TargetRef>,
    #[serde(default)]
    pub view: DiagnosticView,
    #[serde(default = "page_default")]
    pub limit: usize,
}
pub fn page_default() -> usize {
    100
}
pub fn all_checks() -> Vec<DiagnosticCheck> {
    vec![
        DiagnosticCheck::Bounds,
        DiagnosticCheck::Clipping,
        DiagnosticCheck::TextOverflow,
        DiagnosticCheck::ContentOverlap,
        DiagnosticCheck::LegendOcclusion,
        DiagnosticCheck::DataShape,
        DiagnosticCheck::LayoutAesthetics,
    ]
}
pub fn figure_diagnostic_limits() -> Value {
    json!({"rendered_runs_and_polygons_per_plot":4096,"targets":200,"overlap_targets":200,"overlap_pairs":19900,"objects_per_batch":DIAGNOSTIC_OBJECTS,"points_per_batch":DIAGNOSTIC_POINTS,"text_utf8_bytes":DIAGNOSTIC_TEXT_BYTES,"snapshot_bytes":DIAGNOSTIC_SNAPSHOT_BYTES,"asset_probe_bytes":super::super::PREVIEW_ASSET_BYTES,"page_default":100,"page_max":200,"page_value_bytes":DIAGNOSTIC_PAGE_BYTES,"response_bytes":super::super::DOCUMENT_RESPONSE_BYTES,"in_flight":1,"retained_scans":1,"idle_ttl_seconds":DIAGNOSTIC_TTL_SECS,"sampling":false})
}
pub fn figure_diagnostic_schema() -> Value {
    let target = super::super::document_inspect_schema()["properties"]["target"].clone();
    json!({"type":"object","additionalProperties":false,"required":["canvas","output"],"properties":{
        "canvas":super::super::image_preview_schema()["properties"]["target"],
        "output":{"type":"object","additionalProperties":false,"required":["size_mm"],"properties":{"size_mm":{"type":"array","minItems":2,"maxItems":2,"items":{"type":"number","exclusiveMinimum":0},"description":"Physical output size, preserving the canvas aspect ratio; scales the native page without reflow."}}},
        "checks":{"type":"array","minItems":1,"maxItems":7,"uniqueItems":true,"items":{"enum":["bounds","clipping","text_overflow","content_overlap","legend_occlusion","data_shape","layout_aesthetics"]},"description":"data_shape and layout_aesthetics report severity hint issues: the figure could be clearer, nothing is wrong. Each carries suggestions and never edits the document."},
        "targets":{"type":"array","minItems":1,"maxItems":200,"items":target,"description":"Exact content or Series targets; Series queries include owning plot frame/guide context. Omit to scan the entire canvas; never uses selection."},
        "overlap_targets":{"type":"array","maxItems":200,"items":target,"description":"Explicit whole-content targets for frame overlap only."},
        "view":{"enum":["layout","issues","both"],"default":"both"},"limit":{"type":"integer","minimum":1,"maximum":200,"default":100}
    }})
}
pub(super) fn invalid(message: impl Into<String>) -> AutomationError {
    AutomationError::InvalidParameters {
        tool_id: FIGURE_DIAGNOSE.into(),
        message: message.into(),
    }
}
pub(super) fn budget(message: impl Into<String>) -> AutomationError {
    AutomationError::DiagnosticBudget(message.into())
}
