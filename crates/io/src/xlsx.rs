//! XLSX value/cache transport with an optional hidden Plotium schema channel.
//!
//! Plotium deliberately does not evaluate Excel formulas. Imports retain the
//! formula text and use only the cached value stored in the workbook.

use calamine::{Data, HeaderRow, Reader, SheetVisible, Xlsx, open_workbook};
use rust_xlsxwriter::{Workbook, XlsxError as WriteError};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::{
    io::{Cursor, Read, Seek},
    path::Path,
};

pub const PLOTIUM_SCHEMA_SHEET: &str = "_Plotium_schema_v1";
const PLOTIUM_SCHEMA_MARKER: &str = "plotium-schema-v1";
const SCHEMA_CHUNK_BYTES: usize = 30_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XlsxWorkbook {
    pub sheets: Vec<XlsxSheet>,
    pub uses_1904_date_system: bool,
    pub plotium_schema: Option<JsonValue>,
    pub diagnostics: Vec<XlsxDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XlsxSheet {
    pub name: String,
    pub hidden: bool,
    pub rows: Vec<Vec<XlsxCell>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XlsxCell {
    pub value: XlsxValue,
    pub formula: Option<String>,
}

impl XlsxCell {
    pub fn value(value: XlsxValue) -> Self {
        Self {
            value,
            formula: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum XlsxValue {
    Empty,
    Boolean(bool),
    Int64(i64),
    Float64(f64),
    Utf8(String),
    ExcelDateTime {
        serial: f64,
        duration: bool,
        iso8601: String,
    },
    DateTimeIso(String),
    DurationIso(String),
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct XlsxDiagnostic {
    pub code: XlsxDiagnosticCode,
    pub sheet: Option<String>,
    pub row: Option<u32>,
    pub column: Option<u32>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum XlsxDiagnosticCode {
    FormulaWithoutCachedValue,
    ErrorCell,
    InvalidPlotiumSchema,
}

#[derive(Debug, thiserror::Error)]
pub enum XlsxIoError {
    #[error("cannot read XLSX workbook: {0}")]
    Read(#[from] calamine::XlsxError),
    #[error("cannot write XLSX workbook: {0}")]
    Write(#[from] WriteError),
    #[error("XLSX export requires at least one visible worksheet")]
    NoVisibleWorksheet,
    #[error("XLSX export with the 1904 date system is not supported")]
    Unsupported1904DateSystem,
    #[error("worksheet row or column exceeds the XLSX format limit")]
    SheetTooLarge,
    #[error("worksheet '{sheet}' spans more than {max_cells} cells")]
    SheetOverBudget { sheet: String, max_cells: usize },
}

pub fn read_xlsx(path: impl AsRef<Path>) -> Result<XlsxWorkbook, XlsxIoError> {
    let mut reader = XlsxReader::new(open_workbook(path)?, usize::MAX)?;
    let names = reader.sheets.clone();
    let sheets = names
        .iter()
        .map(|sheet| reader.read_sheet(&sheet.name, usize::MAX))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(reader.into_workbook(sheets))
}

/// A worksheet's name and visibility, known before its cells are read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XlsxSheetInfo {
    pub name: String,
    pub hidden: bool,
}

/// Reads worksheets one at a time, so callers can select worksheets and bound
/// each one's size before its cells are materialized.
pub struct XlsxReader<RS> {
    reader: Xlsx<RS>,
    /// Data worksheets in workbook order, without the hidden schema sheet.
    pub sheets: Vec<XlsxSheetInfo>,
    pub uses_1904_date_system: bool,
    pub plotium_schema: Option<JsonValue>,
    pub diagnostics: Vec<XlsxDiagnostic>,
}

impl<'a> XlsxReader<Cursor<&'a [u8]>> {
    /// Opens a workbook already held in memory, so callers that budget and
    /// retain the source bytes parse exactly what they retain. `max_cells`
    /// bounds the hidden schema sheet.
    pub fn from_bytes(bytes: &'a [u8], max_cells: usize) -> Result<Self, XlsxIoError> {
        Self::new(Xlsx::new(Cursor::new(bytes))?, max_cells)
    }
}

impl<RS: Read + Seek> XlsxReader<RS> {
    fn new(mut reader: Xlsx<RS>, max_cells: usize) -> Result<Self, XlsxIoError> {
        reader.with_header_row(HeaderRow::Row(0));
        let uses_1904_date_system = reader.has_1904_epoch();
        let metadata = reader.sheets_metadata().to_vec();
        let mut diagnostics = Vec::new();
        let mut plotium_schema = None;
        if metadata
            .iter()
            .any(|sheet| sheet.name == PLOTIUM_SCHEMA_SHEET)
        {
            check_extent(&mut reader, PLOTIUM_SCHEMA_SHEET, max_cells)?;
            let values = reader.worksheet_range(PLOTIUM_SCHEMA_SHEET)?;
            plotium_schema = read_schema(&values, &mut diagnostics);
        }
        let sheets = metadata
            .into_iter()
            .filter(|sheet| sheet.name != PLOTIUM_SCHEMA_SHEET)
            .map(|sheet| XlsxSheetInfo {
                hidden: sheet.visible != SheetVisible::Visible,
                name: sheet.name,
            })
            .collect();
        Ok(Self {
            reader,
            sheets,
            uses_1904_date_system,
            plotium_schema,
            diagnostics,
        })
    }

    /// Reads one worksheet, failing before materializing it when its used
    /// range spans more than `max_cells` cells. Cell diagnostics accumulate
    /// in `diagnostics`.
    pub fn read_sheet(&mut self, name: &str, max_cells: usize) -> Result<XlsxSheet, XlsxIoError> {
        let hidden = self
            .sheets
            .iter()
            .find(|sheet| sheet.name == name)
            .ok_or_else(|| calamine::XlsxError::WorksheetNotFound(name.to_owned()))?
            .hidden;
        check_extent(&mut self.reader, name, max_cells)?;
        let values = self.reader.worksheet_range(name)?;
        let formulas = self.reader.worksheet_formula(name)?;
        let start = union_start(values.start(), formulas.start());
        let end = union_end(values.end(), formulas.end());
        let rows = match (start, end) {
            (Some(start), Some(end)) => {
                read_rows(name, start, end, &values, &formulas, &mut self.diagnostics)
            }
            _ => Vec::new(),
        };
        Ok(XlsxSheet {
            name: name.to_owned(),
            hidden,
            rows,
        })
    }

    pub fn into_workbook(self, sheets: Vec<XlsxSheet>) -> XlsxWorkbook {
        XlsxWorkbook {
            sheets,
            uses_1904_date_system: self.uses_1904_date_system,
            plotium_schema: self.plotium_schema,
            diagnostics: self.diagnostics,
        }
    }
}

/// Streams a worksheet's cell positions and fails when the used range they
/// span exceeds `max_cells`, since reading a range allocates it densely.
fn check_extent<RS: Read + Seek>(
    reader: &mut Xlsx<RS>,
    name: &str,
    max_cells: usize,
) -> Result<(), XlsxIoError> {
    if max_cells == usize::MAX {
        return Ok(());
    }
    let mut cells = reader.worksheet_cells_reader(name)?;
    let mut extent: Option<((u32, u32), (u32, u32))> = None;
    while let Some(cell) = cells.next_cell_with_formula()? {
        let (row, column) = cell.pos;
        let ((top, left), (bottom, right)) = extent.get_or_insert(((row, column), (row, column)));
        *top = (*top).min(row);
        *left = (*left).min(column);
        *bottom = (*bottom).max(row);
        *right = (*right).max(column);
        let rows = (*bottom - *top) as usize + 1;
        let columns = (*right - *left) as usize + 1;
        if rows.saturating_mul(columns) > max_cells {
            return Err(XlsxIoError::SheetOverBudget {
                sheet: name.to_owned(),
                max_cells,
            });
        }
    }
    Ok(())
}

pub fn write_xlsx(path: impl AsRef<Path>, workbook: &XlsxWorkbook) -> Result<(), XlsxIoError> {
    if workbook.uses_1904_date_system {
        return Err(XlsxIoError::Unsupported1904DateSystem);
    }
    if !workbook.sheets.iter().any(|sheet| !sheet.hidden) {
        return Err(XlsxIoError::NoVisibleWorksheet);
    }

    let mut output = StreamingXlsxWriter::new();
    for sheet in &workbook.sheets {
        let sheet_index = output.add_worksheet(&sheet.name, sheet.hidden)?;
        for (row_index, row) in sheet.rows.iter().enumerate() {
            let row_index = u32::try_from(row_index).map_err(|_| XlsxIoError::SheetTooLarge)?;
            output.write_row(sheet_index, row_index, row.iter().map(|cell| &cell.value))?;
        }
    }
    output.finish(path, workbook.plotium_schema.as_ref())
}

/// Backend-neutral, row-at-a-time XLSX writer. Callers never receive a
/// `rust_xlsxwriter` type and can therefore stream large typed snapshots.
pub struct StreamingXlsxWriter {
    output: Workbook,
    visible_sheets: usize,
}

impl StreamingXlsxWriter {
    pub fn new() -> Self {
        Self {
            output: Workbook::new(),
            visible_sheets: 0,
        }
    }

    pub fn add_worksheet(&mut self, name: &str, hidden: bool) -> Result<usize, XlsxIoError> {
        let index = self.output.worksheets().len();
        let worksheet = self.output.add_worksheet_with_constant_memory()?;
        worksheet.set_name(name)?;
        worksheet.set_hidden(hidden);
        if !hidden {
            self.visible_sheets += 1;
        }
        Ok(index)
    }

    pub fn write_row<'a>(
        &mut self,
        sheet: usize,
        row: u32,
        values: impl IntoIterator<Item = &'a XlsxValue>,
    ) -> Result<(), XlsxIoError> {
        let worksheet = self.output.worksheet_from_index(sheet)?;
        for (column, value) in values.into_iter().enumerate() {
            let column = u16::try_from(column).map_err(|_| XlsxIoError::SheetTooLarge)?;
            write_cell(worksheet, row, column, value)?;
        }
        Ok(())
    }

    pub fn finish(
        mut self,
        path: impl AsRef<Path>,
        plotium_schema: Option<&JsonValue>,
    ) -> Result<(), XlsxIoError> {
        if self.visible_sheets == 0 {
            return Err(XlsxIoError::NoVisibleWorksheet);
        }
        if let Some(schema) = plotium_schema {
            let worksheet = self.output.add_worksheet();
            worksheet.set_name(PLOTIUM_SCHEMA_SHEET)?;
            worksheet.set_hidden(true);
            worksheet.write_string(0, 0, PLOTIUM_SCHEMA_MARKER)?;
            let encoded = serde_json::to_string(schema).expect("JSON values always serialize");
            for (index, chunk) in utf8_chunks(&encoded, SCHEMA_CHUNK_BYTES).enumerate() {
                let row = u32::try_from(index + 1).map_err(|_| XlsxIoError::SheetTooLarge)?;
                worksheet.write_string(row, 0, chunk)?;
            }
        }
        self.output.save(path)?;
        Ok(())
    }
}

impl Default for StreamingXlsxWriter {
    fn default() -> Self {
        Self::new()
    }
}

fn read_rows(
    sheet: &str,
    start: (u32, u32),
    end: (u32, u32),
    values: &calamine::Range<Data>,
    formulas: &calamine::Range<String>,
    diagnostics: &mut Vec<XlsxDiagnostic>,
) -> Vec<Vec<XlsxCell>> {
    (start.0..=end.0)
        .map(|row| {
            (start.1..=end.1)
                .map(|column| {
                    let value = values.get_value((row, column)).unwrap_or(&Data::Empty);
                    let formula = formulas
                        .get_value((row, column))
                        .filter(|value| !value.is_empty())
                        .cloned();
                    if formula.is_some() && matches!(value, Data::Empty) {
                        diagnostics.push(cell_diagnostic(
                            XlsxDiagnosticCode::FormulaWithoutCachedValue,
                            sheet,
                            row,
                            column,
                            "formula has no cached value; imported as null",
                        ));
                    }
                    if let Data::Error(error) = value {
                        diagnostics.push(cell_diagnostic(
                            XlsxDiagnosticCode::ErrorCell,
                            sheet,
                            row,
                            column,
                            format!("Excel error cell {error}"),
                        ));
                    }
                    XlsxCell {
                        value: convert_value(value),
                        formula,
                    }
                })
                .collect()
        })
        .collect()
}

fn convert_value(value: &Data) -> XlsxValue {
    match value {
        Data::Empty => XlsxValue::Empty,
        Data::Bool(value) => XlsxValue::Boolean(*value),
        Data::Int(value) => XlsxValue::Int64(*value),
        Data::Float(value) => XlsxValue::Float64(*value),
        Data::String(value) => XlsxValue::Utf8(value.clone()),
        Data::DateTime(value) => {
            let (year, month, day, hour, minute, second, millis) = value.to_ymd_hms_milli();
            XlsxValue::ExcelDateTime {
                serial: value.as_f64(),
                duration: value.is_duration(),
                iso8601: format!(
                    "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}"
                ),
            }
        }
        Data::DateTimeIso(value) => XlsxValue::DateTimeIso(value.clone()),
        Data::DurationIso(value) => XlsxValue::DurationIso(value.clone()),
        Data::Error(value) => XlsxValue::Error(value.to_string()),
    }
}

fn write_cell(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    row: u32,
    column: u16,
    value: &XlsxValue,
) -> Result<(), WriteError> {
    match value {
        XlsxValue::Empty | XlsxValue::Error(_) => {}
        XlsxValue::Boolean(value) => {
            worksheet.write_boolean(row, column, *value)?;
        }
        XlsxValue::Int64(value) => {
            worksheet.write_number(row, column, *value as f64)?;
        }
        XlsxValue::Float64(value) | XlsxValue::ExcelDateTime { serial: value, .. } => {
            worksheet.write_number(row, column, *value)?;
        }
        XlsxValue::Utf8(value) | XlsxValue::DateTimeIso(value) | XlsxValue::DurationIso(value) => {
            worksheet.write_string(row, column, value)?;
        }
    }
    Ok(())
}

fn read_schema(
    values: &calamine::Range<Data>,
    diagnostics: &mut Vec<XlsxDiagnostic>,
) -> Option<JsonValue> {
    let marker = values.get_value((0, 0));
    if marker != Some(&Data::String(PLOTIUM_SCHEMA_MARKER.to_owned())) {
        diagnostics.push(XlsxDiagnostic {
            code: XlsxDiagnosticCode::InvalidPlotiumSchema,
            sheet: Some(PLOTIUM_SCHEMA_SHEET.to_owned()),
            row: Some(0),
            column: Some(0),
            message: "hidden Plotium schema sheet has an invalid marker".to_owned(),
        });
        return None;
    }
    let mut encoded = String::new();
    let end_row = values.end().map_or(0, |end| end.0);
    for row in 1..=end_row {
        if let Some(Data::String(chunk)) = values.get_value((row, 0)) {
            encoded.push_str(chunk);
        }
    }
    match serde_json::from_str(&encoded) {
        Ok(schema) => Some(schema),
        Err(error) => {
            diagnostics.push(XlsxDiagnostic {
                code: XlsxDiagnosticCode::InvalidPlotiumSchema,
                sheet: Some(PLOTIUM_SCHEMA_SHEET.to_owned()),
                row: None,
                column: None,
                message: format!("hidden Plotium schema JSON is invalid: {error}"),
            });
            None
        }
    }
}

fn cell_diagnostic(
    code: XlsxDiagnosticCode,
    sheet: &str,
    row: u32,
    column: u32,
    message: impl Into<String>,
) -> XlsxDiagnostic {
    XlsxDiagnostic {
        code,
        sheet: Some(sheet.to_owned()),
        row: Some(row),
        column: Some(column),
        message: message.into(),
    }
}

fn union_start(left: Option<(u32, u32)>, right: Option<(u32, u32)>) -> Option<(u32, u32)> {
    match (left, right) {
        (Some(left), Some(right)) => Some((left.0.min(right.0), left.1.min(right.1))),
        (left, right) => left.or(right),
    }
}

fn union_end(left: Option<(u32, u32)>, right: Option<(u32, u32)>) -> Option<(u32, u32)> {
    match (left, right) {
        (Some(left), Some(right)) => Some((left.0.max(right.0), left.1.max(right.1))),
        (left, right) => left.or(right),
    }
}

fn utf8_chunks(value: &str, max_bytes: usize) -> impl Iterator<Item = &str> {
    let mut start = 0;
    std::iter::from_fn(move || {
        if start >= value.len() {
            return None;
        }
        let mut end = (start + max_bytes).min(value.len());
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        let chunk = &value[start..end];
        start = end;
        Some(chunk)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_xlsxwriter::Formula;
    use serde_json::json;
    use std::io::{Read, Write};

    #[test]
    fn round_trips_values_and_hidden_schema() {
        let path = std::env::temp_dir().join(format!(
            "plotium-xlsx-{}-{}.xlsx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let workbook = XlsxWorkbook {
            sheets: vec![XlsxSheet {
                name: "Data".to_owned(),
                hidden: false,
                rows: vec![
                    vec![
                        XlsxCell::value(XlsxValue::Utf8("sample".to_owned())),
                        XlsxCell::value(XlsxValue::Utf8("value".to_owned())),
                    ],
                    vec![
                        XlsxCell::value(XlsxValue::Utf8("α".to_owned())),
                        XlsxCell::value(XlsxValue::Float64(2.5)),
                    ],
                ],
            }],
            uses_1904_date_system: false,
            plotium_schema: Some(json!({"schema_version": 1, "note": "光谱"})),
            diagnostics: Vec::new(),
        };
        write_xlsx(&path, &workbook).unwrap();
        let restored = read_xlsx(&path).unwrap();
        std::fs::remove_file(&path).unwrap();

        assert_eq!(restored.sheets, workbook.sheets);
        assert_eq!(restored.plotium_schema, workbook.plotium_schema);
        assert!(restored.diagnostics.is_empty());
    }

    #[test]
    fn round_trips_multiple_visible_worksheets() {
        let path = temporary_xlsx_path("multiple-sheets");
        let workbook = XlsxWorkbook {
            sheets: vec![
                XlsxSheet {
                    name: "Samples".to_owned(),
                    hidden: false,
                    rows: vec![vec![XlsxCell::value(XlsxValue::Utf8("sample".into()))]],
                },
                XlsxSheet {
                    name: "Measurements".to_owned(),
                    hidden: false,
                    rows: vec![vec![XlsxCell::value(XlsxValue::Float64(2.5))]],
                },
            ],
            uses_1904_date_system: false,
            plotium_schema: None,
            diagnostics: Vec::new(),
        };
        write_xlsx(&path, &workbook).unwrap();
        let restored = read_xlsx(&path).unwrap();
        std::fs::remove_file(path).unwrap();

        assert_eq!(restored.sheets, workbook.sheets);
    }

    #[test]
    fn error_cells_are_explicit_and_diagnosed() {
        let mut values = calamine::Range::new((0, 0), (0, 0));
        values.set_value((0, 0), Data::Error(calamine::CellErrorType::Div0));
        let formulas = calamine::Range::empty();
        let mut diagnostics = Vec::new();

        let rows = read_rows(
            "Errors",
            (0, 0),
            (0, 0),
            &values,
            &formulas,
            &mut diagnostics,
        );

        assert!(matches!(rows[0][0].value, XlsxValue::Error(_)));
        assert_eq!(diagnostics[0].code, XlsxDiagnosticCode::ErrorCell);
        assert_eq!(diagnostics[0].sheet.as_deref(), Some("Errors"));
    }

    #[test]
    fn export_rejects_the_unsupported_1904_date_system() {
        let path = temporary_xlsx_path("1904-date-system");
        let workbook = XlsxWorkbook {
            sheets: vec![XlsxSheet {
                name: "Data".to_owned(),
                hidden: false,
                rows: vec![vec![XlsxCell::value(XlsxValue::Int64(1))]],
            }],
            uses_1904_date_system: true,
            plotium_schema: None,
            diagnostics: Vec::new(),
        };

        assert!(matches!(
            write_xlsx(&path, &workbook),
            Err(XlsxIoError::Unsupported1904DateSystem)
        ));
        assert!(!path.exists());
    }

    #[test]
    fn utf8_schema_chunks_do_not_split_code_points() {
        let input = "光".repeat(20_001);
        let chunks = utf8_chunks(&input, SCHEMA_CHUNK_BYTES).collect::<Vec<_>>();
        assert!(chunks.iter().all(|chunk| chunk.len() <= SCHEMA_CHUNK_BYTES));
        assert_eq!(chunks.concat(), input);
    }

    #[test]
    fn formulas_keep_text_and_missing_cache_becomes_a_diagnostic() {
        let path = temporary_xlsx_path("formula-cache");
        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();
        worksheet.set_name("Calculations").unwrap();
        worksheet
            .write_formula(0, 0, Formula::new("=1+1").set_result(""))
            .unwrap();
        worksheet
            .write_formula(1, 0, Formula::new("=2+2").set_result("4"))
            .unwrap();
        workbook.save(&path).unwrap();
        let path_without_cache = remove_first_formula_cache(&path);

        let restored = read_xlsx(&path_without_cache).unwrap();
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(path_without_cache).unwrap();
        assert!(restored.sheets[0].rows[0][0].formula.is_some());
        assert_eq!(restored.sheets[0].rows[0][0].value, XlsxValue::Empty);
        assert_eq!(restored.sheets[0].rows[1][0].value, XlsxValue::Float64(4.0));
        assert!(restored.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == XlsxDiagnosticCode::FormulaWithoutCachedValue
        }));
    }

    fn temporary_xlsx_path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "plotium-{label}-{}-{}.xlsx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn remove_first_formula_cache(path: &Path) -> std::path::PathBuf {
        let output = path.with_file_name(format!(
            "{}-no-cache.xlsx",
            path.file_stem().unwrap().to_string_lossy()
        ));
        let file = std::fs::File::open(path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut entries = Vec::new();
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).unwrap();
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            entries.push((entry.name().unwrap().into_owned(), entry.is_dir(), bytes));
        }
        let file = std::fs::File::create(&output).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, directory, mut bytes) in entries {
            if name == "xl/worksheets/sheet1.xml" {
                let xml = String::from_utf8(bytes).unwrap();
                bytes = xml.replacen("<v>0</v>", "", 1).into_bytes();
            }
            if directory {
                writer.add_directory(name, options).unwrap();
            } else {
                writer.start_file(name, options).unwrap();
                writer.write_all(&bytes).unwrap();
            }
        }
        writer.finish().unwrap();
        output
    }
}
