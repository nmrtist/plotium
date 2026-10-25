//! XLSX workbooks import one native table per recognized worksheet table, as
//! the desktop does, plus any explicitly named regions.
use super::table_import::{TABLE_IMPORT_COLUMNS, TABLE_IMPORT_ROWS, TABLE_IMPORT_XLSX_CELLS};
use crate::{
    state::{Dataset, TableDataset, TableImportSource, TypedTableState},
    xlsx::{ImportedXlsxSheet, XlsxRegion},
};
use plotium_data::{CodecRegistry, MemoryBlockStore};
use plotium_io::xlsx::{XlsxIoError, XlsxReader, XlsxWorkbook};
use std::{path::Path, sync::Arc};

const XLSX_MEDIA_TYPE: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";

/// The worksheet, and range within it, that an XLSX table was read from.
#[derive(Clone, Debug)]
pub(super) struct SourceSheet {
    pub worksheet: String,
    pub range: Option<String>,
}

/// A prepared table, or why it failed, with the worksheet it came from.
pub(super) type PreparedTable = (Option<SourceSheet>, Result<Dataset, String>);

/// Parses the retained bytes worksheet by worksheet, so one worksheet or
/// region that fails or exceeds the cell budget leaves the others importable.
/// `sheets` selects worksheets for recognition and `regions` adds explicit
/// ranges; without either, every visible worksheet with cells is recognized.
pub(super) fn prepare(
    path: &Path,
    bytes: Vec<u8>,
    sheets: Option<&[String]>,
    regions: Option<&[XlsxRegion]>,
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
    let named = sheets
        .into_iter()
        .flatten()
        .chain(regions.into_iter().flatten().map(|region| &region.sheet));
    for name in named {
        if !visible.contains(name) {
            let available = visible
                .iter()
                .map(|name| format!("'{name}'"))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "Worksheet '{name}' is not a visible worksheet of this workbook. Visible worksheets: {available}."
            ));
        }
    }
    let recognized = |name: &String| match (sheets, regions) {
        (Some(names), _) => names.contains(name),
        (None, Some(_)) => false,
        (None, None) => true,
    };
    let in_region =
        |name: &String| regions.is_some_and(|regions| regions.iter().any(|r| r.sheet == *name));
    let mut remaining = TABLE_IMPORT_XLSX_CELLS;
    let mut read = Vec::new();
    for name in visible
        .iter()
        .filter(|name| recognized(name) || in_region(name))
    {
        let sheet = match reader.read_sheet(name, remaining) {
            Ok(sheet) => {
                let width = sheet.rows.iter().map(Vec::len).max().unwrap_or(0);
                remaining = remaining.saturating_sub(sheet.rows.len() * width);
                // Workbooks often keep blank default sheets, which hold no table.
                if sheets.is_none() && regions.is_none() && sheet.rows.is_empty() {
                    continue;
                }
                Ok(sheet)
            }
            Err(XlsxIoError::SheetOverBudget { .. }) => Err(format!(
                "The worksheet exceeds the cell budget: one XLSX import reads at most {TABLE_IMPORT_XLSX_CELLS} cells across its worksheets. Name fewer worksheets in sheets."
            )),
            Err(error) => Err(error.to_string()),
        };
        read.push((name.clone(), sheet));
    }
    if read.is_empty() {
        return Err("The workbook has no visible worksheet with cells.".into());
    }
    // Each worksheet is imported as a workbook of its own so it fails alone;
    // the schema sidecar and cell diagnostics are shared.
    let shared = reader.into_workbook(Vec::new());
    let read = read
        .into_iter()
        .map(|(name, sheet)| {
            let workbook = sheet.map(|sheet| XlsxWorkbook {
                sheets: vec![sheet],
                ..shared.clone()
            });
            (name, workbook)
        })
        .collect::<Vec<_>>();
    let retained = Retained {
        bytes,
        file_name: path.file_name().map(|s| s.to_string_lossy().into_owned()),
        store: Arc::new(MemoryBlockStore::default()),
    };
    let codecs = CodecRegistry::with_arrow_ipc();
    let mut tables: Vec<(SourceSheet, Result<ImportedXlsxSheet, String>)> = Vec::new();
    for (name, sheet) in read.iter().filter(|(name, _)| recognized(name)) {
        let imported = sheet.as_ref().map_err(Clone::clone).and_then(|workbook| {
            crate::xlsx::import_xlsx_workbook(workbook, retained.store.as_ref(), &codecs)
                .map_err(|e| e.to_string())
        });
        match imported {
            Ok(imported) => tables.extend(imported.into_iter().map(|table| {
                let source = SourceSheet {
                    worksheet: table.worksheet.clone(),
                    range: table.range.clone(),
                };
                (source, Ok(table))
            })),
            Err(error) => tables.push((
                SourceSheet {
                    worksheet: name.clone(),
                    range: None,
                },
                Err(error),
            )),
        }
    }
    for region in regions.into_iter().flatten() {
        let sheet = read
            .iter()
            .find(|(name, _)| *name == region.sheet)
            .map(|(_, sheet)| sheet)
            .ok_or("The region's worksheet was not read.")?;
        let imported = sheet.as_ref().map_err(Clone::clone).and_then(|workbook| {
            crate::xlsx::import_xlsx_regions(
                workbook,
                std::slice::from_ref(region),
                retained.store.as_ref(),
                &codecs,
            )
            .map_err(|e| e.to_string())?
            .pop()
            .ok_or_else(|| "The region produced no table.".to_owned())
        });
        let source = SourceSheet {
            worksheet: region.sheet.clone(),
            range: imported
                .as_ref()
                .map_or(Some(region.range.clone()), |table| table.range.clone()),
        };
        tables.push((source, imported));
    }
    let file_stem = (sheets.is_none() && regions.is_none() && tables.len() == 1).then(|| {
        path.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    });
    Ok(tables
        .into_iter()
        .map(|(source, table)| {
            let dataset = table.and_then(|table| {
                let name = file_stem.clone().unwrap_or_else(|| table.name.clone());
                retained.dataset(table, name)
            });
            (Some(source), dataset)
        })
        .collect())
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
                "The table exceeds the budget of {TABLE_IMPORT_COLUMNS} columns and {TABLE_IMPORT_ROWS} rows per table; import a narrower range with regions."
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
            serde_json::Value::String(sheet.worksheet),
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
