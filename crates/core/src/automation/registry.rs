use super::*;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

pub struct ToolRegistry {
    descriptors: BTreeMap<String, ToolDescriptor>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::built_in()
    }
}

impl ToolRegistry {
    pub fn built_in() -> Self {
        Self {
            descriptors: descriptors()
                .into_iter()
                .map(|item| (item.id.clone(), item))
                .collect(),
        }
    }
    pub fn descriptors(&self) -> impl Iterator<Item = &ToolDescriptor> {
        self.descriptors.values()
    }
    pub fn get(&self, id: &str) -> Option<&ToolDescriptor> {
        self.descriptors.get(id)
    }
    pub fn validate_unique(&self) -> Result<(), AutomationError> {
        let all = descriptors();
        let ids = all
            .iter()
            .map(|item| item.id.as_str())
            .collect::<BTreeSet<_>>();
        (ids.len() == all.len()).then_some(()).ok_or_else(|| {
            AutomationError::InvalidWorkflow(
                "tool registry contains a duplicate tool id".to_owned(),
            )
        })
    }
}

pub(super) fn validate_parameters(
    tool: &str,
    value: serde_json::Value,
) -> Result<(), AutomationError> {
    match tool {
        FIELD_PLOT_CREATE | DIFFUSION_PLOT_CREATE => {
            parse::<ScientificPlotParams>(tool, value).map(drop)
        }
        REGIONS_EXTRACT => parse::<RegionExtractParams>(tool, value).map(drop),
        DIFFUSION_FIT => parse::<DiffusionFitParams>(tool, value).map(drop),
        DOSY_BUILD => parse::<DosyBuildParams>(tool, value).map(drop),
        ANALYSIS_RESULTS_INSPECT => parse::<AnalysisResultsParams>(tool, value).map(drop),
        DIFFUSION_INSPECT => parse::<DiffusionInspectParams>(tool, value).map(drop),
        DIFFUSION_SET_INPUT => parse::<DiffusionSetInputParams>(tool, value).map(drop),
        PROCESSING_EDIT => parse::<ProcessingEditParams>(tool, value).map(drop),
        PROCESSING_INSPECT => parse::<EmptyParams>(tool, value).map(drop),
        "project.get_blueprint" | "resources.inspect" | "render.preview" => {
            parse::<EmptyParams>(tool, value).map(drop)
        }
        FIGURE_DIAGNOSE => parse::<FigureDiagnosticParams>(tool, value).map(drop),
        "render.preview_image" => parse::<ImagePreviewParams>(tool, value).map(drop),
        "document.inspect" => parse::<DocumentInspectParams>(tool, value).map(drop),
        "resources.search" => parse::<SearchParams>(tool, value).map(drop),
        "data.preview" => parse::<PreviewParams>(tool, value).map(drop),
        "results.compare" => parse::<CompareParams>(tool, value).map(drop),
        CURVE_APPEND => parse::<crate::state::CurveAppendParams>(tool, value).map(drop),
        PLOT_CREATE => parse::<crate::state::PlotCreateParams>(tool, value).map(drop),
        FIGURE_EDIT => parse::<crate::state::FigureEditParams>(tool, value).map(drop),
        CURVE_BIND => parse::<CurveBindParams>(tool, value).map(drop),
        CURVE_MATERIALIZE => parse::<CurveMaterializeParams>(tool, value).map(drop),
        "resource.rename" => parse::<RenameParams>(tool, value).map(drop),
        "figure.apply_theme" => parse::<ThemeParams>(tool, value).map(drop),
        "processing.apply_scheme" => parse::<SchemeParams>(tool, value).map(drop),
        TABLE_PATCH => parse::<TablePatchParams>(tool, value).map(drop),
        TABLE_REFRESH => parse::<TableRefreshParams>(tool, value).map(drop),
        TABLE_COLUMNS => parse::<TableColumnsParams>(tool, value).map(drop),
        TABLE_READ_ROWS => parse::<TableReadRowsParams>(tool, value).map(drop),
        "data.import" if is_table_import(&value) => {
            parse::<TableImportParams>(tool, value)?.validate()
        }
        "data.import" => parse::<NativeImportParams>(tool, value)?.validate(),
        "data.transform" if is_table_transform(&value) => parse_table_transform(value).map(drop),
        "data.transform" => parse::<TransformParams>(tool, value).map(drop),
        "figure.export" => parse::<ExportParams>(tool, value).map(drop),
        "properties.inspect" | "properties.reset" => {
            parse::<super::properties::PropertyKeyParams>(tool, value).map(drop)
        }
        "properties.set" => parse::<super::properties::PropertyWriteParams>(tool, value).map(drop),
        super::property_batch::TOOL_SET_MANY => super::property_batch::decode(&value).map(drop),
        _ => Err(AutomationError::UnknownTool(tool.to_owned())),
    }
}

