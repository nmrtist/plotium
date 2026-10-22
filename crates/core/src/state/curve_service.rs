//! One semantic planner/commit boundary for desktop and automation column edits.
mod append;
use super::*;
use crate::automation::{ComponentRef, DocumentRevision, TargetRef, canvas_object_ref};
pub use append::CurveAppendParams;
use plotium_data::{ColumnId, RevisionId, TableId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TableCurveSource {
    pub dataset_id: DatasetId,
    pub field_id: FieldId,
    pub table_id: TableId,
    pub table_revision: RevisionId,
    pub x_column_id: ColumnId,
    pub y_column_id: ColumnId,
    #[serde(deserialize_with = "required_error_column")]
    pub y_error_column_id: Option<ColumnId>,
}
fn required_error_column<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<ColumnId>, D::Error> {
    Option::<ColumnId>::deserialize(d)
}
impl TableCurveSource {
    pub fn source(&self) -> SeriesSource {
        SeriesSource {
            resource: self.dataset_id,
            field: self.field_id,
            selection: SeriesSelection::TableColumns(TableCurveColumns {
                table_id: self.table_id,
                x_column_id: self.x_column_id,
                y_column_id: self.y_column_id,
                y_error_column_id: self.y_error_column_id,
            }),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurveBindingEdit {
    pub target: TargetRef,
    pub source: TableCurveSource,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializeCurveEdit {
    pub target: TargetRef,
    pub retain_y_column_id: ColumnId,
    pub table_revision: RevisionId,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CurveBindingSummary {
    pub binding_mode: String,
    pub target: TargetRef,
    pub source: TableCurveSource,
    pub diagnostics: CurveDiagnostics,
}

#[derive(Clone)]
struct CurvePlotChange {
    canvas: CanvasId,
    object: ObjectId,
    before: Box<PlotObject>,
    after: Box<PlotObject>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct CurveEditPlan {
    pub expected_revision: DocumentRevision,
    pub bindings: Vec<CurveBindingSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub appended: Vec<TargetRef>,
    #[serde(skip)]
    changes: Vec<CurvePlotChange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creation: Option<PlotCreationSummary>,
    #[serde(skip)]
    pub(crate) insertion: Option<Box<crate::actions::NativePlotInsertion>>,
}
impl CurveEditPlan {
    pub(crate) fn for_creation(
        revision: DocumentRevision,
        bindings: Vec<CurveBindingSummary>,
        creation: PlotCreationSummary,
        insertion: crate::actions::NativePlotInsertion,
    ) -> Self {
        Self {
            expected_revision: revision,
            bindings,
            appended: vec![],
            changes: vec![],
            creation: Some(creation),
            insertion: Some(Box::new(insertion)),
        }
    }
    pub(crate) fn copy_candidates_to(&self, page: &mut CanvasDocument) -> Result<(), String> {
        for change in &self.changes {
            if change.canvas != page.resource_id {
                return Err("Curve candidate is on another canvas.".into());
            }
            let plot = page
                .object_mut(change.object)
                .and_then(ContentItem::plot_mut)
                .ok_or("Curve owner disappeared.")?;
            *plot = *change.after.clone();
        }
        Ok(())
    }
    pub fn curve_rows(&self) -> usize {
        self.bindings.iter().map(|b| b.diagnostics.total_rows).sum()
    }
}
impl std::fmt::Debug for CurveEditPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CurveEditPlan")
            .field("revision", &self.expected_revision)
            .field("bindings", &self.bindings)
            .finish()
    }
}
impl PartialEq for CurveEditPlan {
    fn eq(&self, b: &Self) -> bool {
        self.expected_revision == b.expected_revision
            && self.bindings == b.bindings
            && self.creation == b.creation
            && self.appended == b.appended
    }
}

impl PlotiumApp {
    pub fn curve_target(
        &self,
        target: &TargetRef,
    ) -> Result<(CanvasId, ObjectId, SeriesId), String> {
        let Some(ComponentRef::Series(id)) = target.component else {
            return Err("Supply an exact Series target.".into());
        };
        for canvas in &self.doc.canvases {
            for object in &canvas.objects {
                if canvas_object_ref(canvas.resource_id, object.id) == target.resource {
                    let plot = object.plot().ok_or("Target is not a plot.")?;
                    if !plot.binding.series.iter().any(|s| s.id == id) {
                        return Err("SeriesId does not exist in this plot.".into());
                    }
                    return Ok((canvas.resource_id, object.id, id));
                }
            }
        }
        Err("Curve owner identity is stale or forged.".into())
    }
    fn curve_plot(&self, canvas: CanvasId, object: ObjectId) -> Result<&PlotObject, String> {
        self.doc
            .canvas_index(canvas)
            .and_then(|i| self.doc.canvases[i].object(object))
            .and_then(|o| o.plot())
            .ok_or_else(|| "Curve plot is missing.".into())
    }
    pub fn plan_curve_bindings(
        &self,
        revision: DocumentRevision,
        edits: &[CurveBindingEdit],
    ) -> Result<CurveEditPlan, String> {
        self.prepare_curve_edits(revision, edits, &[])
    }
    pub fn plan_materialize_curves(
        &self,
        revision: DocumentRevision,
        edits: &[MaterializeCurveEdit],
    ) -> Result<CurveEditPlan, String> {
        self.prepare_curve_edits(revision, &[], edits)
    }
    fn prepare_curve_edits(
        &self,
        revision: DocumentRevision,
        edits: &[CurveBindingEdit],
        materialize: &[MaterializeCurveEdit],
    ) -> Result<CurveEditPlan, String> {
        if revision.0 != self.doc.automation_revision {
            return Err("Document revision is stale.".into());
        }
        let count = edits.len() + materialize.len();
        if count == 0 || count > 200 {
            return Err("A binding batch requires 1–200 exact Series edits.".into());
        }
        let mut seen = BTreeSet::new();
        let mut changes: Vec<CurvePlotChange> = Vec::new();
        for target in edits
            .iter()
            .map(|e| &e.target)
            .chain(materialize.iter().map(|e| &e.target))
        {
            let (canvas, object, series) = self.curve_target(target)?;
            if !seen.insert((canvas, object, series)) {
                return Err("Duplicate Series target in binding batch.".into());
            }
            if !changes
                .iter()
                .any(|p| p.canvas == canvas && p.object == object)
            {
                let before = self.curve_plot(canvas, object)?.clone();
                changes.push(CurvePlotChange {
                    canvas,
                    object,
                    before: Box::new(before.clone()),
                    after: Box::new(before),
                });
            }
        }
        for edit in edits {
            let (canvas, object, id) = self.curve_target(&edit.target)?;
            let change = changes
                .iter_mut()
                .find(|p| p.canvas == canvas && p.object == object)
                .expect("target was collected");
            let series = change
                .after
                .binding
                .series
                .iter_mut()
                .find(|s| s.id == id)
                .expect("target was checked");
            if !matches!(series.encoding, plotium_figure::SeriesEncoding::Line(_)) {
                return Err("Rebinding requires a numeric curve encoding.".into());
            }
            if series.source.columns().is_none() {
                let (_, columns) = self.table_curve_sources(series.source)?;
                if columns.len() > 1 {
                    return Err(
                        "Materialize this multi-curve default Series before rebinding one curve."
                            .into(),
                    );
                }
                if !columns.is_empty()
                    && let plotium_figure::SeriesEncoding::Line(line) = &mut series.encoding
                {
                    line.kind = plotium_figure::SeriesKind::Points;
                }
            }
            let source = edit.source.source();
            let (table, _) = self.table_curve_sources(source)?;
            if table.typed_state.envelope.revision.id != edit.source.table_revision {
                return Err("Table revision is stale.".into());
            }
            series.source = source;
        }
        for edit in materialize {
            let (canvas, object, id) = self.curve_target(&edit.target)?;
            let change = changes
                .iter_mut()
                .find(|p| p.canvas == canvas && p.object == object)
                .expect("target collected");
            let pos = change
                .after
                .binding
                .series
                .iter()
                .position(|s| s.id == id)
                .expect("target checked");
            let original = change.after.binding.series[pos].clone();
            if !matches!(original.source.selection, SeriesSelection::FieldDefault) {
                return Err("Only a default table field can be materialized.".into());
            }
            if !matches!(original.encoding, plotium_figure::SeriesEncoding::Line(_)) {
                return Err("Materialization requires a numeric curve encoding.".into());
            }
            let (table, columns) = self.table_curve_sources(original.source)?;
            if table.typed_state.envelope.revision.id != edit.table_revision {
                return Err("Table revision is stale.".into());
            }
            if columns.len() > 1
                && (change.after.stack.normalize
                    || !matches!(change.after.stack.mode, StackMode::Superimposed))
            {
                return Err(
                    "Use an unnormalized overlay before expanding a multi-curve default field."
                        .into(),
                );
            }
            if !columns
                .iter()
                .any(|c| c.y_column_id == edit.retain_y_column_id)
            {
                return Err("retain_y_column_id is not a default Y column.".into());
            }
            let unique = columns
                .iter()
                .map(|c| c.y_column_id)
                .collect::<BTreeSet<_>>();
            if unique.len() != columns.len() {
                return Err("Default Y columns are ambiguous.".into());
            }
            let mut expanded = Vec::new();
            for (index, column) in columns.iter().copied().enumerate() {
                let mut series = original.clone();
                series.id = if column.y_column_id == edit.retain_y_column_id {
                    id
                } else {
                    let next = change.after.next_series_id;
                    change.after.next_series_id = next
                        .try_advance(1)
                        .ok_or("Series identity allocator is exhausted.")?;
                    next
                };
                series.source.selection = SeriesSelection::TableColumns(column);
                if let plotium_figure::SeriesEncoding::Line(line) = &mut series.encoding {
                    line.kind = plotium_figure::SeriesKind::Points;
                    if columns.len() > 1 {
                        line.color = plotium_figure::ColorSource::Explicit(
                            table.default_curve_color(index, column.y_column_id),
                        );
                        // An automatic colour is not the user's choice.
                        line.color_locked = false;
                    }
                }
                expanded.push(series);
            }
            change.after.binding.series.splice(pos..=pos, expanded);
        }
        self.check_curve_row_budget(changes.iter().map(|c| &c.after.binding))?;
        let mut bindings = Vec::new();
        for change in &mut changes {
            self.finish_curve_candidate(change)?;
            bindings.extend(self.curve_binding_summaries(
                change.canvas,
                change.object,
                &change.after.binding,
            )?);
        }
        Ok(CurveEditPlan {
            expected_revision: revision,
            bindings,
            appended: vec![],
            changes,
            creation: None,
            insertion: None,
        })
    }
    fn finish_curve_candidate(&self, change: &mut CurvePlotChange) -> Result<(), String> {
        let before = &change.before;
        if !matches!(before.chart.type_id.as_str(), "" | "table_line") {
            return Err("Choose the numeric Line chart before binding columns.".into());
        }
        if change.after.binding.coordinates.is_none() {
            let manual =
                before.axis_overrides.x_range.is_some() || before.axis_overrides.y_range.is_some();
            let basis = if manual {
                &before.binding
            } else {
                &change.after.binding
            };
            let mut units = None;
            for series in &basis.series {
                let (table, columns) = self.table_curve_sources(series.source)?;
                for columns in columns {
                    let current = table_curve_coordinates(table, columns)?;
                    if manual && units.as_ref().is_some_and(|u| u != &current) {
                        return Err(
                            "Existing manual ranges have no unique table coordinate unit contract."
                                .into(),
                        );
                    }
                    if units.is_none() {
                        units = Some(current);
                    }
                }
            }
            change.after.binding.coordinates =
                Some(units.ok_or("No curve coordinates can be established.")?);
        }
        let ci = self
            .doc
            .canvas_index(change.canvas)
            .ok_or("Canvas is missing.")?;
        let frame = self.doc.canvases[ci]
            .object(change.object)
            .ok_or("Plot is missing.")?
            .frame;
        change.after.display_owner = None;
        let active = before
            .stack
            .active
            .and_then(|i| before.binding.series.get(i))
            .map(|s| s.id);
        change.after.stack.active =
            active.and_then(|id| change.after.binding.series.iter().position(|s| s.id == id));
        let figure = self.build_explicit_binding_figure(
            &change.after.binding,
            &change.after.stack,
            [frame.width / MM_TO_PT, frame.height / MM_TO_PT],
        )?;
        super::axis_display::validate_display(
            &change.after.axis_overrides,
            change.after.binding.coordinates.as_ref(),
            &figure,
        )?;
        change.after.preserve_viewport_on_rebuild(figure);
        Ok(())
    }
    /// Reorder/remove existing explicit Series without changing sources or coordinates.
    pub fn plan_curve_series_order(
        &self,
        revision: DocumentRevision,
        canvas: CanvasId,
        object: ObjectId,
        after: DataBinding,
    ) -> Result<CurveEditPlan, String> {
        if revision.0 != self.doc.automation_revision {
            return Err("Document revision is stale.".into());
        }
        let before = self.curve_plot(canvas, object)?.clone();
        if before.binding.coordinates.is_none()
            || before.binding.coordinates != after.coordinates
            || after.series.is_empty()
        {
            return Err(
                "Keep the explicit plot coordinate contract and at least one curve.".into(),
            );
        }
        let mut seen = BTreeSet::new();
        for series in &after.series {
            if !seen.insert(series.id) || !before.binding.series.iter().any(|s| s == series) {
                return Err(
                    "Series-order edits can only move or remove unchanged existing curves.".into(),
                );
            }
        }
        let mut candidate = before.clone();
        candidate.binding = after;
        let mut change = CurvePlotChange {
            canvas,
            object,
            before: Box::new(before),
            after: Box::new(candidate),
        };
        self.finish_curve_candidate(&mut change)?;
        Ok(CurveEditPlan {
            expected_revision: revision,
            bindings: Vec::new(),
            appended: vec![],
            changes: vec![change],
            creation: None,
            insertion: None,
        })
    }
    pub fn commit_curve_bindings(&mut self, plan: CurveEditPlan) -> Result<(), String> {
        if plan.changes.is_empty() && plan.insertion.is_none() {
            return Err("Prepare a new native binding plan before execution.".into());
        }
        if plan.expected_revision.0 != self.doc.automation_revision {
            return Err("Document revision is stale.".into());
        }
        for summary in &plan.bindings {
            self.check_table_curve_revision(&summary.source)?;
        }
        if let Some(insertion) = plan.insertion {
            return self
                .try_execute_action(crate::actions::Action::CreatePlot { insertion })
                .map_err(|e| e.to_string());
        }
        for change in &plan.changes {
            let current = self.curve_plot(change.canvas, change.object)?;
            if current.binding != change.before.binding
                || current.viewport != change.before.viewport
                || current.next_series_id != change.before.next_series_id
                || current.axis_overrides != change.before.axis_overrides
                || current.chart != change.before.chart
                || current.stack != change.before.stack
                || current.projections != change.before.projections
                || current.display_owner != change.before.display_owner
            {
                return Err("Plot changed after binding preflight.".into());
            }
        }
        self.try_execute_action(crate::actions::Action::Composite(
            plan.changes
                .into_iter()
                .map(|c| crate::actions::Action::SetCurveBinding {
                    canvas: c.canvas,
                    object: c.object,
                    before: c.before,
                    after: c.after,
                })
                .collect(),
        ))
        .map_err(|e| e.to_string())
    }
    pub(crate) fn restore_curve_binding(
        &mut self,
        canvas: CanvasId,
        object: ObjectId,
        state: &PlotObject,
    ) {
        if let Some(plot) = self
            .doc
            .canvas_index(canvas)
            .and_then(|ci| self.doc.canvases[ci].object_mut(object))
            .and_then(|o| o.plot_mut())
        {
            let high_water = plot.next_series_id.max(state.next_series_id);
            *plot = state.clone();
            plot.next_series_id = high_water;
        }
    }
}
