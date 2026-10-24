//! Explicit worksheet regions, for sheets whose recognized layout is wrong.
use super::{
    CodecRegistry, ImportedXlsxSheet, XlsxImportError, format_xlsx_diagnostic, import_table,
    layout::{Grid, LaidOutTable, lay_out},
    sheet_diagnostics,
};
use plotium_data::BlockStore;
use plotium_io::xlsx::{XlsxSheet, XlsxWorkbook};
use serde::{Deserialize, Serialize};

/// Imports each explicit region of a workbook as one table.
pub fn import_xlsx_regions(
    workbook: &XlsxWorkbook,
    regions: &[XlsxRegion],
    store: &dyn BlockStore,
    codecs: &CodecRegistry,
) -> Result<Vec<ImportedXlsxSheet>, XlsxImportError> {
    let workbook_diagnostics = workbook
        .diagnostics
        .iter()
        .map(format_xlsx_diagnostic)
        .collect::<Vec<_>>();
    regions
        .iter()
        .map(|region| {
            let sheet = workbook
                .sheets
                .iter()
                .find(|sheet| !sheet.hidden && sheet.name == region.sheet)
                .ok_or_else(|| {
                    XlsxImportError::Region(format!(
                        "Worksheet '{}' is not a visible worksheet of this workbook.",
                        region.sheet
                    ))
                })?;
            let table = region_table(sheet, region).map_err(XlsxImportError::Region)?;
            let range = Some(table.range.clone());
            let diagnostics = sheet_diagnostics(&workbook_diagnostics, sheet);
            import_table(table, sheet, range, diagnostics, store, codecs)
        })
        .collect()
}

/// An explicit rectangle of a worksheet to import as one table.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct XlsxRegion {
    pub sheet: String,
    /// A1 range such as `J2:AC4411`, or whole columns such as `J:AC`.
    pub range: String,
    /// Header rows, counted from the first non-empty row of the range;
    /// detected when absent.
    pub header_rows: Option<usize>,
    pub name: Option<String>,
    /// Splits a column where a text run separates two runs of values.
    #[serde(default = "split_segments_default")]
    pub split_segments: bool,
}

fn split_segments_default() -> bool {
    true
}

/// Lays out one explicit region.
fn region_table(sheet: &XlsxSheet, region: &XlsxRegion) -> Result<LaidOutTable, String> {
    let (rows, columns) = parse_range(&region.range)?;
    let grid = Grid::new(sheet);
    let outside = || {
        format!(
            "Range {} of worksheet '{}' holds no cells.",
            region.range, sheet.name
        )
    };
    // Only the used area holds cells, so a range is clamped to it before any
    // row or column is visited.
    let columns = (columns.0.max(sheet.origin.1)..=columns.1.min(grid.columns.saturating_sub(1)))
        .collect::<Vec<_>>();
    let end = rows
        .1
        .map_or(grid.rows, |end| end.saturating_add(1).min(grid.rows));
    let start = rows.0.unwrap_or(0).max(sheet.origin.0).min(end);
    let start = (start..end)
        .find(|&row| !grid.row_cells(row, &columns).is_empty())
        .ok_or_else(outside)?;
    if let Some(count) = region.header_rows
        && u32::try_from(count).map_or(true, |count| count >= end - start)
    {
        return Err(format!(
            "header_rows {count} leaves no data rows in range {} of worksheet '{}'.",
            region.range, sheet.name
        ));
    }
    let mut table = lay_out(
        &grid,
        &columns,
        start,
        end,
        region.header_rows,
        region.split_segments,
    )
    .ok_or_else(outside)?;
    table.name = region
        .name
        .clone()
        .unwrap_or_else(|| format!("{} {}", sheet.name, table.range));
    Ok(table.finish(sheet, true))
}

/// Zero-based first and last rows; absent for whole columns.
type RowBounds = (Option<u32>, Option<u32>);

/// Parses `J2:AC4411` or `J:AC` into zero-based row bounds and column bounds.
pub(super) fn parse_range(range: &str) -> Result<(RowBounds, (u32, u32)), String> {
    let invalid = || format!("Range '{range}' is not an A1 range such as J2:AC4411 or J:AC.");
    let (from, to) = range.trim().split_once(':').ok_or_else(invalid)?;
    let cell = |text: &str| -> Result<(u32, Option<u32>), String> {
        let split = text
            .find(|c: char| c.is_ascii_digit())
            .unwrap_or(text.len());
        let (letters, digits) = text.split_at(split);
        if letters.is_empty() || !letters.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(invalid());
        }
        let column = letters
            .chars()
            .try_fold(0u32, |acc, c| {
                acc.checked_mul(26)?
                    .checked_add(c.to_ascii_uppercase() as u32 - 'A' as u32 + 1)
            })
            .ok_or_else(invalid)?
            - 1;
        let row = match digits {
            "" => None,
            digits => Some(
                digits
                    .parse::<u32>()
                    .ok()
                    .filter(|row| *row > 0)
                    .ok_or_else(invalid)?
                    - 1,
            ),
        };
        Ok((column, row))
    };
    let (first_column, first_row) = cell(from)?;
    let (last_column, last_row) = cell(to)?;
    if first_row.is_some() != last_row.is_some()
        || first_column > last_column
        || first_row > last_row
    {
        return Err(invalid());
    }
    Ok(((first_row, last_row), (first_column, last_column)))
}
