use super::*;
use crate::state::{DataBinding, SeriesSelection};

pub(super) fn validate_columns(datasets: &[Dataset], binding: &DataBinding) -> Result<()> {
    let explicit = binding.series.iter().any(|s| s.source.columns().is_some());
    if explicit && binding.coordinates.is_none() {
        return Err(ProjectError::Invalid(
            "Explicit curves have no coordinate unit contract.".into(),
        ));
    }
    if let Some(coordinates) = &binding.coordinates {
        for series in &binding.series {
            let dataset = datasets
                .iter()
                .find(|d| d.resource_id() == series.source.resource)
                .ok_or_else(|| ProjectError::Invalid("Curve dataset is missing.".into()))?;
            let table = dataset.as_table().ok_or_else(|| {
                ProjectError::Invalid("Explicit plot has a non-table curve.".into())
            })?;
            let columns = if let Some(columns) = series.source.columns() {
                vec![columns]
            } else {
                let x = table
                    .x_binding
                    .ok_or_else(|| ProjectError::Invalid("Curve X is unbound.".into()))?;
                table
                    .series_bindings
                    .iter()
                    .map(|b| crate::state::TableCurveColumns {
                        table_id: table.typed_state.envelope.revision.table_id,
                        x_column_id: x,
                        y_column_id: b.value_column,
                        y_error_column_id: b.uncertainty_column,
                    })
                    .collect()
            };
            for columns in columns {
                let target = coordinates.for_axis(series.y_axis());
                crate::state::extract_table_curve(table, columns, &target)
                    .map_err(ProjectError::Invalid)?;
            }
        }
    }
    Ok(())
}

pub(super) fn save_series(
    datasets: &[Dataset],
    plot: &PlotObject,
    context: &str,
) -> Result<Vec<SeriesBindingDto>> {
    validate_columns(datasets, &plot.binding)?;
    let mut ids = std::collections::BTreeSet::new();
    plot.binding
        .series
        .iter()
        .map(|sb| {
            if !ids.insert(sb.id) {
                return Err(ProjectError::Invalid(format!(
                    "{context} has duplicate series id {}",
                    sb.id
                )));
            }
            let dataset = datasets
                .iter()
                .find(|d| d.resource_id() == sb.source.resource)
                .ok_or_else(|| {
                    ProjectError::Invalid(format!(
                        "{context} references missing series dataset {}",
                        sb.source.resource
                    ))
                })?;
            super::field_catalog::validate_series_source(
                dataset,
                sb.source.field,
                sb.source.item(),
                &sb.encoding,
                context,
            )?;
            let input = format!("recipe_{}", dataset.resource_id());
            let field = sb.source.field.get();
            let source = match sb.source.selection {
                SeriesSelection::FieldDefault => SeriesSourceDto::Field { input, field },
                SeriesSelection::TraceItem(item) => {
                    SeriesSourceDto::TraceItem { input, field, item }
                }
                SeriesSelection::TableColumns(columns) => SeriesSourceDto::TableColumns {
                    input,
                    field,
                    columns,
                },
            };
            Ok(SeriesBindingDto {
                id: sb.id.get(),
                source,
                label: sb.label.clone(),
                encoding: sb.encoding.clone(),
                visible: sb.visible,
                replicates: sb.replicates,
                error_bars: sb.error_bars,
            })
        })
        .collect()
}
