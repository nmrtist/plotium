//! Scientific context is independent of manual assignment confirmation.
use crate::protein::{self, nucleus_element};
use crate::state::{DatasetId, Document, PlotiumApp};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SampleConditionId(pub Uuid);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampleCondition {
    pub id: SampleConditionId,
    pub name: String,
    pub temperature_kelvin: Option<f64>,
    pub ph: Option<f64>,
    pub solvent: String,
    pub isotope_labeling: String,
    pub construct: String,
    /// Record unsupported chemistry without preventing browsing or annotation.
    pub nonstandard_chemistry: String,
    pub single_major_conformer: bool,
}

impl Default for SampleCondition {
    fn default() -> Self {
        Self {
            id: SampleConditionId(Uuid::new_v4()),
            name: "Sample conditions".into(),
            temperature_kelvin: None,
            ph: None,
            solvent: String::new(),
            isotope_labeling: String::new(),
            construct: String::new(),
            nonstandard_chemistry: String::new(),
            single_major_conformer: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentKind {
    #[default]
    Unknown,
    Noesy,
    Hnca,
    Hnco,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentAxisRole {
    #[default]
    Unknown,
    ObservedProton,
    IndirectProton,
    Nitrogen,
    Carbon,
}
impl ExperimentAxisRole {
    fn element(self) -> Option<&'static str> {
        match self {
            Self::Unknown => None,
            Self::ObservedProton | Self::IndirectProton => Some("H"),
            Self::Nitrogen => Some("N"),
            Self::Carbon => Some("C"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Experiment {
    pub sample: SampleConditionId,
    pub kind: ExperimentKind,
    /// Acquisition order F1/F2/F3, never display-axis order.
    pub roles: [ExperimentAxisRole; 3],
    pub source: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScientificContext {
    pub samples: Vec<SampleCondition>,
    pub experiments: BTreeMap<DatasetId, Experiment>,
}
impl ScientificContext {
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty() && self.experiments.is_empty()
    }
    pub fn validate(&self, doc: &Document) -> Result<(), String> {
        let mut ids = HashSet::new();
        for s in &self.samples {
            if !ids.insert(s.id) || s.name.trim().is_empty() {
                return Err("Sample conditions need a unique ID and nonempty name".into());
            }
            if s.temperature_kelvin
                .is_some_and(|v| !v.is_finite() || v <= 0.0)
                || s.ph
                    .is_some_and(|v| !v.is_finite() || !(0.0..=14.0).contains(&v))
            {
                return Err("Enter positive temperature in kelvin and pH between 0 and 14".into());
            }
        }
        for (&dataset, e) in &self.experiments {
            let v = protein::volume(doc, dataset)?;
            if !ids.contains(&e.sample) {
                return Err("Experiment references missing sample conditions".into());
            }
            for (axis, role) in e.roles.iter().enumerate() {
                if let Some(element) = role.element()
                    && nucleus_element(v.processed.axes()[axis].nucleus().unwrap_or_default())
                        .is_none_or(|n| !n.eq_ignore_ascii_case(element))
                {
                    return Err(format!(
                        "F{} role requires a known {element} nucleus",
                        axis + 1
                    ));
                }
            }
        }
        Ok(())
    }

    /// Scope gate only: this does not certify assignments or quantitative evidence.
    pub fn check_structure_scope(&self, doc: &Document, dataset: DatasetId) -> Result<(), String> {
        self.validate(doc)?;
        let e = self
            .experiments
            .get(&dataset)
            .ok_or("Record experiment metadata first")?;
        let s = self
            .samples
            .iter()
            .find(|s| s.id == e.sample)
            .ok_or("Missing sample conditions")?;
        let m = doc.protein.molecule(
            *doc.protein
                .bindings
                .get(&dataset)
                .ok_or("Bind a molecule first")?,
        )?;
        if m.chains.len() != 1 {
            return Err("Structure workflow supports a single standard-residue chain only".into());
        }
        if !s.nonstandard_chemistry.trim().is_empty() {
            return Err("Modified residues, ligands and special bonding are outside the first structure workflow".into());
        }
        if !s.single_major_conformer {
            return Err(
                "Confirm a single major conformer before entering the structure workflow".into(),
            );
        }
        if e.kind == ExperimentKind::Unknown || e.roles.contains(&ExperimentAxisRole::Unknown) {
            return Err("Specify the experiment and every acquisition-axis role first".into());
        }
        Ok(())
    }

    pub fn check_noe_roles(&self, doc: &Document, dataset: DatasetId) -> Result<(), String> {
        self.check_structure_scope(doc, dataset)?;
        let e = self.experiments.get(&dataset).ok_or("Missing experiment")?;
        if e.kind != ExperimentKind::Noesy
            || e.roles
                .iter()
                .filter(|&&r| r == ExperimentAxisRole::ObservedProton)
                .count()
                != 1
            || e.roles
                .iter()
                .filter(|&&r| r == ExperimentAxisRole::IndirectProton)
                .count()
                != 1
        {
            return Err(
                "NOE generation requires NOESY with distinct observed and indirect proton axes"
                    .into(),
            );
        }
        Ok(())
    }
}
impl PlotiumApp {
    pub fn save_scientific_context_draft(
        &mut self,
        baseline: &ScientificContext,
        context: ScientificContext,
    ) -> Result<(), String> {
        if baseline != &self.doc.protein.context {
            return Err(
                "Scientific context changed. Reload current context or discard this draft.".into(),
            );
        }
        self.edit_scientific_context(context)
    }

    pub fn edit_scientific_context(&mut self, context: ScientificContext) -> Result<(), String> {
        let mut state = self.doc.protein.clone();
        state.context = context;
        self.commit_protein(state)
    }
}
