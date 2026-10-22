//! Presentation for the property catalog: what a property is called in the
//! interface and where it lives.
//!
//! The semantic half — identity, schema, applicability, tier, default policy —
//! stays in `plotium_core::properties`. This table adds only what the interface
//! needs, and may never introduce an entry that has no definition behind it. In
//! particular the tier is *read* from the definition rather than repeated here,
//! so a property cannot be Essential in the panel and Advanced in the catalog.

pub(crate) mod discovery;
#[path = "groups.rs"]
mod groups;
pub(crate) mod panel;
mod presentations;
pub(crate) mod readout;
mod routes;
mod search;
mod types;

#[cfg(test)]
pub(crate) mod fixture;

pub(crate) use groups::GROUPS;
pub use presentations::PRESENTATIONS;
#[cfg(test)]
use routes::*;
pub(crate) use search::property_hits;
pub use types::*;

use plotium_core::properties::PropertyId;
#[cfg(test)]
use plotium_core::properties::{Tier, definition};
#[cfg(test)]
use plotium_core::properties::{
    axis, baseline, bin, canvas, contour, guide, object, phase, smooth, typography,
};
#[cfg(test)]
use plotium_core::state::SettingsCategory;
use plotium_core::state::WorkflowTab;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
