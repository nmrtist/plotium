use rmcp::model::ToolAnnotations;

#[derive(Clone, Copy)]
enum ToolEffect {
    Read,
    Diagnostic,
    Prepare,
    Execute,
    Cancel,
}

impl ToolEffect {
    fn for_tool(name: &str) -> Self {
        match name {
            "plotium_get_state"
            | "plotium_capabilities_get"
            | "plotium_resources_search"
            | "plotium_resources_inspect"
            | "plotium_properties_list"
            | "plotium_properties_inspect"
            | "plotium_document_inspect"
            | "plotium_preview_plan"
            | "plotium_render_preview"
            | "plotium_table_columns"
            | "plotium_table_read_rows"
            | "plotium_analysis_results_inspect"
            | "plotium_diffusion_inspect"
            | "plotium_processing_inspect"
            | "plotium_tasks_get"
            | "plotium_tasks_list"
            | "plotium_save_status"
            | "plotium_export_status" => Self::Read,
            "plotium_figure_diagnose" => Self::Diagnostic,
            "plotium_plan" => Self::Prepare,
            "plotium_execute" | "plotium_apply" => Self::Execute,
            "plotium_tasks_cancel" => Self::Cancel,
            _ => unreachable!("every advertised tool needs an effect classification"),
        }
    }

    fn annotations(self) -> ToolAnnotations {
        let (read_only, destructive, idempotent, open_world) = match self {
            Self::Read => (true, false, true, false),
            // Continuation consumes a one-use cursor even though the project is unchanged.
            Self::Diagnostic => (true, false, false, false),
            // Preflight retains a new executable plan and may read files, schemas,
            // processing schemes or image assets. It does not commit the candidate.
            Self::Prepare => (false, false, false, true),
            // The same entry point can edit/delete, submit scientific work, or
            // request save/export writes. One-use plans are not a retry guarantee.
            Self::Execute => (false, true, false, true),
            // Cancellation changes task state; repeating it has no additional effect.
            Self::Cancel => (false, true, true, false),
        };
        ToolAnnotations::new()
            .read_only(read_only)
            .destructive(destructive)
            .idempotent(idempotent)
            .open_world(open_world)
    }
}

pub(crate) fn annotations(name: &str) -> ToolAnnotations {
    ToolEffect::for_tool(name).annotations()
}

#[cfg(test)]
#[path = "tool_effects_tests.rs"]
mod tests;
