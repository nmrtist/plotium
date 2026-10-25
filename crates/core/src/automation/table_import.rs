//! Prepared data-only imports share native tables, actions and the automation audit.
use super::table_read::table_metadata;
use super::*;
use crate::{
    actions::Action,
    delimited::{Delimiter, HeaderMode, ParseOptions},
    state::{Dataset, PlotiumApp, TableDataset, TableImportSource, TypedTableState},
};
use plotium_data::{CodecRegistry, MemoryBlockStore, TableId, TableSchema};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};

pub const TABLE_IMPORT_FILES: usize = 16;
pub const TABLE_IMPORT_FILE_BYTES: u64 = 16 * 1024 * 1024;
pub const TABLE_IMPORT_TOTAL_BYTES: u64 = 64 * 1024 * 1024;
pub const TABLE_IMPORT_COLUMNS: usize = 256;
pub const TABLE_IMPORT_ROWS: usize = 1_000_000;
/// Worksheet ranges are read densely, so XLSX is bounded by cells read rather
/// than by its compressed size.
pub const TABLE_IMPORT_XLSX_CELLS: usize = 8_000_000;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TableImportMode {
    TableDataOnly,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportHeader {
    #[default]
    Auto,
    Present,
    Absent,
}
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportFormat {
    Csv,
    Tsv,
    Xlsx,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TableImportParams {
    pub mode: TableImportMode,
    pub paths: Vec<PathBuf>,
    #[serde(default)]
    pub header: ImportHeader,
    pub format: Option<ImportFormat>,
    /// XLSX worksheets to import by name; every visible worksheet when absent.
    pub sheets: Option<Vec<String>>,
    /// An explicit ordered schema, not display-name-based resource addressing.
    pub schema: Option<TableSchema>,
    pub schema_path: Option<PathBuf>,
}
pub fn is_table_import(value: &Value) -> bool {
    value.get("mode").and_then(Value::as_str) == Some("table_data_only")
}
pub fn table_import_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["mode","paths"],"properties":{
        "mode":{"const":"table_data_only"},"paths":{"type":"array","minItems":1,"maxItems":TABLE_IMPORT_FILES,"items":{"type":"string"}},
        "header":{"enum":["auto","present","absent"],"default":"auto","description":"CSV/TSV only."},"format":{"enum":["csv","tsv","xlsx"]},
        "sheets":{"type":"array","minItems":1,"items":{"type":"string"},"description":"XLSX worksheet names, each imported as its own table. Default: every visible worksheet."},
        "schema":table_declaration_schema(),
        "schema_path":{"type":"string","description":"Explicit PlotiumDelimitedSchemaV1 JSON file for CSV/TSV. Never discovered automatically."}},
        "not":{"required":["schema","schema_path"]}})
}
pub fn table_declaration_schema() -> Value {
    let mut types = ["null","boolean","int64","float64","utf8","date"].iter().map(|kind| json!({"type":"object","required":["kind"],"additionalProperties":false,"properties":{"kind":{"const":kind}}})).collect::<Vec<_>>();
    types.push(json!({"type":"object","required":["kind","display_timezone"],"additionalProperties":false,"properties":{"kind":{"const":"timestamp"},"display_timezone":{"type":"string"}}}));
    let unit = json!({"type":["object","null"],"required":["quantity","dimension","canonical_unit","display_unit","scale","offset"],"properties":{
        "quantity":{"type":"string"},"dimension":{"type":"object","additionalProperties":{"type":"integer","minimum":-128,"maximum":127}},
        "canonical_unit":{"type":"string"},"display_unit":{"type":"string"},"scale":{"type":"number"},"offset":{"type":"number"},"ucum":{"type":["string","null"]},"extension_id":{"type":["string","null"]}}});
    json!({"type":"object","required":["columns"],"properties":{
        "columns":{"type":"array","minItems":1,"maxItems":TABLE_IMPORT_COLUMNS,"items":{"type":"object","required":["id","name","logical_type","nullable"],"properties":{
            "id":{"type":"string","format":"uuid"},"name":{"type":"string"},"logical_type":{"oneOf":types},"nullable":{"type":"boolean"},"unit":unit,"metadata":{"type":"object"},"role":{"description":"Native SemanticRole; value is the default."},"constraints":{"type":"object","properties":{"unique":{"const":false}}}}}},
        "business_keys":{"type":"array","maxItems":0},"metadata":{"type":"object"}}})
}
pub fn table_import_limits() -> Value {
    json!({"files":TABLE_IMPORT_FILES,"file_bytes":TABLE_IMPORT_FILE_BYTES,"total_bytes":TABLE_IMPORT_TOTAL_BYTES,"columns":TABLE_IMPORT_COLUMNS,"rows_per_file":TABLE_IMPORT_ROWS,"xlsx_cells_per_file":TABLE_IMPORT_XLSX_CELLS,"prepared_plans":4,"in_flight":1,"schema_file_bytes":TABLE_RESPONSE_BYTES,"prepared_summary_bytes":TABLE_RESPONSE_BYTES/2,"preflight_response_bytes":TABLE_RESPONSE_BYTES})
}
fn invalid(message: impl Into<String>) -> AutomationError {
    AutomationError::InvalidParameters {
        tool_id: "data.import".into(),
        message: message.into(),
    }
}
pub(super) fn too_many_files(given: usize) -> String {
    format!(
        "{given} paths were given; each data.import call takes at most {TABLE_IMPORT_FILES}. Import the rest in further calls."
    )
}
impl TableImportParams {
    pub fn validate(&self) -> Result<(), AutomationError> {
        if self.paths.len() > TABLE_IMPORT_FILES {
            return Err(invalid(too_many_files(self.paths.len())));
        }
        if self.paths.is_empty() {
            return Err(invalid("Import requires 1–16 explicit files."));
        }
        if self.sheets.as_ref().is_some_and(Vec::is_empty) {
            return Err(invalid("sheets must name at least one worksheet."));
        }
        if self.schema.is_some() && self.schema_path.is_some() {
            return Err(invalid("Supply schema or schema_path, not both."));
        }
        if self.paths.iter().any(|p| !p.is_absolute())
            || self.schema_path.as_ref().is_some_and(|p| !p.is_absolute())
        {
            return Err(invalid("Import paths must be absolute."));
        }
        if let Some(schema) = &self.schema {
            if schema.columns.len() > TABLE_IMPORT_COLUMNS {
                return Err(invalid("Declared schema exceeds 256 columns."));
            }
            schema.validate().map_err(|e| invalid(e.to_string()))?;
        }
        Ok(())
    }
}
struct ImportItem {
    path: PathBuf,
    /// The source worksheet of an XLSX table.
    sheet: Option<String>,
    dataset: Option<Dataset>,
    summary: Value,
    error: Option<String>,
}
impl ImportItem {
    fn json(&self, status: &str) -> Value {
        let mut value =
            json!({"path":self.path,"status":status,"metadata":self.summary,"error":self.error});
        if let Some(sheet) = &self.sheet {
            value["sheet"] = json!(sheet);
        }
        value
    }
}
pub struct PreparedTableImport {
    items: Vec<ImportItem>,
    params: TableImportParams,
}
impl PreparedTableImport {
    /// Reads only the named inputs, never directories or adjacent schema files.
    pub fn prepare(
        params: TableImportParams,
        cancelled: impl Fn() -> bool,
    ) -> Result<Self, AutomationError> {
        params.validate()?;
        let mut budget = TABLE_IMPORT_TOTAL_BYTES;
        let (schema, source) = match (&params.schema, &params.schema_path) {
            (Some(schema), _) => (Some(schema.clone()), "user"),
            (_, Some(path)) => {
                let bytes = read_selected(path, &mut budget, TABLE_RESPONSE_BYTES as u64)
                    .map_err(invalid)?;
                let contract: crate::xlsx::PlotiumDelimitedSchemaV1 =
                    serde_json::from_slice(&bytes)
                        .map_err(|e| invalid(format!("Invalid declared schema: {e}")))?;
                if contract.schema_version != 1 {
                    return Err(invalid("Declared schema must be v1."));
                }
                if !contract.uncertainty.is_empty() {
                    return Err(invalid(
                        "Uncertainty declarations are outside data-only import; import plain columns.",
                    ));
                }
                contract
                    .schema
                    .validate()
                    .map_err(|e| invalid(e.to_string()))?;
                (Some(contract.schema), "file")
            }
            _ => (None, "inferred"),
        };
        let mut items = Vec::new();
        for path in &params.paths {
            if cancelled() {
                return Err(AutomationError::PreviewCancelled);
            }
            match prepare_file(path, &params, schema.clone(), source, &mut budget) {
                Ok(tables) => {
                    for (sheet, dataset) in tables {
                        let (summary, dataset, error) = match dataset {
                            Ok(dataset) => (table_metadata(&dataset)?, Some(dataset), None),
                            Err(error) => (Value::Null, None, Some(error)),
                        };
                        items.push(ImportItem {
                            path: path.clone(),
                            sheet,
                            summary,
                            dataset,
                            error,
                        });
                    }
                }
                Err(error) => items.push(ImportItem {
                    path: path.clone(),
                    sheet: None,
                    dataset: None,
                    summary: Value::Null,
                    error: Some(error),
                }),
            }
        }
        let prepared = Self { items, params };
        if serde_json::to_vec(&prepared.preflight())
            .map_err(|e| invalid(e.to_string()))?
            .len()
            > TABLE_RESPONSE_BYTES / 2
        {
            let mut workbooks: Vec<(&Path, Vec<String>)> = Vec::new();
            for item in &prepared.items {
                let Some(sheet) = &item.sheet else { continue };
                let name = format!("'{sheet}'");
                match workbooks.iter_mut().find(|(path, _)| *path == item.path) {
                    Some((_, names)) => names.push(name),
                    None => workbooks.push((&item.path, vec![name])),
                }
            }
            if workbooks.is_empty() {
                return Err(AutomationError::ResponseTooLarge);
            }
            let listed = workbooks
                .iter()
                .map(|(path, names)| format!("{}: {}", path.display(), names.join(", ")))
                .collect::<Vec<_>>()
                .join("; ");
            return Err(invalid(format!(
                "The import summary exceeds 128 KiB; import one workbook at a time and name fewer worksheets in sheets. Worksheets by file: {listed}."
            )));
        }
        Ok(prepared)
    }
    pub fn preflight(&self) -> Value {
        json!({"phase":"preflight","prepared_files":self.files(|i|i.dataset.is_some()),"failed_files":self.files(|i|i.error.is_some()),"status":if self.status()=="succeeded"{"prepared"}else{self.status()},"items":self.items.iter().map(|item| item.json(if item.error.is_some(){"failed"}else{"prepared"})).collect::<Vec<_>>()})
    }
    /// Counts distinct files with a matching item; a workbook has an item per table.
    fn files(&self, matches: impl Fn(&ImportItem) -> bool) -> usize {
        let mut paths = Vec::new();
        for item in self.items.iter().filter(|item| matches(item)) {
            if !paths.contains(&&item.path) {
                paths.push(&item.path);
            }
        }
        paths.len()
    }
    fn status(&self) -> &'static str {
        let success = self.items.iter().filter(|i| i.dataset.is_some()).count();
        if success == self.items.len() {
            "succeeded"
        } else if success == 0 {
            "failed"
        } else {
            "partial"
        }
    }
    pub(super) fn commit(
        self,
        app: &mut PlotiumApp,
        plan: &ToolPlan,
    ) -> Result<ToolResult, AutomationError> {
        let declared: TableImportParams =
            super::registry::parse("data.import", plan.request.parameters.clone())?;
        if declared != self.params {
            return Err(invalid(
                "Prepared import belongs to a different invocation.",
            ));
        }
        let status = self.status();
        let mut result = super::tool_executors::readonly_value(
            "data.import",
            DocumentRevision(app.doc.automation_revision),
            Value::Null,
        );
        let mut actions = Vec::new();
        let mut values = Vec::new();
        for mut item in self.items {
            let success = item.dataset.is_some();
            if let Some(dataset) = item.dataset.take() {
                result.produced.push(dataset.resource_id().into());
                actions.push(Action::InsertTableData {
                    dataset: Box::new(dataset),
                });
            }
            result.targets.push(TargetResult {
                target: TargetRef::resource(ResourceRef {
                    id: item.path.display().to_string(),
                    kind: ResourceKindId::new(KIND_EXTERNAL_INPUT),
                    parent_id: None,
                    local_id: None,
                }),
                outcome: if success {
                    TargetOutcome::Succeeded
                } else {
                    TargetOutcome::Failed
                },
                message: item.error.clone().unwrap_or_else(|| match &item.sheet {
                    Some(sheet) => format!("Imported worksheet '{sheet}' as native table data."),
                    None => "Imported native table data.".into(),
                }),
                skip_reason: None,
                fingerprints: Vec::new(),
            });
            values.push(item.json(if success { "succeeded" } else { "failed" }));
        }
        if !actions.is_empty() {
            app.try_execute_action(Action::Composite(actions))
                .map_err(|e| AutomationError::Execution(e.to_string()))?;
        }
        result.before_revision = plan.frozen_targets.revision;
        result.after_revision = DocumentRevision(app.doc.automation_revision);
        result.value = json!({"status":status,"items":values});
        Ok(result)
    }
}

