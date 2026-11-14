/// A rail row in Preferences, including controls that only live for this session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SettingsCategory {
    #[default]
    General,
    Appearance,
    Processing,
    Export,
    Recent,
    AgentConnections,
}

impl SettingsCategory {
    pub const ALL: [SettingsCategory; 6] = [
        SettingsCategory::General,
        SettingsCategory::Appearance,
        SettingsCategory::Processing,
        SettingsCategory::Export,
        SettingsCategory::Recent,
        SettingsCategory::AgentConnections,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            SettingsCategory::General => "General",
            SettingsCategory::Appearance => "Appearance",
            SettingsCategory::Processing => "Processing",
            SettingsCategory::Export => "Export",
            SettingsCategory::Recent => "Recent",
            SettingsCategory::AgentConnections => "AI connections (MCP)",
        }
    }

    pub const fn section_id(self) -> &'static str {
        match self {
            SettingsCategory::General => "preferences.general",
            SettingsCategory::Appearance => "preferences.appearance",
            SettingsCategory::Processing => "preferences.processing",
            SettingsCategory::Export => "preferences.export",
            SettingsCategory::Recent => "preferences.recent",
            SettingsCategory::AgentConnections => "preferences.agent_connections",
        }
    }
}

/// The Preferences window's working state: the draft [`crate::settings::Settings`] every widget
/// edits, the selected rail category, a scheduled debounced-flush time, and the
/// last non-fatal save error. The draft is applied to the live app on every edit
/// and persisted on the debounce or on close; the on-disk file is only ever
/// replaced wholesale by a valid draft.
pub struct SettingsDialog {
    pub category: SettingsCategory,
    pub draft: crate::settings::Settings,
    pub flush_at: Option<f64>,
    pub last_error: Option<String>,
}

impl SettingsDialog {
    pub fn new(settings: crate::settings::Settings) -> Self {
        Self {
            category: SettingsCategory::default(),
            draft: settings,
            flush_at: None,
            last_error: None,
        }
    }
}
