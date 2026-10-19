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
pub use preview::*;
mod registry;
mod resources;
mod tasks;
mod tool_executors;
mod tools;
mod types;
mod workflow;

pub(crate) use document::content_geometry_detail;
pub use document::{
    DOCUMENT_INSPECT, DOCUMENT_PAGE_DEFAULT, DOCUMENT_PAGE_MAX, DOCUMENT_RESPONSE_BYTES,
    DocumentInspectParams, DocumentView, KIND_PANEL, document_inspect_schema, document_root_target,
    panel_resource,
};
pub use properties::{TOOL_INSPECT, TOOL_RESET, TOOL_SET, property_definition_description};
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
