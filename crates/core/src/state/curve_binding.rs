//! Plot-owned column selections and coordinate contracts. Table defaults are not edited.
use super::*;
use plotium_data::{ColumnId, TableId, UnitSpec};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TableCurveColumns {
    pub table_id: TableId,
    pub x_column_id: ColumnId,
    pub y_column_id: ColumnId,
    pub y_error_column_id: Option<ColumnId>,
}

/// Coordinates in which authored ranges and line shifts are interpreted.
/// None means unknown units, not an inferred dimensionless physical quantity.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurveCoordinates {
    pub x: Option<UnitSpec>,
    pub y: Option<UnitSpec>,
    /// Unit of the right y axis, fixed by the first curve placed on it, like
    /// `y` is by the plot's first curve.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y2: Option<UnitSpec>,
    /// Draw the data x along the vertical axis and values along the
    /// horizontal one (horizontal bars). Presentation only: units, ranges and
    /// extraction keep the data's own x and y.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub transposed: bool,
}

impl CurveCoordinates {
    /// The coordinates a curve measured on `side` is converted into.
    pub fn for_axis(&self, side: plotium_figure::YAxisSide) -> std::borrow::Cow<'_, Self> {
        if side.is_right() {
            std::borrow::Cow::Owned(Self {
                x: self.x.clone(),
                y: self.y2.clone(),
                y2: None,
                transposed: self.transposed,
            })
        } else {
            std::borrow::Cow::Borrowed(self)
        }
    }

    /// Unit of the values drawn along the horizontal axis.
    pub fn horizontal(&self) -> Option<&UnitSpec> {
        if self.transposed { &self.y } else { &self.x }.as_ref()
    }

    /// Unit of the values drawn along the vertical axis.
    pub fn vertical(&self) -> Option<&UnitSpec> {
        if self.transposed { &self.x } else { &self.y }.as_ref()
    }
}

/// Metadata is checked once per selected curve, before visiting any rows.
pub(crate) struct CurveUnitConversion<'a> {
    from: &'a Option<UnitSpec>,
    to: &'a Option<UnitSpec>,
    amplitude: bool,
}
impl<'a> CurveUnitConversion<'a> {
    pub(crate) fn new(
        from: &'a Option<UnitSpec>,
        to: &'a Option<UnitSpec>,
        amplitude: bool,
    ) -> Result<Self, String> {
        match (from, to) {
            (None, None) => {}
            (Some(a), Some(b)) => {
                a.validate().map_err(|e| e.to_string())?;
                b.validate().map_err(|e| e.to_string())?;
                if !a.is_compatible_with(b) {
                    return Err(format!(
                        "Incompatible curve units: {} and {}.",
                        a.display_unit, b.display_unit
                    ));
                }
            }
            _ => return Err("Known and unknown curve units cannot be combined.".into()),
        }
        Ok(Self {
            from,
            to,
            amplitude,
        })
    }
    pub(crate) fn apply(&self, value: f64) -> Result<f64, String> {
        let out = match (self.from, self.to) {
            (Some(a), Some(b)) if self.amplitude => value * a.scale.abs() / b.scale.abs(),
            (Some(a), Some(b)) => (value * a.scale + a.offset - b.offset) / b.scale,
            (None, None) => value,
            _ => unreachable!("unit contracts were validated at construction"),
        };
        out.is_finite()
            .then_some(out)
            .ok_or_else(|| "Curve unit conversion overflowed.".into())
    }
}
pub fn convert_curve_value(
    value: f64,
    from: &Option<UnitSpec>,
    to: &Option<UnitSpec>,
    amplitude: bool,
) -> Result<f64, String> {
    CurveUnitConversion::new(from, to, amplitude)?.apply(value)
}

impl PlotiumApp {
    pub fn table_curve_sources(
        &self,
        source: SeriesSource,
    ) -> Result<(&TableDataset, Vec<TableCurveColumns>), String> {
        let dataset = self
            .doc
            .dataset_by_id(source.resource)
            .ok_or("Curve dataset is missing.")?;
        let descriptor = dataset
            .field_descriptor(source.field)
            .ok_or("Curve field is missing.")?;
        if !descriptor
            .capabilities
            .contains(crate::automation::CAP_FIELD_TABLE)
            || !descriptor
                .capabilities
                .contains(crate::automation::CAP_FIELD_CURVE_1D)
        {
            return Err("Select a numeric table curve field.".into());
        }
        let table = dataset
            .as_table()
            .ok_or("The field does not provide table columns.")?;
        let snapshot = &table.typed_state.envelope.revision;
        let columns = match source.selection {
            SeriesSelection::TableColumns(columns) => {
                if columns.table_id != snapshot.table_id {
                    return Err("Curve TableId does not match the dataset.".into());
                }
                vec![columns]
            }
            SeriesSelection::FieldDefault => {
                let Some(x) = table.x_binding else {
                    return Ok((table, Vec::new()));
                };
                if table.series_bindings.iter().any(|b| b.fit.is_some())
                    || !table.line_fits.is_empty()
                {
                    return Err(
                        "Default fit overlays cannot be materialized as raw column curves.".into(),
                    );
                }
                table
                    .series_bindings
                    .iter()
                    .map(|b| TableCurveColumns {
                        table_id: snapshot.table_id,
                        x_column_id: x,
                        y_column_id: b.value_column,
                        y_error_column_id: b.uncertainty_column,
                    })
                    .collect()
            }
            SeriesSelection::TraceItem(_) => {
                return Err("A trace item is not a table column binding.".into());
            }
        };
        Ok((table, columns))
    }
}

pub fn table_curve_coordinates(
    table: &TableDataset,
    columns: TableCurveColumns,
) -> Result<CurveCoordinates, String> {
    let schema = &table.typed_state.envelope.revision.snapshot.schema;
    let unit = |id| {
        schema
            .column(id)
            .map(|c| c.unit.clone())
            .ok_or_else(|| format!("Curve column {id} is absent."))
    };
    Ok(CurveCoordinates {
        x: unit(columns.x_column_id)?,
        y: unit(columns.y_column_id)?,
        y2: None,
        transposed: false,
    })
}
