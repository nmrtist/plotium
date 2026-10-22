//! Native field and immutable diffusion-fit presentation, without recomputation.
use super::*;
use crate::{actions::Action, state::*};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const FIELD_PLOT_CREATE: &str = "plot.field.create";
pub const DIFFUSION_PLOT_CREATE: &str = "analysis.diffusion.plot";
pub fn is_scientific_plot_tool(id: &str) -> bool {
    matches!(id, FIELD_PLOT_CREATE | DIFFUSION_PLOT_CREATE)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScientificPlotParams {
    pub source: ResourceRef,
    pub name: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct ScientificPlotPlan {
    pub summary: Value,
    pub retained_elements: usize,
    #[serde(skip)]
    action: Option<Box<Action>>,
}
impl std::fmt::Debug for ScientificPlotPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.summary.fmt(f)
    }
}
impl PartialEq for ScientificPlotPlan {
    fn eq(&self, other: &Self) -> bool {
        self.summary == other.summary && self.retained_elements == other.retained_elements
    }
}
fn invalid(tool: &str, message: impl Into<String>) -> AutomationError {
    AutomationError::InvalidParameters {
        tool_id: tool.into(),
        message: message.into(),
    }
}
pub(super) fn freeze(request: &ToolRequest) -> Result<FrozenTargetSet, AutomationError> {
    let p: ScientificPlotParams = registry::parse(&request.tool_id, request.parameters.clone())?;
    Ok(FrozenTargetSet {
        revision: request.expected_revision,
        targets: vec![p.source],
        component_targets: vec![],
        total_matches: 1,
        reasons: vec![],
        truncated: false,
    })
}
pub(super) fn prepare(
    app: &PlotiumApp,
    request: &ToolRequest,
) -> Result<ScientificPlotPlan, AutomationError> {
    let tool = request.tool_id.as_str();
    let err = |s: &str| invalid(tool, s);
    if request.targets != (TargetSelector::Explicit { ids: vec![] }) {
        return Err(err(
            "Use source inside parameters; omit top-level selectors.",
        ));
    }
    let p: ScientificPlotParams = registry::parse(tool, request.parameters.clone())?;
    if p.name.trim().is_empty() || p.name.len() > 1024 {
        return Err(err("Specify a nonblank name of at most 1024 UTF-8 bytes."));
    }
    let descriptor = ProjectResourceProvider::new(app)
        .inspect(&p.source.id)
        .filter(|d| d.resource == p.source)
        .ok_or_else(|| err("Source identity is missing or forged."))?;
    let source = app
        .doc
        .datasets
        .iter()
        .find(|d| {
            d.resource_id().to_string() == p.source.parent_id.as_deref().unwrap_or(&p.source.id)
        })
        .ok_or_else(|| err("Source dataset is missing."))?;
    let mut worker = PlotiumApp::with_compute(Default::default(), ComputeService::inactive());
    worker.doc.style_library = app.doc.style_library.clone();
    let (page, table, evidence, retained_elements) = if tool == FIELD_PLOT_CREATE {
        if p.source.kind.0 != KIND_FIELD {
            return Err(err("Select one native field."));
        }
        let field = source
            .field_descriptors()
            .into_iter()
            .find(|f| Some(&f.local_id) == p.source.local_id.as_ref())
            .ok_or_else(|| err("Field is unavailable."))?;
        let elements = field
            .dimensions
            .iter()
            .try_fold(1usize, |n, d| n.checked_mul(*d))
            .unwrap_or(usize::MAX);
        // A default 128-row ILT of a 20,480-point spectrum has 2,621,440
        // cells. Bound preparation work separately from retained plot geometry.
        if elements > 4_000_000 {
            return Err(err(
                "Native plot preparation exceeds 4,000,000 field elements.",
            ));
        }
        let evidence = if descriptor
            .capabilities
            .contains(&CapabilityId::new(CAP_DIFFUSION_RESULT))
        {
            if field.dimensions.contains(&0) {
                return Err(err(
                    "The DOSY result is missing or has been invalidated by an input or processing change. Recompute the selected DOSY method before plotting.",
                ));
            }
            let plan = result_plan(request, &p.source);
            let report = super::dosy::inspect(app, &plan)?.value;
            if report["valid"] != true || report["status"] != "available" {
                return Err(err(
                    "The DOSY result is stale or empty. Recompute the selected DOSY method before plotting.",
                ));
            }
            report
        } else {
            json!({"field":p.source,"units":field.units})
        };
        let mut series = SeriesBinding::from_field_all(source, field.id);
        for (index, item) in series.iter_mut().enumerate() {
            item.id = SeriesId::new(index as u64);
        }
        if series.is_empty()
            || series.len() > 200
            || series
                .iter()
                .any(|s| !source.supports_encoding(field.id, &s.encoding))
        {
            return Err(err(
                "Field has no supported native encoding or exceeds 200 series.",
            ));
        }
        worker.doc.datasets.push(source.clone());
        for item in &series {
            worker
                .session
                .compute
                .prepare_frozen_contour(source, item)
                .map_err(|e| invalid(tool, e))?;
        }
        let mut page = CanvasDocument::new(p.name.clone(), DEFAULT_CANVAS_SIZE_MM);
        let [w, h] = page.size_pt();
        let id = page.allocate_object_id();
        let mut object = crate::workflow::build_plot_object(
            source,
            0,
            ObjectFrame::new(0., 0., w, h),
            id,
            p.name.clone(),
        );
        let plot = object.plot_mut().expect("native plot constructor");
        plot.next_series_id = SeriesId::new(series.len() as u64);
        plot.binding = DataBinding {
            coordinates: None,
            series,
        };
        // A field plot is pinned to this field, independent of the owner's display mode.
        plot.display_owner = None;
        let figure = worker.build_binding_figure(
            &plot.binding,
            &plot.chart,
            &plot.stack,
            DEFAULT_CANVAS_SIZE_MM,
        );
        // The prepared action retains the native figure, not the source grid.
        let retained_elements = figure.series.iter().map(|s| s.points.len()).sum::<usize>()
            + figure
                .contours
                .iter()
                .map(|c| c.segments.len() * 2)
                .sum::<usize>()
            + figure.heatmap.as_ref().map_or(0, |h| h.values.len());
        plot.adopt_rebuilt_figure(figure);
        page.objects.push(object);
        page.create_panel_for_plot(id)
            .ok_or_else(|| err("Could not create native plot panel."))?;
        (page, None, evidence, retained_elements)
    } else {
        if p.source.kind.0 != KIND_ANALYSIS {
            return Err(err("Select a diffusion-fit analysis resource."));
        }
        let report = super::dosy::inspect(app, &result_plan(request, &p.source))?.value;
        if report["valid"] != true {
            return Err(err("The fit selection is stale."));
        }
        let table = source
            .as_table()
            .ok_or_else(|| err("Fit source is not a table."))?;
        let id = p
            .source
            .local_id
            .as_deref()
            .and_then(|s| s.strip_prefix("diffusion_fit:"))
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or_else(|| err("Invalid diffusion analysis identity."))?;
        let analysis = table
            .curve_fit_analyses
            .iter()
            .find(|a| a.id == id)
            .ok_or_else(|| err("Fit analysis is missing."))?;
        if analysis.result.datasets.len() != 1 {
            return Err(err("Select a fit with exactly one response dataset."));
        }
        // Check the selected attempt as well as the stored outer status before
        // presenting persisted numerical estimates as a successful fit.
        let selected_attempt = analysis
            .result
            .attempts
            .iter()
            .filter(|a| a.cost.is_finite())
            .reduce(|best, candidate| {
                if candidate.cost < best.cost {
                    candidate
                } else {
                    best
                }
            });
        if !analysis.result.converged || !selected_attempt.is_some_and(|a| a.converged) {
            return Err(err(
                "The selected fit attempt did not converge. Inspect its termination diagnostics and refit before plotting.",
            ));
        }
        let mut rows = Vec::new();
        for point in &analysis.result.points {
            let input = analysis
                .result
                .datasets
                .iter()
                .find(|d| d.id == point.dataset_id)
                .and_then(|d| d.inputs.get("b"))
                .and_then(|b| b.get(point.row))
                .copied()
                .ok_or_else(|| err("Stored fit point has no matching b value."))?;
            rows.push((
                input,
                Some(point.observed),
                Some(point.predicted),
                Some(point.residual),
            ));
        }
        if analysis
            .result
            .points
            .iter()
            .any(|p| !p.observed.is_finite() || !p.predicted.is_finite() || !p.residual.is_finite())
        {
            return Err(err("Fit contains non-finite observations or residuals."));
        }
        if analysis.plot_samples.is_empty() {
            return Err(err("Fit has no stored overlay samples."));
        }
        for samples in analysis
            .plot_samples
            .values()
            .flat_map(|responses| responses.values())
        {
            for &[x, y] in samples {
                rows.push((x, None, Some(y), None));
            }
        }
        if rows.is_empty()
            || rows.len() > 20_000
            || rows
                .iter()
                .any(|r| !r.0.is_finite() || r.2.is_some_and(|y| !y.is_finite()))
        {
            return Err(err("Fit plot requires 1..=20000 finite input rows."));
        }
        rows.sort_by(|a, b| a.0.total_cmp(&b.0));
        let retained_elements = rows.len() * 4;
        let series = ["Observed intensity", "Fitted intensity", "Residual"]
            .into_iter()
            .enumerate()
            .map(|(i, name)| FloatSeries {
                name: name.into(),
                unit: String::new(),
                values: rows
                    .iter()
                    .map(|r| match i {
                        0 => r.1,
                        1 => r.2,
                        _ => r.3,
                    })
                    .collect(),
                uncertainty: None,
                fit: None,
            })
            .collect();
        let mut derived = materialized_float_series_table(
            (
                "b".into(),
                "s/m²".into(),
                rows.iter().map(|r| Some(r.0)).collect(),
            ),
            series,
            "plotium.diffusion-plot.v1",
        )
        .map_err(|e| invalid(tool, e.to_string()))?;
        derived.name = Some(p.name.clone());
        derived.meta.diffusion_input = table.meta.diffusion_input.clone();
        derived.lineage = Some(DatasetLineage::new(
            DerivationKind::DiffusionFitTable,
            [table.resource_id],
        ));
        worker
            .doc
            .datasets
            .push(Dataset::Table(Box::new(derived.clone())));
        let field_id = worker.doc.datasets[0]
            .default_field_id()
            .ok_or_else(|| err("Derived table field is missing."))?;
        let revision = &derived.typed_state.envelope.revision;
        let curve = |index: usize, kind| NewCurve {
            kind,
            source: TableCurveSource {
                dataset_id: derived.resource_id,
                field_id,
                table_id: revision.table_id,
                table_revision: revision.id,
                x_column_id: derived.x_binding.expect("materialized x"),
                y_column_id: derived.series_bindings[index].value_column,
                y_error_column_id: None,
            },
            y_axis: plotium_figure::YAxisSide::Left,
        };
        let plan = worker
            .plan_plot_creation(
                DocumentRevision(0),
                &PlotCreateParams {
                    parent: document_root_target(),
                    name: p.name.clone(),
                    canvas: Some(NewPlotCanvas {
                        name: p.name.clone(),
                        size_mm: Some([180., 150.]),
                        size_preset: None,
                    }),
                    panel_name: None,
                    panel_frame_pt: Some([36., 50., 438., 235.]),
                    series: vec![curve(0, CurveKind::Scatter), curve(1, CurveKind::Line)],
                },
            )
            .map_err(|e| invalid(tool, e))?;
        worker
            .commit_curve_bindings(plan)
            .map_err(|e| invalid(tool, e))?;
        let canvas = TargetRef::resource(worker.doc.canvases[0].resource_id.into());
        let plan = worker
            .plan_plot_creation(
                DocumentRevision(worker.doc.automation_revision),
                &PlotCreateParams {
                    parent: canvas.clone(),
                    name: "Residuals".into(),
                    canvas: None,
                    panel_name: None,
                    panel_frame_pt: Some([36., 305., 438., 90.]),
                    series: vec![curve(2, CurveKind::Scatter)],
                },
            )
            .map_err(|e| invalid(tool, e))?;
        worker
            .commit_curve_bindings(plan)
            .map_err(|e| invalid(tool, e))?;
        // Keep panel letters clear of the vertical axis title even in the
        // short residual panel, whose title can extend to its upper edge.
        for panel in &mut worker.doc.canvases[0].panels {
            panel.label.position = [panel.frame.width - 14., 5.];
        }
        let d = analysis
            .result
            .parameters
            .iter()
            .find(|v| v.parameter == "D")
            .ok_or_else(|| err("Fit has no D estimate."))?;
        if !d.value.is_finite() || d.value <= 0. {
            return Err(err("Fit has no finite positive D estimate."));
        }
        let uncertainty = if d.standard_error.is_finite() && d.standard_error >= 0. {
            format!("± {:.3e}", d.standard_error)
        } else {
            "(uncertainty unavailable)".into()
        };
        let caption = format!(
            "D = {:.3e} {uncertainty} m²/s; R² = {:.5}; n = {}",
            d.value, analysis.result.statistics.r_squared, analysis.result.statistics.observations
        );
        let plan = worker
            .plan_figure_edit(
                DocumentRevision(worker.doc.automation_revision),
                &FigureEditParams {
                    canvas,
                    edits: vec![FigureEdit::TextCreate {
                        parent: TargetRef::resource(worker.doc.canvases[0].resource_id.into()),
                        name: "Diffusion fit summary".into(),
                        frame_pt: [36., 12., 438., 30.],
                        text: caption,
                        font_pt: 10.,
                    }],
                },
            )
            .map_err(|e| invalid(tool, e))?;
        worker
            .commit_figure_edit(plan)
            .map_err(|e| invalid(tool, e))?;
        // Markers must remain visible when an excellent fit lies on top of
        // every observation. These are persisted native presentation defaults.
        for object in &mut worker.doc.canvases[0].objects {
            if let Some(plot) = object.plot_mut() {
                // The viewport holds the data-derived window, without legend room.
                let (x, y) = (plot.viewport.full_x, plot.viewport.full_y);
                plot.axis_overrides.x_range = Some(AxisRange::new(
                    x.min - x.span() * 0.05,
                    x.max + x.span() * 0.05,
                ));
                // The short residual panel needs room for the full marker envelope,
                // including observations whose residual is exactly zero.
                plot.axis_overrides.y_range = Some(AxisRange::new(
                    y.min - y.span() * 0.2,
                    y.max + y.span() * 0.2,
                ));
                for binding in &mut plot.binding.series {
                    if let plotium_figure::SeriesEncoding::Line(line) = &mut binding.encoding {
                        line.width =
                            plotium_figure::PositiveFiniteF32::new(0.8).expect("positive literal");
                        if line.kind == plotium_figure::SeriesKind::Points {
                            line.appearance.marker_size =
                                plotium_figure::PositiveFiniteF32::new(4.);
                        } else {
                            line.color = plotium_figure::ColorSource::Explicit(
                                plotium_figure::Color::rgb(209, 80, 30),
                            );
                        }
                    }
                }
            }
        }
        worker.rebuild_canvases_for(0);
        for object in &mut worker.doc.canvases[0].objects {
            if let Some(plot) = object.plot_mut() {
                plot.adopt_rebuilt_figure(plot.figure().clone());
            }
        }
        (
            worker.doc.canvases.remove(0),
            Some(derived),
            report,
            retained_elements,
        )
    };
    page.validate_structure().map_err(|e| invalid(tool, e))?;
    let canvas = TargetRef::resource(page.resource_id.into());
    let mut produced = vec![canvas.resource.clone()];
    let action = if let Some(table) = table {
        produced.push(ResourceRef::from(table.resource_id));
        let mut action = Action::insert_dataset_with_default_canvas(
            app,
            Dataset::Table(Box::new(table)),
            p.name,
            page.size_mm,
        );
        if let Action::InsertDatasetWithCanvas {
            prepared_canvas,
            canvas_resource_id,
            ..
        } = &mut action
        {
            *canvas_resource_id = page.resource_id;
            *prepared_canvas = Some(Box::new(page));
        }
        action
    } else {
        Action::insert_canvas(app.doc.canvases.len(), page, app.session.active_canvas)
    };
    Ok(ScientificPlotPlan {
        summary: json!({"canvas":canvas,"source":p.source,"produced":produced,"evidence":evidence}),
        action: Some(Box::new(action)),
        retained_elements,
    })
}
// Inspection reuses the exact result-validity rules and provenance report.
fn result_plan(request: &ToolRequest, source: &ResourceRef) -> ToolPlan {
    ToolPlan {
        request: ToolRequest {
            tool_id: ANALYSIS_RESULTS_INSPECT.into(),
            parameters: json!({}),
            ..request.clone()
        },
        frozen_targets: FrozenTargetSet {
            revision: request.expected_revision,
            targets: vec![source.clone()],
            component_targets: vec![],
            total_matches: 1,
            reasons: vec![],
            truncated: false,
        },
        desktop_region: None,
        task_id: None,
        submitted_plan_id: None,
        scientific_input: None,
        scientific_ilt_params: None,
        processing_edit: None,
        scientific_preparation: None,
        frozen_scheme: None,
        figure_plan: None,
        binding_plan: None,
        scientific_plot: None,
        targets: vec![],
        estimated_changes: vec![],
        outputs: vec![],
        required_authority: ExecutionAuthority::Read,
    }
}
pub(super) fn execute(
    app: &mut PlotiumApp,
    plan: &ToolPlan,
) -> Result<ToolResult, AutomationError> {
    let prepared = plan
        .scientific_plot
        .as_ref()
        .ok_or_else(|| invalid(&plan.request.tool_id, "Native plot preflight is missing."))?;
    let action = prepared.action.as_ref().ok_or_else(|| {
        invalid(
            &plan.request.tool_id,
            "Prepare a fresh in-process plot plan.",
        )
    })?;
    let produced: Vec<ResourceRef> = serde_json::from_value(prepared.summary["produced"].clone())
        .map_err(|e| AutomationError::Execution(e.to_string()))?;
    app.try_execute_action(action.as_ref().clone())
        .map_err(|e| AutomationError::Execution(e.to_string()))?;
    let mut result = super::tool_executors::readonly_value(
        &plan.request.tool_id,
        plan.frozen_targets.revision,
        prepared.summary.clone(),
    );
    result.after_revision = DocumentRevision(app.doc.automation_revision);
    result.targets = produced
        .iter()
        .map(|r| TargetResult {
            target: TargetRef::resource(r.clone()),
            outcome: TargetOutcome::Succeeded,
            message: "Created native scientific plot content.".into(),
            skip_reason: None,
            fingerprints: vec![],
        })
        .collect();
    result.produced = produced;
    Ok(result)
}
pub fn scientific_plot_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["source","name"],"properties":{"source":document_inspect_schema()["properties"]["target"]["properties"]["resource"],"name":{"type":"string","minLength":1,"maxLength":1024}}})
}
pub(super) fn descriptors() -> Vec<ToolDescriptor> {
    [FIELD_PLOT_CREATE,DIFFUSION_PLOT_CREATE].into_iter().map(|id|ToolDescriptor {id:id.into(),version:1,title:id.into(),description:"Create a native field plot or frozen diffusion fit with residual panel and D uncertainty; no scientific recomputation. Returns canvas for diagnosis and PNG preview.".into(),
        parameter_schema:scientific_plot_schema(),
        result_schema:json!({"type":"object","required":["value","produced"]}),target_kinds:vec![ResourceKindId::new(if id==FIELD_PLOT_CREATE {KIND_FIELD}else{KIND_ANALYSIS})],required_capabilities:vec![],effect:EffectLevel::Reversible,undoable:true,deterministic:true,task_kind:"synchronous".into()}).collect()
}
