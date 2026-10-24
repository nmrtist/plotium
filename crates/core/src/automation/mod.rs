//! Model-neutral automation primitives shared by the desktop, CLI and future
//! agents. The public boundary is deliberately semantic: callers can inspect,
//! select and invoke registered tools, but cannot construct document actions.

mod native_import;
pub use native_import::*;
mod table_import;
mod table_provenance;
mod table_read;
mod table_transform;
mod table_update;
pub use table_import::*;
pub use table_read::*;
pub use table_transform::*;
pub use table_update::*;
mod curve_bindings;
mod dataset_delete;
pub use dataset_delete::DATASET_DELETE;
mod figure_edit;
mod plot_creation;
pub use figure_edit::*;
pub use plot_creation::*;
mod diagnostics;
pub(crate) use diagnostics::candidate_figure_cost;
pub use diagnostics::*;
mod document;
pub use curve_bindings::*;
mod candidate_preview;
mod preview;
pub use candidate_preview::*;
mod properties;
mod property_batch;
pub use preview::*;
mod dosy;
mod region_fit;
pub use region_fit::{DIFFUSION_FIT, DiffusionFitParams, REGIONS_EXTRACT, RegionExtractParams};
mod frozen_scheme;
mod prepared_processing;
mod processing_edit;
mod registry;
mod resources;
mod science;
mod scientific_plot;
mod scientific_schema;
pub use dosy::{ANALYSIS_RESULTS_INSPECT, AnalysisResultsParams, DOSY_BUILD, DosyBuildParams};
pub use scientific_plot::*;
mod task_receipts;
mod tasks;
pub use frozen_scheme::*;
pub use processing_edit::{
    PROCESSING_EDIT, PipelineEdit, ProcessingEditParams, processing_edit_schema,
};
pub use science::{
    DIFFUSION_INSPECT, DIFFUSION_SET_INPUT, DiffusionInspectParams, DiffusionSetInputParams,
    PROCESSING_INSPECT, diffusion_input_schema, inspect_diffusion_input,
};
pub use task_receipts::*;
mod tool_executors;
pub use tool_executors::canvas_export_stem;
mod tools;
mod types;
mod workflow;
mod workflow_processing;

pub(crate) use document::content_geometry_detail;
pub use document::{
    DOCUMENT_INSPECT, DOCUMENT_PAGE_DEFAULT, DOCUMENT_PAGE_MAX, DOCUMENT_RESPONSE_BYTES,
    DocumentInspectParams, DocumentView, KIND_PANEL, canvas_selection_reading,
    document_inspect_schema, document_root_target, panel_resource,
};
pub(crate) use properties::parse_color;
pub use properties::{
    TOOL_INSPECT, TOOL_RESET, TOOL_SET, hand_edit_readings, property_definition_description,
};
pub use property_batch::{MAX_WRITES as PROPERTY_BATCH_MAX_WRITES, TOOL_SET_MANY};
pub use registry::ToolRegistry;
pub use resources::*;
pub use tasks::*;
pub use tools::*;
pub use types::*;
pub use workflow::*;

pub const WORKFLOW_SCHEMA: &str = "plotium.workflow.v1";
pub const RUN_MANIFEST_SCHEMA: &str = "plotium.run-manifest.v1";

#[cfg(test)]
mod tests;

#[cfg(test)]
mod table_tests;
