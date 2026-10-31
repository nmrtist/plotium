use plotium_core::state::{Dataset, ObjectId, PlotiumApp, Tool};

use super::{CommandExecutionClass, CommandId};

impl CommandId {
    pub(super) fn tool_target(self) -> Option<Tool> {
        match self {
            Self::SelectRange => Some(Tool::SelectRegion),
            Self::Regions => Some(Tool::Regions),
            Self::PeakList => Some(Tool::Peaks),
            Self::LineFit => Some(Tool::LineFit),
            Self::Integrate => Some(Tool::Integrate),
            Self::Tool(tool) => Some(tool),
            _ => None,
        }
    }

    pub fn execution_class(self) -> CommandExecutionClass {
        match self {
            Self::RunBatchWorkflow | Self::RunScientificScript => CommandExecutionClass::ToolEditor,
            Self::OperationHistory | Self::CommandPalette | Self::About => {
                CommandExecutionClass::UiOnly
            }
            Self::ExportData
            | Self::Export(_)
            | Self::ApplyProcessingTemplate
            | Self::RunCraft
            | Self::CraftComponentTable
            | Self::ApplyTheme(_) => CommandExecutionClass::ToolBacked,
            Self::RecomputeScientificTask(_) => CommandExecutionClass::ToolBacked,
            _ => CommandExecutionClass::UiOnly,
        }
    }
}

pub(super) fn has_active_plot(app: &PlotiumApp) -> bool {
    app.session
        .active_canvas
        .and_then(|ci| app.doc.canvases.get(ci))
        .is_some_and(|canvas| canvas.active_plot_object_id().is_some())
}

pub(super) fn requires(ok: bool, reason: &'static str) -> Result<(), &'static str> {
    if ok { Ok(()) } else { Err(reason) }
}

pub(super) fn selected_paths_unlocked(app: &PlotiumApp) -> bool {
    app.session.active_canvas.is_some_and(|ci| {
        app.session
            .ui
            .hierarchical_selection
            .paths()
            .iter()
            .all(|path| {
                path.panel
                    .and_then(|id| app.doc.canvases[ci].panel(id))
                    .is_none_or(|panel| !panel.locked)
                    && path
                        .content
                        .and_then(|id| app.doc.canvases[ci].object(id))
                        .is_none_or(|item| !item.locked)
            })
    })
}

pub(crate) fn chart_plot_target(app: &PlotiumApp, dataset: usize) -> Option<(usize, ObjectId)> {
    let candidates = app
        .session
        .active_canvas
        .into_iter()
        .chain(0..app.doc.canvases.len());
    for ci in candidates {
        let Some(canvas) = app.doc.canvases.get(ci) else {
            continue;
        };
        let hit = canvas.objects.iter().find(|object| {
            object.plot().is_some_and(|plot| {
                plot.binding.primary_dataset() == Some(app.doc.datasets[dataset].resource_id())
            })
        });
        if let Some(object) = hit {
            return Some((ci, object.id));
        }
    }
    None
}

pub(super) fn tool_commands() -> [Tool; 17] {
    [
        Tool::Select,
        Tool::BrowseZoom,
        Tool::ManualPhase,
        Tool::Integrate,
        Tool::Peaks,
        Tool::InspectCursor,
        Tool::DeltaCursor,
        Tool::Symmetry,
        Tool::Slice,
        Tool::LineFit,
        Tool::Annotate,
        Tool::Text,
        Tool::PanelLabel,
        Tool::Rect,
        Tool::Ellipse,
        Tool::Line,
        Tool::Arrow,
    ]
}

pub(super) fn peak_height_gate(app: &PlotiumApp) -> Result<(), &'static str> {
    requires(
        crate::ui::canvas::can_reset_peak_height(app),
        "Select an unlocked numeric trace plot before resetting peak height.",
    )
}

pub(super) fn template_gate(
    app: &PlotiumApp,
    dataset: Option<usize>,
    id: CommandId,
) -> Result<(), &'static str> {
    let usable = |di| crate::ui::processing_templates::can_use_templates(app, di);
    let reason = if id == CommandId::SaveProcessingTemplate {
        "Select a dataset with a processing pipeline before saving a template."
    } else {
        "Select a dataset with a processing pipeline before applying a template."
    };
    requires(dataset.is_some_and(usable), reason)
}

pub(super) fn xas_summary_gate(dataset: Option<&Dataset>) -> Result<(), &'static str> {
    requires(
        dataset
            .and_then(Dataset::as_xas)
            .is_some_and(|xas| xas.k_end().is_some()),
        "Select an XAS scan with an EXAFS region before building the summary figure.",
    )
}
