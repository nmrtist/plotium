//! What the author wants a plot to show, stated by the user directly or
//! through the agent. The intent is advisory: it never changes the drawing.
use serde::{Deserialize, Serialize};

/// Longest claim, in characters: one sentence, not a caption.
pub const CLAIM_MAX_CHARS: usize = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlotPurpose {
    /// Compare values between groups.
    Compare,
    /// A trend along a continuous variable.
    Trend,
    /// Distribution or spread of samples.
    Distribution,
    /// Relationship between two quantities.
    Relationship,
    /// Parts of a whole.
    Composition,
    /// Change, ratio or fold difference between paired values.
    Change,
    /// A local detail, such as a zoom or an inset.
    Detail,
}

impl PlotPurpose {
    pub const ALL: [Self; 7] = [
        Self::Compare,
        Self::Trend,
        Self::Distribution,
        Self::Relationship,
        Self::Composition,
        Self::Change,
        Self::Detail,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Compare => "compare",
            Self::Trend => "trend",
            Self::Distribution => "distribution",
            Self::Relationship => "relationship",
            Self::Composition => "composition",
            Self::Change => "change",
            Self::Detail => "detail",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.id() == id)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlotIntent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<PlotPurpose>,
    /// The one sentence the plot should support; empty when not stated.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub claim: String,
}

impl PlotIntent {
    pub fn is_empty(&self) -> bool {
        self.purpose.is_none() && self.claim.is_empty()
    }

    pub fn normalized_claim(claim: &str) -> Result<String, String> {
        let claim = claim.trim();
        if claim.chars().count() > CLAIM_MAX_CHARS {
            return Err(format!(
                "A claim is one sentence of at most {CLAIM_MAX_CHARS} characters."
            ));
        }
        Ok(claim.to_owned())
    }

    pub fn normalized(&self) -> Result<Self, String> {
        Ok(Self {
            purpose: self.purpose,
            claim: Self::normalized_claim(&self.claim)?,
        })
    }
}
