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
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TableImportParams {
    pub mode: TableImportMode,
    pub paths: Vec<PathBuf>,
    #[serde(default)]
    pub header: ImportHeader,
    pub format: Option<ImportFormat>,
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
        "header":{"enum":["auto","present","absent"],"default":"auto"},"format":{"enum":["csv","tsv"]},
        "schema":table_declaration_schema(),
        "schema_path":{"type":"string","description":"Explicit PlotiumDelimitedSchemaV1 JSON file. Never discovered automatically."}},
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
    json!({"files":TABLE_IMPORT_FILES,"file_bytes":TABLE_IMPORT_FILE_BYTES,"total_bytes":TABLE_IMPORT_TOTAL_BYTES,"columns":TABLE_IMPORT_COLUMNS,"rows_per_file":TABLE_IMPORT_ROWS,"prepared_plans":4,"in_flight":1,"schema_file_bytes":TABLE_RESPONSE_BYTES,"prepared_summary_bytes":TABLE_RESPONSE_BYTES/2,"preflight_response_bytes":TABLE_RESPONSE_BYTES})
}
fn invalid(message: impl Into<String>) -> AutomationError {
    AutomationError::InvalidParameters {
        tool_id: "data.import".into(),
        message: message.into(),
    }
}
impl TableImportParams {
    pub fn validate(&self) -> Result<(), AutomationError> {
        if self.paths.is_empty() || self.paths.len() > TABLE_IMPORT_FILES {
            return Err(invalid("Import requires 1–16 explicit files."));
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
    dataset: Option<Dataset>,
    summary: Value,
    error: Option<String>,
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
            let result = prepare_file(path, &params, schema.clone(), source, &mut budget);
            let (dataset, summary, error) = match result {
                Ok(dataset) => {
                    let summary = table_metadata(&dataset)?;
                    (Some(dataset), summary, None)
                }
                Err(error) => (None, Value::Null, Some(error)),
            };
            items.push(ImportItem {
                path: path.clone(),
                dataset,
                summary,
                error,
            });
        }
        let prepared = Self { items, params };
        if serde_json::to_vec(&prepared.preflight())
            .map_err(|e| invalid(e.to_string()))?
            .len()
            > TABLE_RESPONSE_BYTES / 2
        {
            return Err(AutomationError::ResponseTooLarge);
        }
        Ok(prepared)
    }
    pub fn preflight(&self) -> Value {
        json!({"phase":"preflight","prepared_files":self.items.iter().filter(|i|i.dataset.is_some()).count(),"failed_files":self.items.iter().filter(|i|i.error.is_some()).count(),"status":if self.status()=="succeeded"{"prepared"}else{self.status()},"items":self.items.iter().map(|item| json!({"path":item.path,"status":if item.error.is_some(){"failed"}else{"prepared"},"metadata":item.summary,"error":item.error})).collect::<Vec<_>>()})
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
        for item in self.items {
            let success = item.dataset.is_some();
            if let Some(dataset) = item.dataset {
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
                message: item
                    .error
                    .clone()
                    .unwrap_or_else(|| "Imported native table data.".into()),
                skip_reason: None,
                fingerprints: Vec::new(),
            });
            values.push(json!({"path":item.path,"status":if success{"succeeded"}else{"failed"},"metadata":item.summary,"error":item.error}));
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
fn prepare_file(
    path: &Path,
    params: &TableImportParams,
    schema: Option<TableSchema>,
    source: &str,
    budget: &mut u64,
) -> Result<Dataset, String> {
    let format = params
        .format
        .or_else(|| match path.extension().and_then(|s| s.to_str()) {
            Some(ext) if ext.eq_ignore_ascii_case("csv") => Some(ImportFormat::Csv),
            Some(ext) if ext.eq_ignore_ascii_case("tsv") => Some(ImportFormat::Tsv),
            _ => None,
        })
        .ok_or("Specify CSV or TSV format for this file.")?;
    let bytes = read_selected(path, budget, TABLE_IMPORT_FILE_BYTES)?;
    let input = std::str::from_utf8(&bytes).map_err(|e| format!("CSV/TSV must be UTF-8: {e}"))?;
    let delimiter = match format {
        ImportFormat::Csv => Delimiter::Comma,
        ImportFormat::Tsv => Delimiter::Tab,
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
        },
    );
    retained.name = path.file_name().map(|s| s.to_string_lossy().into_owned());
    table.import_sources.push(retained);
    Ok(Dataset::Table(Box::new(table)))
}