pub(super) fn parse<T: DeserializeOwned>(
    tool: &str,
    value: serde_json::Value,
) -> Result<T, AutomationError> {
    serde_json::from_value(value).map_err(|error| AutomationError::InvalidParameters {
        tool_id: tool.to_owned(),
        message: error.to_string(),
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EmptyParams {}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SearchParams {
    pub query: ResourceQuery,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PreviewParams {
    #[serde(default = "preview_limit")]
    pub limit: usize,
}
fn preview_limit() -> usize {
    100
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CompareParams {
    #[serde(default)]
    pub before: Vec<ResourceDescriptor>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RenameParams {
    pub name: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ThemeParams {
    pub theme_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SchemeParams {
    pub path: PathBuf,
    #[serde(default)]
    pub compatible_only: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TransformParams {
    pub plan: plotium_data::RelPlanV1,
    pub name: String,
    #[serde(default = "default_table_memory_bytes")]
    pub memory_limit_bytes: u64,
}
fn default_table_memory_bytes() -> u64 {
    512 * 1024 * 1024
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExportParams {
    pub directory: PathBuf,
    pub format: String,
    #[serde(default = "default_dpi")]
    pub dpi: u16,
    #[serde(default)]
    pub overwrite: bool,
}
fn default_dpi() -> u16 {
    crate::export::DEFAULT_BITMAP_DPI
}

trait V1Schema {
    fn schema() -> serde_json::Value;
}
macro_rules! schema {
    ($type:ty, [$($required:literal => $rkind:literal),*], [$($optional:literal => $okind:literal),*]) => {
        impl V1Schema for $type {
            fn schema() -> serde_json::Value { object_schema(&[$(($required, $rkind)),*], &[$(($optional, $okind)),*]) }
        }
    };
}
schema!(EmptyParams, [], []);
schema!(SearchParams, ["query" => "object"], []);
schema!(PreviewParams, [], ["limit" => "integer"]);
schema!(CompareParams, [], ["before" => "array"]);
schema!(RenameParams, ["name" => "string"], []);
schema!(ThemeParams, ["theme_id" => "string"], []);
schema!(SchemeParams, ["path" => "string"], ["compatible_only" => "boolean"]);
impl V1Schema for NativeImportParams {
    fn schema() -> serde_json::Value {
        data_import_schema()
    }
}
impl V1Schema for TableColumnsParams {
    fn schema() -> serde_json::Value {
        table_read_schema(false)
    }
}
impl V1Schema for TableReadRowsParams {
    fn schema() -> serde_json::Value {
        table_read_schema(true)
    }
}
impl V1Schema for TransformParams {
    fn schema() -> serde_json::Value {
        let legacy = object_schema(
            &[("plan", "object"), ("name", "string")],
            &[("memory_limit_bytes", "integer")],
        );
        let table = table_transform_schema();
        let mut combined = legacy.clone();
        combined["properties"]
            .as_object_mut()
            .expect("local properties")
            .extend(
                table["properties"]
                    .as_object()
                    .expect("local properties")
                    .clone(),
            );
        combined["properties"]["plan"] = legacy["properties"]["plan"].clone();
        combined["properties"]["memory_limit_bytes"] =
            legacy["properties"]["memory_limit_bytes"].clone();
        combined["$defs"] = table["$defs"].clone();
        combined["oneOf"] = serde_json::json!([legacy, table]);
        combined
    }
}
schema!(ExportParams, ["directory" => "string", "format" => "string"], ["dpi" => "integer", "overwrite" => "boolean"]);
schema!(super::properties::PropertyKeyParams, ["key" => "string"], []);
// `value` has no single JSON type: which one is admissible is declared by the
// addressed property's own `ValueSchema` and is therefore decided per call, not
// per tool. Announcing one type here would be wrong for every other property.
impl V1Schema for super::properties::PropertyWriteParams {
    fn schema() -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "key": {"type": "string"},
                "value": {"description": "typed per the addressed property's value schema"},
            },
            "required": ["key", "value"],
            "additionalProperties": false,
        })
    }
}

struct Spec {
    id: &'static str,
    title: &'static str,
    description: &'static str,
    schema: serde_json::Value,
    kinds: &'static [&'static str],
    capabilities: &'static [&'static str],
    effect: EffectLevel,
    deterministic: bool,
}

macro_rules! tool {
    ($id:literal, $title:literal, $description:literal, $params:ty, [$($kind:expr),*], [$($cap:expr),*], $effect:expr, $deterministic:literal) => {
        Spec { id: $id, title: $title, description: $description, schema: <$params>::schema(), kinds: &[$($kind),*], capabilities: &[$($cap),*], effect: $effect, deterministic: $deterministic }
    };
}

impl V1Schema for FigureDiagnosticParams {
    fn schema() -> serde_json::Value {
        figure_diagnostic_schema()
    }
}

impl V1Schema for ImagePreviewParams {
    fn schema() -> serde_json::Value {
        image_preview_schema()
    }
}

impl V1Schema for DocumentInspectParams {
    fn schema() -> serde_json::Value {
        document_inspect_schema()
    }
}

impl V1Schema for crate::state::CurveAppendParams {
    fn schema() -> serde_json::Value {
        curve_append_schema()
    }
}
impl V1Schema for CurveBindParams {
    fn schema() -> serde_json::Value {
        curve_binding_schema(false)
    }
}
impl V1Schema for CurveMaterializeParams {
    fn schema() -> serde_json::Value {
        curve_binding_schema(true)
    }
}
impl V1Schema for crate::state::PlotCreateParams {
    fn schema() -> serde_json::Value {
        plot_create_schema()
    }
}
impl V1Schema for crate::state::FigureEditParams {
    fn schema() -> serde_json::Value {
        figure_edit_schema()
    }
}
impl V1Schema for TablePatchParams {
    fn schema() -> serde_json::Value {
        table_patch_schema()
    }
}
impl V1Schema for TableRefreshParams {
    fn schema() -> serde_json::Value {
        table_refresh_schema()
    }
}
fn descriptors() -> Vec<ToolDescriptor> {
    vec![
        tool!("figure.diagnose", "Observe figure layout", "Bounded native geometry observations with explicit coverage at a physical output size", FigureDiagnosticParams, [KIND_CANVAS], [CAP_RENDER], EffectLevel::ReadOnly, true),
        tool!("plot.series.append", "Append explicit curves", "Atomically append selected curves to an existing explicit plot", crate::state::CurveAppendParams, [KIND_CANVAS_OBJECT], [], EffectLevel::Reversible, true),
        tool!("table.patch", "Patch existing table cells", "Atomic typed cell patch by stable RowId and ColumnId", TablePatchParams, [KIND_DATASET], [], EffectLevel::Reversible, true),
        tool!("table.refresh", "Refresh a derived table", "Explicitly rerun a saved native recipe on current source revisions", TableRefreshParams, [KIND_DATASET], [], EffectLevel::Reversible, true),
        tool!("figure.edit", "Edit native figure layout and content", "Atomic page, Panel layout, content-local frames, text and frozen local image creation/replacement", crate::state::FigureEditParams, [KIND_CANVAS], [], EffectLevel::Reversible, true),
        tool!("plot.create", "Create native single plot", "Atomically create a native single plot and required containers using explicit table curves", crate::state::PlotCreateParams,
            [KIND_DOCUMENT,KIND_CANVAS,KIND_PANEL], [], EffectLevel::Reversible, true),
        tool!(
            "plot.series.bind",
            "Bind curve columns",
            "Atomically rebind exact native Series without editing table defaults",
            CurveBindParams,
            [KIND_CANVAS_OBJECT],
            [],
            EffectLevel::Reversible,
            true
        ),
        tool!(
            "plot.series.materialize_default",
            "Materialize default curves",
            "Expand existing default table curves into stable per-curve bindings",
            CurveMaterializeParams,
            [KIND_CANVAS_OBJECT],
            [],
            EffectLevel::Reversible,
            true
        ),
        tool!(
            "render.preview_image",
            "Preview canvas image",
            "Render an exact canvas snapshot to bounded in-memory PNG",
            ImagePreviewParams,
            [KIND_CANVAS],
            [CAP_RENDER],
            EffectLevel::ReadOnly,
            true
        ),
        tool!(
            "document.inspect",
            "Inspect document structure",
            "Read bounded structure and source bindings for an exact target",
            DocumentInspectParams,
            [],
            [],
            EffectLevel::ReadOnly,
            true
        ),
        tool!(
            "project.get_blueprint",
            "Get project blueprint",
            "Read compact project structure",
            EmptyParams,
            [],
            [],
            EffectLevel::ReadOnly,
            true
        ),
        tool!(
            "resources.search",
            "Search resources",
            "Query resources by kind and capability",
            SearchParams,
            [],
            [],
            EffectLevel::ReadOnly,
            true
        ),
        tool!(
            "resources.inspect",
            "Inspect resources",
            "Read resource descriptors",
            EmptyParams,
            [],
            [],
            EffectLevel::ReadOnly,
            true
        ),
        tool!(
            "data.preview",
            "Preview data",
            "Read a bounded data slice and statistics",
            PreviewParams,
            [],
            [CAP_PREVIEW],
            EffectLevel::ReadOnly,
            true
        ),
        tool!(
            "render.preview",
            "Render preview",
            "Render a Plotium canvas to SVG",
            EmptyParams,
            [KIND_CANVAS],
            [CAP_RENDER],
            EffectLevel::ReadOnly,
            true
        ),
        tool!(
            "results.compare",
            "Compare results",
            "Compare resource descriptions",
            CompareParams,
            [],
            [],
            EffectLevel::ReadOnly,
            true
        ),
        tool!(
            "resource.rename",
            "Rename resource",
            "Rename resources that expose the generic rename capability",
            RenameParams,
            [],
            [CAP_RENAME],
            EffectLevel::Reversible,
            true
        ),
        tool!(
            "figure.apply_theme",
            "Apply theme",
            "Apply a registered Plotium theme",
            ThemeParams,
            [KIND_CANVAS],
            [CAP_THEME],
            EffectLevel::Reversible,
            true
        ),
        tool!(
            "processing.apply_scheme",
            "Apply processing scheme",
            "Apply a .plotiumproc parameter resource",
            SchemeParams,
            [KIND_DATASET],
            [CAP_PROCESSING_SCHEME],
            EffectLevel::Reversible,
            true
        ),
        tool!(
            "data.import",
            "Import data",
            "Import canonical datasets into the project",
            NativeImportParams,
            [],
            [],
            EffectLevel::Reversible,
            false
        ),
        tool!(
            "table.columns",
            "Table columns",
            "Inspect native columns and source metadata",
            TableColumnsParams,
            [KIND_DATASET],
            [],
            EffectLevel::ReadOnly,
            true
        ),
        tool!(
            "table.read_rows",
            "Read table rows",
            "Read a bounded page at an explicit table revision",
            TableReadRowsParams,
            [KIND_DATASET],
            [],
            EffectLevel::ReadOnly,
            true
        ),
        tool!(
            "data.transform",
            "Transform typed tables",
            "Execute a frozen Plotium RelPlanV1 against selected typed tables",
            TransformParams,
            [KIND_DATASET],
            [CAP_TRANSFORM],
            EffectLevel::Reversible,
            true
        ),
        tool!(
            "figure.export",
            "Export figures",
            "Export canvases with the Plotium renderer",
            ExportParams,
            [KIND_CANVAS],
            [CAP_EXPORT],
            EffectLevel::ExternalWrite,
            true
        ),
        // The property tools are admitted by capability, never by object type:
        // any resource that exposes `CAP_PROPERTY_CATALOG` gains them, and a
        // canvas object without a plot binding is skipped by the same shared
        // gate that skips a dataset for `figure.apply_theme`.
        tool!(
            "properties.inspect",
            "Inspect a property",
            "Read one catalog property across the components of the selected resources",
            super::properties::PropertyKeyParams,
            [
                KIND_APP,
                KIND_DOCUMENT,
                KIND_DATASET,
                KIND_CANVAS,
                KIND_CANVAS_OBJECT
            ],
            [CAP_PROPERTY_CATALOG],
            EffectLevel::ReadOnly,
            true
        ),
        tool!(
            "properties.set",
            "Set a property",
            "Write one catalog property through the same planner the panel controls use",
            super::properties::PropertyWriteParams,
            [
                KIND_APP,
                KIND_DOCUMENT,
                KIND_DATASET,
                KIND_CANVAS,
                KIND_CANVAS_OBJECT
            ],
            [CAP_PROPERTY_CATALOG],
            EffectLevel::Reversible,
            true
        ),
        Spec {
            id: super::property_batch::TOOL_SET_MANY,
            title: "Set several properties",
            description: "Write exact (target, property, value) triples as one atomic commit and one undo step",
            schema: super::property_batch::schema(),
            kinds: &[KIND_APP, KIND_DOCUMENT, KIND_DATASET, KIND_CANVAS, KIND_CANVAS_OBJECT],
            capabilities: &[CAP_PROPERTY_CATALOG],
            effect: EffectLevel::Reversible,
            deterministic: true,
        },
        tool!(
            "properties.reset",
            "Reset a property",
            "Re-derive one catalog property from its default policy in each target's context",
            super::properties::PropertyKeyParams,
            [
                KIND_APP,
                KIND_DOCUMENT,
                KIND_DATASET,
                KIND_CANVAS,
                KIND_CANVAS_OBJECT
            ],
            [CAP_PROPERTY_CATALOG],
            EffectLevel::Reversible,
            true
        ),
    ]
    .into_iter()
    .map(make_descriptor)
    .chain(super::scientific_plot::descriptors())
    .chain(super::science::descriptors())
    .chain(super::dosy::descriptors())
    .chain(super::region_fit::descriptors())
    .chain(std::iter::once(super::processing_edit::descriptor()))
    .collect()
}

fn make_descriptor(spec: Spec) -> ToolDescriptor {
    ToolDescriptor {
        id: spec.id.to_owned(),
        version: 1,
        title: spec.title.to_owned(),
        description: spec.description.to_owned(),
        parameter_schema: spec.schema,
        result_schema: if spec.id == "processing.apply_scheme" {
            super::scientific_schema::task_result()
        } else {
            serde_json::json!({"type":"object","additionalProperties":false})
        },
        target_kinds: spec
            .kinds
            .iter()
            .copied()
            .map(ResourceKindId::new)
            .collect(),
        required_capabilities: spec
            .capabilities
            .iter()
            .copied()
            .map(CapabilityId::new)
            .collect(),
        effect: spec.effect,
        undoable: spec.effect == EffectLevel::Reversible,
        deterministic: spec.deterministic,
        task_kind: if spec.id == "processing.apply_scheme" {
            "background"
        } else if spec.effect == EffectLevel::ReadOnly {
            "synchronous"
        } else {
            "snapshot_then_commit"
        }
        .to_owned(),
    }
}

fn object_schema(required: &[(&str, &str)], optional: &[(&str, &str)]) -> serde_json::Value {
    let properties = required
        .iter()
        .chain(optional)
        .map(|(name, kind)| ((*name).to_owned(), serde_json::json!({"type":kind})))
        .collect::<serde_json::Map<_, _>>();
    serde_json::json!({"type":"object","properties":properties,"required":required.iter().map(|(name, _)| *name).collect::<Vec<_>>(),"additionalProperties":false})
}

pub(super) fn tool_outputs(id: &str) -> Vec<String> {
    match id {
        "data.import" | "data.transform" => vec!["resources".to_owned()],
        "figure.export" => vec!["files".to_owned()],
        _ => vec!["result".to_owned()],
    }
}
