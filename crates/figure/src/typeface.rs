//! Typeface and weight of a figure's text.

/// Typeface of a figure's text. Each one ships with macOS and Windows, or, for
/// Helvetica, with macOS while Windows draws its metric twin Arial; Linux
/// distributions carry a metric-compatible Liberation substitute. A figure
/// therefore lays out alike on screen and in every export format. Layout
/// estimates text width from proportional sans metrics, which a narrower
/// serif never exceeds; a monospaced face would need its own estimate.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Typeface {
    /// The Nature-portfolio default.
    #[default]
    Arial,
    Helvetica,
    TimesNewRoman,
}

impl Typeface {
    pub const ALL: [Typeface; 3] = [Self::Arial, Self::Helvetica, Self::TimesNewRoman];

    /// Font family name as installed on macOS (and Windows, except Helvetica).
    pub const fn family_name(self) -> &'static str {
        match self {
            Self::Arial => "Arial",
            Self::Helvetica => "Helvetica",
            Self::TimesNewRoman => "Times New Roman",
        }
    }

    /// Metric-compatible twin installed where this family is not, if any.
    pub const fn metric_twin(self) -> Option<&'static str> {
        match self {
            Self::Arial => Some("Helvetica"),
            Self::Helvetica => Some("Arial"),
            Self::TimesNewRoman => None,
        }
    }

    /// Metric-compatible family that Linux installs in place of this one.
    pub const fn metric_substitute(self) -> &'static str {
        match self {
            Self::Arial | Self::Helvetica => "Liberation Sans",
            Self::TimesNewRoman => "Liberation Serif",
        }
    }
}

/// Weight of a figure's text. Elements that are always bold, such as panel
/// letters, stay bold under either weight. Layout estimates text width from
/// regular metrics, so long bold labels may sit slightly tighter.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum FontWeight {
    #[default]
    Regular,
    Bold,
}

impl FontWeight {
    pub const fn is_bold(self) -> bool {
        matches!(self, Self::Bold)
    }
}
