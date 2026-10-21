//! Frozen native imports use the same loaders and undo lifecycle as the desktop.
use super::*;
use crate::{
    actions::Action,
    state::{CanvasDocument, PlotiumApp},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{io::Write, path::PathBuf};

pub const NATIVE_IMPORT_BYTES: usize = 512 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeImportParams {
    pub paths: Vec<PathBuf>,
    #[serde(default)]
    pub sampling_declaration: Option<plotium_io::nmr_sampling::SamplingDeclaration>,
}
fn invalid(message: impl Into<String>) -> AutomationError {
    AutomationError::InvalidParameters {
        tool_id: "data.import".into(),
        message: message.into(),
    }
}
impl NativeImportParams {
    pub fn validate(&self) -> Result<(), AutomationError> {
        if self.paths.is_empty()
            || self.paths.len() > 16
            || self.paths.iter().any(|p| !p.is_absolute())
        {
            return Err(invalid(
                "Native import requires 1–16 absolute explicit data files or atomic acquisition directories.",
            ));
        }
        Ok(())
    }
}
pub fn data_import_schema() -> Value {
    let native = json!({"type":"object","additionalProperties":false,"required":["paths"],"properties":{
        "paths":{"type":"array","minItems":1,"maxItems":16,"items":{"type":"string"}},
        "sampling_declaration":{"type":"object"}}});
    let table = table_import_schema();
    let mut combined = native.clone();
    combined["properties"]
        .as_object_mut()
        .expect("local properties")
        .extend(
            table["properties"]
                .as_object()
                .expect("local properties")
                .clone(),
        );
    combined["oneOf"] = json!([native, table]);
    combined
}
pub fn native_import_limits() -> Value {
    json!({"inputs":16,"in_flight":1,"prepared_plans":4,"frozen_payload_bytes":NATIVE_IMPORT_BYTES,"response_bytes":TABLE_RESPONSE_BYTES})
}
pub enum DataImportParams {
    Table(TableImportParams),
    Native(NativeImportParams),
}
impl DataImportParams {
    pub fn parse(value: Value) -> Result<Self, AutomationError> {
        if is_table_import(&value) {
            let p: TableImportParams = super::registry::parse("data.import", value)?;
            p.validate()?;
            Ok(Self::Table(p))
        } else {
            let p: NativeImportParams = super::registry::parse("data.import", value)?;
            p.validate()?;
            Ok(Self::Native(p))
        }
    }
}
pub enum PreparedDataImport {
    Table(PreparedTableImport),
    Native(PreparedNativeImport),
}
impl From<PreparedTableImport> for PreparedDataImport {
    fn from(value: PreparedTableImport) -> Self {
        Self::Table(value)
    }
}
impl PreparedDataImport {
    pub fn prepare(
        params: DataImportParams,
        cancelled: impl Fn() -> bool,
    ) -> Result<Self, AutomationError> {
        Self::prepare_with_budget(params, NATIVE_IMPORT_BYTES, cancelled)
    }
    pub fn prepare_with_budget(
        params: DataImportParams,
        budget: usize,
        cancelled: impl Fn() -> bool,
    ) -> Result<Self, AutomationError> {
        match params {
            DataImportParams::Table(p) => {
                Ok(Self::Table(PreparedTableImport::prepare(p, cancelled)?))
            }
            DataImportParams::Native(p) => Ok(Self::Native(PreparedNativeImport::prepare(
                p,
                budget.min(NATIVE_IMPORT_BYTES),
                cancelled,
            )?)),
        }
    }
    pub fn preflight(&self) -> Value {
        match self {
            Self::Table(p) => p.preflight(),
            Self::Native(p) => p.preflight(),
        }
    }
    pub fn reserved_bytes(&self) -> usize {
        match self {
            Self::Table(_) => 0,
            Self::Native(p) => p.reserved_bytes,
        }
    }
    pub(super) fn commit(
        self,
        app: &mut PlotiumApp,
        plan: &ToolPlan,
    ) -> Result<ToolResult, AutomationError> {
        match self {
            Self::Table(p) => p.commit(app, plan),
            Self::Native(p) => p.commit(app, plan),
        }
    }
}
struct NativeItem {
    path: PathBuf,
    loaded: Option<crate::workflow::LoadedDataset>,
    canvas: Option<CanvasDocument>,
    metadata: Value,
    error: Option<String>,
}
pub struct PreparedNativeImport {
    params: NativeImportParams,
    items: Vec<NativeItem>,
    reserved_bytes: usize,
}
impl PreparedNativeImport {
    fn prepare(
        params: NativeImportParams,
        budget: usize,
        cancelled: impl Fn() -> bool,
    ) -> Result<Self, AutomationError> {
        params.validate()?;
        let mut prepared = Self {
            params,
            items: Vec::new(),
            reserved_bytes: 0,
        };
        for path in &prepared.params.paths {
            if cancelled() {
                return Err(AutomationError::PreviewCancelled);
            }
            let result = plotium_io::resolve_explicit_input(path)
                .map_err(crate::workflow::WorkflowError::from)
                .and_then(|input| {
                    prepared.params.sampling_declaration.clone().map_or_else(
                        || crate::workflow::load_dataset(&input),
                        |declaration| {
                            crate::workflow::load_dataset_with_sampling(&input, declaration)
                        },
                    )
                });
            let item = match result {
                Ok(loaded) => {
                    let canvas_name = path
                        .file_stem()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "Imported Data".into());
                    let canvas = crate::workflow::build_default_canvas_for_dataset(
                        &loaded.dataset,
                        0,
                        canvas_name,
                        crate::state::DEFAULT_CANVAS_SIZE_MM,
                    );
                    let bytes = reservation(&loaded.dataset, &canvas)?;
                    prepared.reserved_bytes = prepared
                        .reserved_bytes
                        .checked_add(bytes)
                        .ok_or_else(|| invalid("Native frozen payload budget overflow."))?;
                    if prepared.reserved_bytes > budget {
                        return Err(AutomationError::NativeImportBudget(
                            "Frozen payload exceeds the remaining native import budget (512 MiB shared); import a smaller batch.".into(),
                        ));
                    }
                    let metadata = json!({"dataset_id":loaded.dataset.resource_id(),"canvas_id":canvas.resource_id,"inspection":loaded.inspection});
                    NativeItem {
                        path: path.clone(),
                        loaded: Some(loaded),
                        canvas: Some(canvas),
                        metadata,
                        error: None,
                    }
                }
                Err(e) => NativeItem {
                    path: path.clone(),
                    loaded: None,
                    canvas: None,
                    metadata: Value::Null,
                    error: Some(e.to_string()),
                },
            };
            prepared.items.push(item);
        }
        if cancelled() {
            return Err(AutomationError::PreviewCancelled);
        }
        if serde_json::to_vec(&prepared.preflight())
            .map_err(|e| invalid(e.to_string()))?
            .len()
            > TABLE_RESPONSE_BYTES / 2
        {
            return Err(AutomationError::ResponseTooLarge);
        }
        Ok(prepared)
    }
    fn status(&self) -> &'static str {
        let count = self.items.iter().filter(|i| i.loaded.is_some()).count();
        if count == 0 {
            "failed"
        } else if count == self.items.len() {
            "succeeded"
        } else {
            "partial"
        }
    }
    fn preflight(&self) -> Value {
        json!({"phase":"preflight","status":if self.status()=="succeeded" {"prepared"} else {self.status()},"prepared_files":self.items.iter().filter(|i|i.loaded.is_some()).count(),"failed_files":self.items.iter().filter(|i|i.error.is_some()).count(),"reserved_bytes":self.reserved_bytes,"items":self.items.iter().map(|i|json!({"path":i.path,"status":if i.loaded.is_some(){"prepared"}else{"failed"},"metadata":i.metadata,"error":i.error})).collect::<Vec<_>>()})
    }
    fn commit(self, app: &mut PlotiumApp, plan: &ToolPlan) -> Result<ToolResult, AutomationError> {
        let declared: NativeImportParams =
            super::registry::parse("data.import", plan.request.parameters.clone())?;
        if declared != self.params {
            return Err(invalid("Prepared import belongs to another invocation."));
        }
        let mut result = super::tool_executors::readonly_value(
            "data.import",
            DocumentRevision(app.doc.automation_revision),
            Value::Null,
        );
        let status = self.status();
        let mut actions = Vec::new();
        let mut values = Vec::new();
        let active_canvas = app.session.active_canvas;
        let selection = app.session.ui.data_selection.clone();
        let view = app.session.view;
        for item in self.items {
            let success = item.loaded.is_some();
            if let (Some(loaded), Some(canvas)) = (item.loaded, item.canvas) {
                result.produced.push(loaded.dataset.resource_id().into());
                result.produced.push(ResourceRef {
                    id: canvas.resource_id.to_string(),
                    kind: ResourceKindId::new(KIND_CANVAS),
                    parent_id: None,
                    local_id: None,
                });
                let offset = actions.len();
                actions.push(Action::InsertDatasetWithCanvas {
                    dataset_index: app.doc.datasets.len() + offset,
                    canvas_index: app.doc.canvases.len() + offset,
                    canvas_resource_id: canvas.resource_id,
                    prepared_canvas: Some(Box::new(canvas)),
                    dataset: Box::new(loaded.dataset),
                    canvas_name: loaded.source,
                    size_mm: crate::state::DEFAULT_CANVAS_SIZE_MM,
                    active_canvas_before: active_canvas,
                    active_dataset_before: app.active_dataset(),
                    inserted_into_existing_canvas: None,
                    inserted_object_id: None,
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
                    .unwrap_or_else(|| "Imported native dataset and default canvas.".into()),
                skip_reason: None,
                fingerprints: Vec::new(),
            });
            values.push(json!({"path":item.path,"status":if success{"succeeded"}else{"failed"},"metadata":item.metadata,"error":item.error}));
        }
        if !actions.is_empty() {
            app.try_execute_action(Action::Composite(actions))
                .map_err(|e| AutomationError::Execution(e.to_string()))?;
            app.session.active_canvas = active_canvas;
            app.session.ui.data_selection = selection;
            app.session.view = view;
        }
        app.session.status = format!("Native import {status}.");
        if let Some(error) = values.iter().find_map(|v| v["error"].as_str()) {
            app.session.status.push_str(&format!(" {error}"));
        }
        result.before_revision = plan.frozen_targets.revision;
        result.after_revision = DocumentRevision(app.doc.automation_revision);
        result.value = json!({"status":status,"items":values});
        Ok(result)
    }
}

