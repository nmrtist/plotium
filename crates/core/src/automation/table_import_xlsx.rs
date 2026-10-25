//! XLSX workbooks import one native table per worksheet, as the desktop does.
use super::table_import::{TABLE_IMPORT_COLUMNS, TABLE_IMPORT_ROWS, TABLE_IMPORT_XLSX_CELLS};
use crate::{
    state::{Dataset, TableDataset, TableImportSource, TypedTableState},
    xlsx::ImportedXlsxSheet,
};
use plotium_data::{CodecRegistry, MemoryBlockStore};
use plotium_io::xlsx::{XlsxIoError, XlsxReader, XlsxWorkbook};
use std::{path::Path, sync::Arc};

const XLSX_MEDIA_TYPE: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";

/// A prepared table, or why it failed, with the worksheet it came from.
pub(super) type PreparedTable = (Option<String>, Result<Dataset, String>);

/// Parses the retained bytes worksheet by worksheet, so one worksheet that
/// fails or exceeds the cell budget leaves the others importable. `sheets`
/// selects worksheets by name; otherwise every visible worksheet with cells is
/// imported.
pub(super) fn prepare(
    path: &Path,
    bytes: Vec<u8>,
    sheets: Option<&[String]>,
) -> Result<Vec<PreparedTable>, String> {
    let bytes = Arc::<[u8]>::from(bytes);
    let mut reader =
        XlsxReader::from_bytes(&bytes, TABLE_IMPORT_XLSX_CELLS).map_err(|e| e.to_string())?;
    let visible = reader
        .sheets
        .iter()
        .filter(|sheet| !sheet.hidden)
        .map(|sheet| sheet.name.clone())
        .collect::<Vec<_>>();
    let selected = match sheets {
        Some(names) => {
            if let Some(missing) = names.iter().find(|name| !visible.contains(name)) {
                let available = visible
                    .iter()
                    .map(|name| format!("'{name}'"))
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(format!(
                    "Worksheet '{missing}' is not a visible worksheet of this workbook. Visible worksheets: {available}."
                ));
            }
            visible
                .iter()
                .filter(|name| names.contains(name))
                .cloned()
                .collect()
        }
        None => visible,
    };
    let mut remaining = TABLE_IMPORT_XLSX_CELLS;
    let mut read = Vec::new();
    for name in selected {
        match reader.read_sheet(&name, remaining) {
            Ok(sheet) => {
                let width = sheet.rows.iter().map(Vec::len).max().unwrap_or(0);
                remaining = remaining.saturating_sub(sheet.rows.len() * width);
                // Workbooks often keep blank default sheets, which hold no table.
                if sheets.is_some() || !sheet.rows.is_empty() {
                    read.push((name, Ok(sheet)));
                }
            }
            Err(XlsxIoError::SheetOverBudget { .. }) => read.push((
                name,
                Err(format!(
                    "The worksheet exceeds the cell budget: one XLSX import reads at most {TABLE_IMPORT_XLSX_CELLS} cells across its worksheets. Name fewer worksheets in sheets."
                )),
            )),
            Err(error) => read.push((name, Err(error.to_string()))),
        }
    }
    if read.is_empty() {
        return Err("The workbook has no visible worksheet with cells.".into());
    }
    let shared = reader.into_workbook(Vec::new());
    let file_stem = (sheets.is_none() && read.len() == 1).then(|| {
        path.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    });
    let retained = Retained {
        bytes,
        file_name: path.file_name().map(|s| s.to_string_lossy().into_owned()),
        store: Arc::new(MemoryBlockStore::default()),
    };
    let codecs = CodecRegistry::with_arrow_ipc();
    let mut prepared = Vec::new();
    for (name, sheet) in read {
        let tables = sheet.and_then(|sheet| {
            let workbook = XlsxWorkbook {
                sheets: vec![sheet],
                ..shared.clone()
            };
            crate::xlsx::import_xlsx_workbook(&workbook, retained.store.as_ref(), &codecs)
                .map_err(|e| e.to_string())
        });
        match tables {
            Ok(tables) => prepared.extend(tables.into_iter().map(|table| {
                let dataset_name = file_stem.clone().unwrap_or_else(|| table.name.clone());
                (
                    Some(table.name.clone()),
                    retained.dataset(table, dataset_name),
                )
            })),
            Err(error) => prepared.push((Some(name), Err(error))),
        }
    }
    Ok(prepared)
}

/// What every table of one workbook shares.
struct Retained {
    bytes: Arc<[u8]>,
    file_name: Option<String>,
    store: Arc<MemoryBlockStore>,
}

impl Retained {
    fn dataset(&self, sheet: ImportedXlsxSheet, name: String) -> Result<Dataset, String> {
        if sheet.snapshot.schema.columns.len() > TABLE_IMPORT_COLUMNS
            || sheet.snapshot.row_count > TABLE_IMPORT_ROWS as u64
        {
            return Err(format!(
                "The worksheet exceeds the budget of {TABLE_IMPORT_COLUMNS} columns and {TABLE_IMPORT_ROWS} rows per table."
            ));
        }
        let typed = TypedTableState::imported_with_operation(
            sheet.snapshot,
            Arc::clone(&self.store),
            "plotium.import.xlsx.v1",
        )
        .map_err(|e| e.to_string())?;
        let mut table = TableDataset::from_typed(typed);
        table.name = Some(name);
        let mut source = TableImportSource::new(Arc::clone(&self.bytes), XLSX_MEDIA_TYPE);
        source.name = self.file_name.clone();
        source.metadata.insert(
            "space.nmrtist.plotium.import.worksheet".into(),
            serde_json::Value::String(sheet.name),
        );
        source.metadata.insert(
            "space.nmrtist.plotium.import.formula_policy".into(),
            serde_json::Value::String("cached-values-only".into()),
        );
        table.import_sources.push(source);
        Ok(Dataset::Table(Box::new(table)))
    }
}

#[cfg(test)]
#[path = "table_import_xlsx_tests.rs"]
mod tests;
