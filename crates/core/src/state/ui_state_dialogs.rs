//! Working state for the modal dialogs the UI can open: command palette,
//! processing schemes and templates, spectrum arithmetic, and alignment.
//! Split from `ui_state.rs` to keep that file under the source-size limit.

#[derive(Default)]
pub struct CommandPaletteState {
    pub query: String,
    pub selected: usize,
}

pub enum ProcessingSchemeDialogState {
    ResolvePending {
        fallback_dataset: usize,
    },
    Review {
        path: std::path::PathBuf,
        plan: crate::project::SchemeApplicationPlan,
        policy: crate::project::SchemeApplicationPolicy,
    },
}

pub struct TemplateBrowserEntry {
    pub name: String,
    pub path: std::path::PathBuf,
    pub scheme: Result<crate::project::ProcessingScheme, String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SpectrumArithmeticOp {
    AddDataset,
    SubtractDataset,
    MultiplyConstant,
    AddConstant,
}

impl SpectrumArithmeticOp {
    pub const ALL: [Self; 4] = [
        Self::AddDataset,
        Self::SubtractDataset,
        Self::MultiplyConstant,
        Self::AddConstant,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::AddDataset => "A + k·B",
            Self::SubtractDataset => "A − k·B",
            Self::MultiplyConstant => "A × k",
            Self::AddConstant => "A + c",
        }
    }

    pub fn is_binary(self) -> bool {
        matches!(self, Self::AddDataset | Self::SubtractDataset)
    }
}

#[derive(Clone, Copy)]
pub struct SpectrumArithmeticDialogState {
    pub a: usize,
    pub b: usize,
    pub op: SpectrumArithmeticOp,
    pub k: f64,
    pub constant: f64,
}

#[derive(Clone)]
pub struct AlignSpectraDialogState {
    pub lo: f64,
    pub hi: f64,
    pub custom_target: bool,
    pub target_ppm: f64,
    /// Preview cache: peak detection over every candidate is too heavy to rerun
    /// on each repaint, so the plan persists until inputs or the doc change.
    pub plan: Option<crate::state::AlignPlan>,
    pub history_mark: (usize, usize),
}

pub enum ProcessingTemplateDialogState {
    SaveAs {
        dataset: usize,
        name: String,
    },
    Browse {
        dataset: usize,
        entries: Vec<TemplateBrowserEntry>,
        confirm_delete: Option<usize>,
    },
}

/// One row of the user-style browser: a saved style, or why its file could
/// not be read.
pub struct CanvasStyleEntry {
    pub name: String,
    pub theme: Result<crate::theme::Theme, String>,
}

/// Save or manage user styles. The target canvas is resolved from the active
/// canvas when the user acts, so no canvas index is held across frames.
pub enum CanvasStyleDialogState {
    SaveAs {
        name: String,
    },
    Browse {
        entries: Vec<CanvasStyleEntry>,
        /// The style awaiting a second Delete click, by name.
        confirm_delete: Option<String>,
    },
}