// Count canonical payloads without copying sample buffers. Materialized NMR
// outputs can exceed the acquisition shape (zero filling/NUS); charge those
// separately, then reserve headroom for native caches and container overhead.
fn reservation(
    dataset: &crate::state::Dataset,
    canvas: &CanvasDocument,
) -> Result<usize, AutomationError> {
    struct Counter(usize);
    impl Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self.0.saturating_add(bytes.len());
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let objects = crate::project::dataset_to_objects(dataset, "data", "recipe")
        .map_err(|e| AutomationError::Execution(e.to_string()))?;
    let mut counter = Counter(std::mem::size_of_val(dataset));
    crate::project::write_dataset_payload(&mut counter, &objects.blob)
        .map_err(|e| AutomationError::Execution(e.to_string()))?;
    let processed_2d_bytes = |data: &plotium_processing::Processed2D| match data {
        plotium_processing::Processed2D::Ft(s) => s
            .data
            .capacity()
            .saturating_mul(16)
            .saturating_add(s.f1_ppm.capacity().saturating_mul(8))
            .saturating_add(s.f2_ppm.capacity().saturating_mul(8))
            .saturating_add(
                s.magnitude_plane
                    .as_ref()
                    .map_or(0, |v| v.len().saturating_mul(8)),
            ),
        plotium_processing::Processed2D::Stack(s) => s
            .traces
            .iter()
            .fold(s.ppm.capacity().saturating_mul(8), |sum, v| {
                sum.saturating_add(v.capacity().saturating_mul(16))
            }),
    };
    match dataset {
        crate::state::Dataset::Nmr(data) => {
            for source in [&data.native_base, &data.native_processed] {
                crate::project::write_dataset_payload(
                    &mut counter,
                    &crate::project::DatasetBlob::Nmr(source.dataset()),
                )
                .map_err(|e| AutomationError::Execution(e.to_string()))?;
            }
            for output in [&data.base, &data.processed] {
                counter.0 = counter
                    .0
                    .saturating_add(output.values().len().saturating_mul(24));
            }
        }
        crate::state::Dataset::Nmr2D(data) => {
            for source in [&data.native_base, &data.native_processed] {
                crate::project::write_dataset_payload(
                    &mut counter,
                    &crate::project::DatasetBlob::Nmr(source.dataset()),
                )
                .map_err(|e| AutomationError::Execution(e.to_string()))?;
            }
            counter.0 = counter
                .0
                .saturating_add(processed_2d_bytes(&data.base))
                .saturating_add(processed_2d_bytes(&data.processed));
            serde_json::to_writer(&mut counter, data.processed_figure.as_ref())
                .map_err(|e| invalid(e.to_string()))?;
        }
        _ => {}
    }
    for (_, bytes) in &objects.extra_blobs {
        counter.0 = counter.0.saturating_add(bytes.len());
    }
    let view = crate::project::canvas_to_view(std::slice::from_ref(dataset), canvas, "view")
        .map_err(|e| AutomationError::Execution(e.to_string()))?;
    for value in [json!(objects.data), json!(objects.recipe), json!(view)] {
        serde_json::to_writer(&mut counter, &value).map_err(|e| invalid(e.to_string()))?;
    }
    Ok(counter.0.saturating_mul(4))
}

pub fn execute_prepared_data_import(
    app: &mut PlotiumApp,
    plan: ToolPlan,
    prepared: PreparedDataImport,
    authority: ExecutionAuthority,
) -> Result<ToolResult, AutomationError> {
    let run = PreparedToolRun::begin(app, &plan);
    let outcome = (|| {
        if plan.request.tool_id != "data.import" {
            return Err(invalid("Expected data.import."));
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
