use plotium_core::state::{Dataset, ObjectId, PlotiumApp, Tool};

pub(super) fn editing_gate(app: &PlotiumApp, id: CommandId) -> Result<(), &'static str> {
    super::super::nmr_volume::editing_gate(app, id).and_then(|_| spotlight_edit_gate(app, id))
}

fn spotlight_edit_gate(app: &PlotiumApp, id: CommandId) -> Result<(), &'static str> {
    use CommandId::*;
    let edits_canvas = matches!(
        id,
        StepProperty(_)
            | Group
            | Ungroup
            | CreatePanel
            | ComposePanel
            | DissolvePanel
            | DeletePanel
            | DuplicatePanel
            | MergePanels
            | StackPanelsSharedX
            | SplitPanel
            | SetPanelLayout(_)
            | MoveContentToPanel(_)
            | ToggleGrid
            | CanvasSettings
            | SetCanvasSizePreset(_)
            | ArrangeGrid(_, _)
            | ArrangeGridCustom
            | SimplifyInnerAxes
            | SetSpacingMode(_)
            | SetGutterPreset(_)
            | Align(_)
            | Distribute(_)
            | ZOrder(_)
            | ApplyTheme(_)
            | FigureTypography
            | FitPlotY
            | FitPlotXY
            | ResetPeakHeight
            | PasteImage
            | ReplaceImage
    );
    requires(
        !edits_canvas || !super::super::canvas::active_canvas_is_placeholder(app),
        "Select a matching canvas or turn off spotlight before editing.",
    )
}

pub(super) fn spotlight_gate(app: &PlotiumApp, id: CommandId) -> Result<(), &'static str> {
    requires(
        (id == CommandId::ToggleBoardSpotlight && app.session.ui.board_spotlight.is_some())
            || (!app.doc.canvases.is_empty() && plotium_core::state::board_filter_active(app)),
        "Filter the Data or Canvases list before highlighting matches.",
    )
}

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

pub(super) fn xas_merge_gate(app: &PlotiumApp) -> Result<(), &'static str> {
    let selection = &app.session.ui.data_selection;
    let scans = selection
        .iter()
        .filter(|index| {
            app.doc
                .datasets
                .get(**index)
                .is_some_and(|d| d.as_xas().is_some())
        })
        .count();
    requires(
        scans >= 2 && scans == selection.len(),
        "Select two or more XAS scans in the Data list before merging.",
    )
}

pub(super) fn xas_summary_gate(dataset: Option<&Dataset>) -> Result<(), &'static str> {
    requires(
        dataset
            .and_then(Dataset::as_xas)
            .is_some_and(|xas| xas.k_end().is_some()),
        "Select an XAS scan with an EXAFS region before building the summary figure.",
    )
}

pub(super) fn palette_available(app: &PlotiumApp, id: super::CommandId) -> bool {
    use super::CommandId;
    id != CommandId::CommandPalette
        || (app.session.ui.processing_scheme_dialog.is_none()
            && app.session.ui.processing_template_dialog.is_none()
            && app.session.ui.canvas_style_dialog.is_none()
            && app.session.ui.spectrum_arithmetic_dialog.is_none()
            && app.session.ui.align_spectra_dialog.is_none()
            && app.session.ui.trace_alignment_dialog.is_none()
            && app.session.ui.trace_composer.is_none()
            && !app.session.ui.interaction.is_active())
}

pub(super) fn data_navigation_gate(app: &PlotiumApp, id: CommandId) -> Result<(), &'static str> {
    match id {
        CommandId::EditDatasetMetadata(Some(dataset)) => requires(
            app.doc.dataset_index(dataset).is_some(),
            "Choose an available dataset.",
        ),
        CommandId::EditDatasetMetadata(None) | CommandId::LocateDatasetPlot(None) => requires(
            app.active_dataset().is_some(),
            "Select data in the Data List first.",
        ),
        CommandId::LocateDatasetPlot(Some(_)) => Ok(()),
        CommandId::GoToDatasetPlot(target) => requires(
            target.hidden(app).is_some(),
            "Choose an available plot series.",
        ),
        CommandId::RevealDatasetPlot(target) => requires(
            target.hidden(app) == Some(true),
            "Choose a hidden plot series to reveal.",
        ),
        CommandId::ViewImportBatch(batch) => {
            crate::ui::import_feedback::view_batch_gate(app, batch)
        }
        _ => unreachable!("not a data navigation command"),
    }
}
