//! Modal and floating application windows grouped by responsibility.

mod canvas_settings;
mod external_export;
mod external_save;
pub(super) use external_export::external_export_window;
mod panel_note_edit;
mod plot_text_edit;
mod project;
pub(super) use external_save::external_save_window;
mod text_edit;

use super::*;

pub(super) use canvas_settings::{canvas_settings_layer, canvas_settings_window};
pub(super) use panel_note_edit::panel_note_edit_window;
pub(super) use plot_text_edit::plot_text_edit_window;
pub(crate) use project::handle_close_request;
pub(super) use project::{quit_confirm_window, save_project_window};
pub(super) use text_edit::text_edit_window;
