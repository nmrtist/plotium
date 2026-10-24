//! Shared typed revision adoption for sheet edits and explicit refreshes.
use super::{Dataset, DatasetId, PlotiumApp, TableEditDelta, TypedTableState};
use plotium_data::{CellPatch, CodecRegistry, RevisionId, TableTransaction};
use std::collections::BTreeSet;

pub fn prepare_table_cells(
    before: &TypedTableState,
    edits: &[CellPatch],
) -> Result<TypedTableState, String> {
    let mut seen = BTreeSet::new();
    let mut transaction = TableTransaction::new(&before.envelope.revision);
    for edit in edits {
        if !seen.insert((edit.row, edit.column)) {
            return Err("A patch addresses the same cell more than once.".into());
        }
        transaction.set(edit.row, edit.column, edit.value.clone());
    }
    let revision = transaction
        .execute_and_commit(
            &before.envelope.revision,
            before.store.as_ref(),
            &CodecRegistry::with_arrow_ipc(),
            env!("CARGO_PKG_VERSION"),
        )
        .map_err(|e| e.to_string())?;
    // A no-op must neither invalidate analyses nor create an Undo step.
    if revision.snapshot.fingerprint == before.envelope.revision.snapshot.fingerprint {
        return Ok(before.clone());
    }
    let mut envelope = before.envelope.clone();
    envelope.advance(revision).map_err(|e| e.to_string())?;
    Ok(TypedTableState {
        envelope,
        store: before.store.clone(),
    })
}

impl PlotiumApp {
    pub fn commit_table_revision(
        &mut self,
        dataset: DatasetId,
        expected: RevisionId,
        after: TypedTableState,
    ) -> Result<(), String> {
        let table = self
            .doc
            .dataset_by_id(dataset)
            .and_then(Dataset::as_table)
            .ok_or("The data table no longer exists.")?;
        let current = &table.typed_state.envelope.revision;
        if current.id != expected {
            return Err("The table revision changed; prepare the edit again.".into());
        }
        if current.table_id != after.envelope.revision.table_id {
            return Err("The prepared revision belongs to another table.".into());
        }
        // Plot-owned columns can impose stricter value constraints than the table schema.
        // Validate before touching document state or history, including refresh results.
        let mut candidate = table.clone();
        candidate.typed_state = after.clone();
        self.validate_table_curve_bindings(&candidate)?;
        let delta = TableEditDelta::from_revision(table, after);
        self.try_execute_action(crate::actions::Action::edit_table(dataset, delta))
            .map_err(|e| e.to_string())
    }

    fn validate_table_curve_bindings(&self, table: &super::TableDataset) -> Result<(), String> {
        for plot in self
            .doc
            .canvases
            .iter()
            .flat_map(|canvas| canvas.objects.iter().filter_map(|object| object.plot()))
        {
            let Some(coordinates) = &plot.binding.coordinates else {
                continue;
            };
            for series in &plot.binding.series {
                if series.source.resource != table.resource_id {
                    continue;
                }
                let columns = if let Some(columns) = series.source.columns() {
                    vec![columns]
                } else {
                    let x = table.x_binding.ok_or("Curve X is unbound.")?;
                    table
                        .series_bindings
                        .iter()
                        .map(|binding| super::TableCurveColumns {
                            table_id: table.typed_state.envelope.revision.table_id,
                            x_column_id: x,
                            y_column_id: binding.value_column,
                            y_error_column_id: binding.uncertainty_column,
                        })
                        .collect()
                };
                for columns in columns {
                    super::extract_table_curve(
                        table,
                        columns,
                        &coordinates.for_axis(series.y_axis()),
                    )
                    .map_err(|error| {
                        format!("Table update would invalidate a plotted curve: {error}")
                    })?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "table_update_tests.rs"]
mod tests;