/// Prepared payloads are invocation state, never a second persisted document.
pub fn execute_prepared_table_import(
    app: &mut PlotiumApp,
    plan: ToolPlan,
    prepared: PreparedTableImport,
    authority: ExecutionAuthority,
) -> Result<ToolResult, AutomationError> {
    let run = PreparedToolRun::begin(app, &plan);
    let outcome = (|| {
        if plan.request.tool_id != "data.import" || !is_table_import(&plan.request.parameters) {
            return Err(invalid("Prepared table import requires data-only mode."));
        }
        if authority < ExecutionAuthority::ReversibleModify {
            return Err(AutomationError::InsufficientAuthority {
                granted: authority,
                required: ExecutionAuthority::ReversibleModify,
            });
        }
        if plan.request.expected_revision.0 != app.doc.automation_revision {
            return Err(AutomationError::StaleRevision {
                expected: plan.request.expected_revision.0,
                actual: app.doc.automation_revision,
            });
        }
        prepared.commit(app, &plan)
    })();
    run.finish(app, &outcome)?;
    outcome
}
fn read_selected(path: &Path, budget: &mut u64, limit: u64) -> Result<Vec<u8>, String> {
    if !std::fs::metadata(path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .is_file()
    {
        return Err("Only explicitly named regular files may be imported.".into());
    }
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Only explicitly named regular files may be imported.".into());
    }
    let max = limit.min(*budget);
    let mut bytes = Vec::new();
    file.take(max + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    *budget = budget.saturating_sub(bytes.len() as u64);
    if bytes.len() as u64 > max {
        return Err("Selected file exceeds the per-file or remaining batch byte budget.".into());
    }
    Ok(bytes)
}
/// Prepares every table in one file, paired with its XLSX worksheet name.
fn prepare_file(
    path: &Path,
    params: &TableImportParams,
    schema: Option<TableSchema>,
    source: &str,
    budget: &mut u64,
) -> Result<Vec<super::table_import_xlsx::PreparedTable>, String> {
    let format = params
        .format
        .or_else(|| match path.extension().and_then(|s| s.to_str()) {
            Some(ext) if ext.eq_ignore_ascii_case("csv") => Some(ImportFormat::Csv),
            Some(ext) if ext.eq_ignore_ascii_case("tsv") => Some(ImportFormat::Tsv),
            Some(ext) if ext.eq_ignore_ascii_case("xlsx") => Some(ImportFormat::Xlsx),
            _ => None,
        })
        .ok_or("Specify CSV, TSV or XLSX format for this file.")?;
    if format == ImportFormat::Xlsx {
        if params.header != ImportHeader::Auto {
            return Err(
                "header applies to CSV/TSV; XLSX detects each worksheet's header row.".into(),
            );
        }
        if schema.is_some() {
            return Err(
                "A declared schema applies to CSV/TSV; XLSX keeps the column types stored in the workbook."
                    .into(),
            );
        }
        let bytes = read_selected(path, budget, TABLE_IMPORT_FILE_BYTES)?;
        return super::table_import_xlsx::prepare(path, bytes, params.sheets.as_deref());
    }
    if params.sheets.is_some() {
        return Err("sheets applies to XLSX workbooks only.".into());
    }
    let bytes = read_selected(path, budget, TABLE_IMPORT_FILE_BYTES)?;
    let input = std::str::from_utf8(&bytes).map_err(|e| format!("CSV/TSV must be UTF-8: {e}"))?;
    let delimiter = match format {
        ImportFormat::Csv => Delimiter::Comma,
        ImportFormat::Tsv => Delimiter::Tab,
        ImportFormat::Xlsx => unreachable!("XLSX returned above"),
    };
    let parsed = crate::delimited::parse_delimited_columns(
        input,
        ParseOptions {
            delimiter: Some(delimiter),
            header: match params.header {
                ImportHeader::Auto => HeaderMode::Auto,
                ImportHeader::Present => HeaderMode::Present,
                ImportHeader::Absent => HeaderMode::Absent,
            },
        },
        1,
    )
    .map_err(|e| e.to_string())?;
    if parsed.rows.len() > TABLE_IMPORT_ROWS
        || parsed
            .rows
            .first()
            .is_some_and(|r| r.cells.len() > TABLE_IMPORT_COLUMNS)
    {
        return Err("Selected table exceeds the row or column budget.".into());
    }
    let store = Arc::new(MemoryBlockStore::default());
    let snapshot = parsed
        .into_declared_snapshot(
            TableId::new(),
            store.as_ref(),
            &CodecRegistry::with_arrow_ipc(),
            schema,
            source,
        )
        .map_err(|e| e.to_string())?;
    let mut table = TableDataset::from_typed(
        TypedTableState::imported(snapshot, store).map_err(|e| e.to_string())?,
    );
    table.name = Some(
        path.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
    );
    let mut retained = TableImportSource::new(
        Arc::<[u8]>::from(bytes),
        match format {
            ImportFormat::Csv => "text/csv",
            ImportFormat::Tsv => "text/tab-separated-values",
            ImportFormat::Xlsx => unreachable!("XLSX returned above"),
        },
    );
    retained.name = path.file_name().map(|s| s.to_string_lossy().into_owned());
    table.import_sources.push(retained);
    Ok(vec![(None, Ok(Dataset::Table(Box::new(table))))])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn too_many_paths_say_how_many_and_to_import_in_batches() {
        let paths: Vec<_> = (0..17)
            .map(|i| PathBuf::from(format!("/data/{i}.csv")))
            .collect();
        let table: TableImportParams =
            serde_json::from_value(json!({"mode": "table_data_only", "paths": paths})).unwrap();
        let native = NativeImportParams {
            paths,
            sampling_declaration: None,
        };
        for error in [table.validate(), native.validate()] {
            let message = error.unwrap_err().to_string();
            assert!(message.contains("17 paths"), "{message}");
            assert!(message.contains("at most 16"), "{message}");
            assert!(message.contains("further calls"), "{message}");
        }
    }
}
