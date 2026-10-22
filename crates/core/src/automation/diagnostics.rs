//! Revision-bound, non-persistent native layout observations and scan cursors.
mod capture;
mod hints;
mod scan;
mod schema;
use super::*;
use crate::state::*;
use plotium_render::{Rect, diagnostics as geometry};
pub use schema::*;
use schema::{budget, invalid};
use serde_json::{Value, json};
use std::collections::{BTreeSet, VecDeque};

const COMPACT_EVIDENCE: &[&str] = &[
    "other_target",
    "other_component",
    "point_range",
    "interpretation",
];
struct SnapshotItem {
    target: TargetRef,
    frame: Rect,
    visible: bool,
    kind: SnapshotKind,
}
enum SnapshotKind {
    Plot {
        figure: Box<plotium_figure::Figure>,
        /// Shared-x stack geometry the renderers apply to this plot.
        stack: Option<plotium_render::StackPlacement>,
        selected_series: Option<BTreeSet<SeriesId>>,
        declared: Vec<(SeriesId, bool, String)>,
        facts: hints::PlotFacts,
    },
    Text {
        text: String,
        font: f32,
        align: plotium_render::OverlayAlign,
        part: &'static str,
    },
    Raster {
        bounds: Option<Rect>,
        clip: Option<Rect>,
        status: String,
    },
    Shape {
        bounds: Rect,
    },
}
struct CurveWork {
    target: TargetRef,
    frame: Rect,
    plot: Rect,
    legend: Option<Rect>,
    figure: std::sync::Arc<plotium_figure::Figure>,
    // These are native rendered runs, drained with their identities. No object
    // collection index becomes a job, cursor, or resource identity.
    runs: VecDeque<plotium_figure::Series>,
    current: Option<(plotium_figure::Series, geometry::CurveScan)>,
}
pub struct FigureDiagnosticScan {
    pub project_context: ProjectContextId,
    pub revision: DocumentRevision,
    params: FigureDiagnosticParams,
    page: Rect,
    scale: f32,
    items: VecDeque<SnapshotItem>,
    curves: VecDeque<CurveWork>,
    overlaps: VecDeque<(TargetRef, Rect, TargetRef, Rect)>,
    pending: VecDeque<Value>,
    scanned_objects: usize,
    scanned_points: usize,
    scanned_pairs: usize,
    unavailable: usize,
    unsupported: usize,
    snapshot_bytes: usize,
    font_status: Option<&'static str>,
    /// Page-space text boxes and plot areas gathered for the canvas-wide
    /// layout hints, which run once after every item is scanned.
    hint_texts: Vec<(TargetRef, String, Rect)>,
    hint_plots: Vec<(TargetRef, Rect, Rect)>,
    hints_done: bool,
    texts_dropped: usize,
}
impl FigureDiagnosticScan {
    pub fn release_completed_storage(&mut self) {
        if self.items.is_empty() {
            self.items.shrink_to_fit();
        }
        if self.overlaps.is_empty() {
            self.overlaps.shrink_to_fit();
        }
    }
    pub fn audit_parameters(&self) -> Value {
        json!(self.params)
    }
    pub fn canvas_target(&self) -> TargetRef {
        self.params.canvas.clone()
    }
    pub fn is_complete(&self) -> bool {
        self.items.is_empty()
            && self.curves.is_empty()
            && self.overlaps.is_empty()
            && self.pending.is_empty()
            && self.hints_done
    }
    fn checks(&self, c: DiagnosticCheck) -> bool {
        self.params.checks.contains(&c)
    }
    fn rect(&self, r: Rect) -> Value {
        let r = geometry::scale(r, self.scale);
        json!({"x":r.left,"y":r.top,"width":r.width,"height":r.height})
    }
    fn emit(&mut self, mut row: Value) {
        let compact = self.params.detail == DiagnosticDetail::Compact;
        // Compact pages keep only coverage that limits what the scan can claim.
        if compact && row["kind"] == "coverage" && row["status"] == "complete" {
            return;
        }
        // Compact issues keep the evidence that says what they refer to and
        // drop only the geometry.
        if compact && row["kind"] == "issue" {
            let evidence = row["evidence"].as_object().map(|evidence| {
                evidence
                    .iter()
                    .filter(|(key, _)| COMPACT_EVIDENCE.contains(&key.as_str()))
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect::<serde_json::Map<_, _>>()
            });
            match evidence {
                Some(evidence) if !evidence.is_empty() => row["evidence"] = Value::Object(evidence),
                _ => {
                    if let Some(row) = row.as_object_mut() {
                        row.remove("evidence");
                    }
                }
            }
        }
        let kind = row["kind"].as_str().unwrap_or("");
        if kind == "coverage"
            || self.params.view == DiagnosticView::Both
            || (kind == "issue" && self.params.view == DiagnosticView::Issues)
            || (kind == "layout" && self.params.view == DiagnosticView::Layout)
        {
            self.pending.push_back(row);
        }
    }
    fn coverage(&mut self, target: &TargetRef, check: &str, status: &str, reason: &str) {
        if status == "unsupported" {
            self.unsupported += 1;
        }
        if status == "unavailable" {
            self.unavailable += 1;
        }
        self.emit(json!({"kind":"coverage","target":target,"check":check,"status":status,"reason":reason}));
    }
    fn issue(&mut self, target: &TargetRef, part: &str, code: &str, basis: &str, evidence: Value) {
        let (severity, message) = match code {
            "out_of_canvas" => ("info", "Geometry extends beyond the output page."),
            "partially_clipped" => (
                "info",
                "The effective clip removes part of this geometry; authored limits may be intentional.",
            ),
            "fully_clipped" => (
                "info",
                "This geometry is entirely outside its effective clip.",
            ),
            "shape_clipped" => (
                "info",
                "The effective page clip removes at least part of the native shape geometry; overlap with other content is not assessed.",
            ),
            "curve_primitives_clipped" => (
                "info",
                "The effective plot/page clip removes painted primitives in the reported point range; manual ranges may be intentional.",
            ),
            "text_container_overflow" => (
                "warning",
                "Estimated text bounds exceed the authored container; the native text renderer does not wrap to fit.",
            ),
            "points_hidden_by_axis_break" => (
                "info",
                "Data points fall inside an axis break and are not drawn; check that the break hides only empty range.",
            ),
            "legend_occlusion_potential" => (
                "warning",
                "Visible curve primitives intersect the legend layout box. Its translucent background may obscure data; this is a potential visual conflict.",
            ),
            "right_axis_untitled" => (
                "warning",
                "Curves use the right y axis but it shows no title, so readers cannot tell which quantity it measures. Set object.axes.y2_label.",
            ),
            _ => (
                "info",
                "Selected content frames overlap. Intentional overlap is allowed; this is not a pixel-occlusion judgment.",
            ),
        };
        self.emit(json!({"kind":"issue","target":target,"component":part,"code":code,"severity":severity,"message":message,"measurement_basis":basis,"evidence":evidence}));
    }
}
pub(super) fn freeze(
    app: &PlotiumApp,
    request: &ToolRequest,
) -> Result<FrozenTargetSet, AutomationError> {
    let params: FigureDiagnosticParams =
        super::registry::parse(FIGURE_DIAGNOSE, request.parameters.clone())?;
    capture::validate(app, &params)?;
    if request.targets
        != (TargetSelector::Explicit {
            ids: vec![params.canvas.resource.id.clone()],
        })
    {
        return Err(invalid("Selector must match the explicit canvas."));
    }
    Ok(FrozenTargetSet {
        revision: request.expected_revision,
        targets: vec![params.canvas.resource],
        component_targets: vec![],
        reasons: vec![],
        total_matches: 1,
        truncated: false,
    })
}

#[cfg(test)]
mod tests;

pub(crate) fn candidate_figure_cost(f: &plotium_figure::Figure) -> usize {
    capture::figure_cost(f).0
}
