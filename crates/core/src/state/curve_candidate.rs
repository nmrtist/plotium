//! Shared validation and summaries for detached creations and existing curve edits.
use super::*;
use crate::automation::{ComponentRef, TargetRef, canvas_object_ref};

impl PlotiumApp {
    pub(crate) fn check_table_curve_revision(
        &self,
        source: &TableCurveSource,
    ) -> Result<(), String> {
        let (table, _) = self.table_curve_sources(source.source())?;
        if table.typed_state.envelope.revision.id != source.table_revision {
            return Err("Table revision is stale.".into());
        }
        Ok(())
    }

    pub(crate) fn check_curve_row_budget<'a>(
        &self,
        bindings: impl IntoIterator<Item = &'a DataBinding>,
    ) -> Result<(), String> {
        let mut rows = 0_u64;
        for binding in bindings {
            for series in &binding.series {
                let (table, columns) = self.table_curve_sources(series.source)?;
                rows = rows
                    .checked_add(
                        table
                            .typed_state
                            .envelope
                            .revision
                            .snapshot
                            .row_count
                            .saturating_mul(columns.len() as u64),
                    )
                    .ok_or("Binding row budget overflowed.")?;
            }
        }
        if rows > 2_000_000 {
            return Err(
                "Binding batch exceeds 2,000,000 curve-row visits; use a smaller batch.".into(),
            );
        }
        Ok(())
    }

    pub(crate) fn curve_binding_summaries(
        &self,
        canvas: CanvasId,
        object: ObjectId,
        binding: &DataBinding,
    ) -> Result<Vec<CurveBindingSummary>, String> {
        let coordinates = binding
            .coordinates
            .as_ref()
            .ok_or("Candidate has no coordinates.")?;
        let resource = canvas_object_ref(canvas, object);
        let mut summaries = Vec::new();
        for series in &binding.series {
            let (table, columns) = self.table_curve_sources(series.source)?;
            for columns in columns {
                let target = coordinates.for_axis(series.y_axis());
                let diagnostics = extract_table_curve(table, columns, &target)?.diagnostics;
                summaries.push(CurveBindingSummary {
                    binding_mode: if series.source.columns().is_some() {
                        "table_explicit"
                    } else {
                        "table_default"
                    }
                    .into(),
                    target: TargetRef {
                        resource: resource.clone(),
                        component: Some(ComponentRef::Series(series.id)),
                    },
                    source: TableCurveSource {
                        dataset_id: series.source.resource,
                        field_id: series.source.field,
                        table_id: columns.table_id,
                        table_revision: table.typed_state.envelope.revision.id,
                        x_column_id: columns.x_column_id,
                        y_column_id: columns.y_column_id,
                        y_error_column_id: columns.y_error_column_id,
                    },
                    diagnostics,
                });
            }
        }
        Ok(summaries)
    }
}
