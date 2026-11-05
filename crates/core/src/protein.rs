//! Protein sequence and manual 3D assignment state.
use crate::state::{DatasetId, Document, Peak3DId, PlotiumApp};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use uuid::Uuid;

macro_rules! id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Uuid);
    };
}

id!(MoleculeId);
id!(ChainId);
id!(ResidueId);
id!(CandidateId);

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProteinState {
    #[serde(default)]
    pub shift_reviews: Vec<crate::protein_shifts::ShiftReview>,
    #[serde(default)]
    pub context: crate::protein_context::ScientificContext,
    pub molecules: Vec<Molecule>,
    pub bindings: BTreeMap<DatasetId, MoleculeId>,
    pub assignments: Vec<Assignment>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Molecule {
    pub id: MoleculeId,
    pub name: String,
    pub source: String,
    pub fasta_sha256: String,
    pub chains: Vec<Chain>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chain {
    pub id: ChainId,
    pub title: String,
    pub name: String,
    pub start: i32,
    pub residues: Vec<Residue>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Residue {
    pub id: ResidueId,
    pub code: char,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomTarget {
    pub residue: ResidueId,
    pub members: BTreeSet<String>,
    #[serde(default)]
    pub group_meaning: GroupMeaning,
}

/// A shared resonance is an observation, not a claim of chemical equivalence.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupMeaning {
    #[default]
    Unspecified,
    OneOf,
    Shared,
}

pub type Targets = [Option<AtomTarget>; 3];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub id: CandidateId,
    pub targets: Targets,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Assignment {
    pub dataset: DatasetId,
    pub peak: Peak3DId,
    pub candidates: Vec<Candidate>,
    pub confirmed: Option<CandidateId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChainLabel {
    pub name: String,
    pub start: i32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Edit {
    Import {
        source: String,
        fasta: String,
        name: Option<String>,
        chains: Option<Vec<ChainLabel>>,
    },
    Rename {
        molecule: MoleculeId,
        name: String,
    },
    LabelChain {
        molecule: MoleculeId,
        chain: ChainId,
        label: ChainLabel,
    },
    Delete {
        molecule: MoleculeId,
    },
    ClearSpectrumAssignments {
        dataset: DatasetId,
    },
    Bind {
        dataset: DatasetId,
        molecule: Option<MoleculeId>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum AssignmentEdit {
    Add {
        targets: Targets,
        #[serde(default)]
        note: String,
    },
    Update {
        candidate: CandidateId,
        targets: Option<Targets>,
        note: Option<String>,
    },
    Delete {
        candidate: CandidateId,
    },
    Confirm {
        candidate: CandidateId,
    },
    Withdraw,
    Clear,
}

#[path = "protein_catalog.rs"]
mod catalog;
pub use catalog::{atom_matches_nucleus, atoms, nucleus_element, residue_atoms};

fn chain_name(mut index: usize) -> String {
    let mut reversed = Vec::new();
    loop {
        reversed.push(char::from(b'A' + (index % 26) as u8));
        if index < 26 {
            break;
        }
        index = index / 26 - 1;
    }
    reversed.into_iter().rev().collect()
}

pub fn import_preview(source: &str, fasta: &str) -> Result<Molecule, String> {
    let records = plotium_io::fasta::parse(fasta)?;
    let name = std::path::Path::new(source)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Protein");
    Ok(Molecule {
        id: MoleculeId(Uuid::new_v4()),
        name: name.into(),
        source: source.into(),
        fasta_sha256: format!("{:x}", Sha256::digest(fasta.as_bytes())),
        chains: records
            .into_iter()
            .enumerate()
            .map(|(i, r)| Chain {
                id: ChainId(Uuid::new_v4()),
                title: r.title,
                name: chain_name(i),
                start: 1,
                residues: r
                    .sequence
                    .chars()
                    .map(|code| Residue {
                        id: ResidueId(Uuid::new_v4()),
                        code,
                    })
                    .collect(),
            })
            .collect(),
    })
}

impl ProteinState {
    pub fn is_empty(&self) -> bool {
        self.shift_reviews.is_empty()
            && self.context.is_empty()
            && self.molecules.is_empty()
            && self.bindings.is_empty()
            && self.assignments.is_empty()
    }
    pub fn molecule(&self, id: MoleculeId) -> Result<&Molecule, String> {
        self.molecules
            .iter()
            .find(|m| m.id == id)
            .ok_or_else(|| format!("Unknown molecule {}", id.0))
    }
    pub fn assignment(&self, dataset: DatasetId, peak: Peak3DId) -> Option<&Assignment> {
        self.assignments
            .iter()
            .find(|a| a.dataset == dataset && a.peak == peak)
    }
    pub fn remove_dataset(&mut self, dataset: DatasetId) {
        self.context.experiments.remove(&dataset);
        self.bindings.remove(&dataset);
        self.assignments.retain(|a| a.dataset != dataset);
    }

    pub fn validate(&self, doc: &Document) -> Result<(), String> {
        self.context.validate(doc)?;
        crate::protein_shifts::validate_reviews(&self.shift_reviews)?;
        let (mut molecules, mut chains, mut residues, mut peaks, mut candidates) = (
            HashSet::new(),
            HashSet::new(),
            HashSet::new(),
            HashSet::new(),
            HashSet::new(),
        );
        for m in &self.molecules {
            if !molecules.insert(m.id) || m.chains.is_empty() || m.name.trim().is_empty() {
                return Err(format!("Invalid or duplicate molecule {}", m.id.0));
            }
            let mut labels = HashSet::new();
            for c in &m.chains {
                if !chains.insert(c.id)
                    || c.name.trim().is_empty()
                    || !labels.insert(&c.name)
                    || c.residues.is_empty()
                    || i32::try_from(c.residues.len() - 1)
                        .ok()
                        .and_then(|n| c.start.checked_add(n))
                        .is_none()
                {
                    return Err(format!(
                        "Invalid or duplicate chain {} ({})",
                        c.name, c.id.0
                    ));
                }
                for r in &c.residues {
                    if !residues.insert(r.id)
                        || !plotium_io::fasta::STANDARD_RESIDUES.contains(r.code)
                    {
                        return Err(format!("Invalid or duplicate residue {}", r.id.0));
                    }
                }
            }
        }
        for (&dataset, &molecule) in &self.bindings {
            self.molecule(molecule)?;
            volume(doc, dataset)?;
        }
        for a in &self.assignments {
            let check = || -> Result<(), String> {
                if !peaks.insert((a.dataset, a.peak)) || a.candidates.is_empty() {
                    return Err("Duplicate peak assignment or empty candidate list".into());
                }
                let molecule = self.molecule(
                    *self
                        .bindings
                        .get(&a.dataset)
                        .ok_or("Spectrum has no molecule binding")?,
                )?;
                let v = volume(doc, a.dataset)?;
                if !v.peaks.marks.iter().any(|p| p.id == a.peak) {
                    return Err("3D peak no longer exists".into());
                }
                if a.confirmed
                    .is_some_and(|id| !a.candidates.iter().any(|c| c.id == id))
                {
                    return Err("Confirmed candidate no longer exists".into());
                }
                for c in &a.candidates {
                    if !candidates.insert(c.id) || c.targets.iter().all(Option::is_none) {
                        return Err(format!("Candidate {} is empty or duplicated", c.id.0));
                    }
                    for (i, target) in c.targets.iter().enumerate() {
                        let Some(t) = target else { continue };
                        let result = validate_target(
                            molecule,
                            t,
                            v.processed.axes()[i].nucleus().unwrap_or_default(),
                            a.confirmed == Some(c.id),
                        );
                        result.map_err(|e| format!("Candidate {} F{}: {e}", c.id.0, i + 1))?;
                    }
                }
                Ok(())
            };
            let mut check = check;
            check().map_err(|e| format!("Spectrum {}, peak {}: {e}", a.dataset, a.peak.0))?;
        }
        Ok(())
    }
}

pub fn volume(doc: &Document, dataset: DatasetId) -> Result<&crate::state::NmrVolumeState, String> {
    doc.dataset_by_id(dataset)
        .and_then(|d| d.as_nmr2d())
        .and_then(|n| n.volume.as_ref())
        .ok_or_else(|| format!("Dataset {dataset} is missing or is not a 3D spectrum"))
}

fn validate_target(
    molecule: &Molecule,
    target: &AtomTarget,
    nucleus: &str,
    confirmed: bool,
) -> Result<(), String> {
    let residue = molecule
        .chains
        .iter()
        .flat_map(|c| &c.residues)
        .find(|r| r.id == target.residue)
        .ok_or_else(|| {
            format!(
                "Residue {} does not belong to the bound molecule",
                target.residue.0
            )
        })?;
    let allowed: Vec<_> = residue_atoms(molecule, target.residue)?.collect();
    if target.members.len() == 1 && target.group_meaning != GroupMeaning::Unspecified {
        return Err("Group meaning applies only to multiple atoms".into());
    }
    if confirmed && target.members.len() > 1 && target.group_meaning == GroupMeaning::Unspecified {
        return Err("Choose whether the group means one of the atoms or a shared resonance before confirming".into());
    }
    let mut element = None;
    if target.members.is_empty() {
        return Err("Select at least one atom".into());
    }
    for member in &target.members {
        let (_, current) = atoms(residue.code)
            .find(|(name, _)| *name == member)
            .ok_or_else(|| format!("Unknown atom {member} for {}", residue.code))?;
        if !allowed.iter().any(|(name, _)| *name == member) {
            return Err(format!(
                "Atom {member} is not applicable at this position in a standard peptide chain"
            ));
        }
        if element.replace(current).is_some_and(|e| e != current) {
            return Err("Atom group members must have the same element".into());
        }
    }
    match nucleus_element(nucleus) {
        None if confirmed => {
            return Err("Cannot confirm an assigned axis without a known nucleus".into());
        }
        Some(nucleus) if !element.is_some_and(|e| e.eq_ignore_ascii_case(nucleus)) => {
            return Err(format!("Atom element does not match nucleus {nucleus}"));
        }
        _ => {}
    }
    Ok(())
}

impl PlotiumApp {
    pub fn edit_protein(&mut self, edit: Edit) -> Result<(), String> {
        let mut state = self.doc.protein.clone();
        match edit {
            Edit::Import {
                source,
                fasta,
                name,
                chains,
            } => {
                let mut m = import_preview(&source, &fasta)?;
                if let Some(name) = name {
                    m.name = name;
                }
                if let Some(labels) = chains {
                    if labels.len() != m.chains.len() {
                        return Err("Supply one label per FASTA record".into());
                    }
                    for (c, label) in m.chains.iter_mut().zip(labels) {
                        c.name = label.name;
                        c.start = label.start;
                    }
                }
                state.molecules.push(m);
            }
            Edit::Rename { molecule, name } => {
                state
                    .molecules
                    .iter_mut()
                    .find(|m| m.id == molecule)
                    .ok_or("Unknown molecule")?
                    .name = name;
            }
            Edit::LabelChain {
                molecule,
                chain,
                label,
            } => {
                let c = state
                    .molecules
                    .iter_mut()
                    .find(|m| m.id == molecule)
                    .and_then(|m| m.chains.iter_mut().find(|c| c.id == chain))
                    .ok_or("Unknown chain in molecule")?;
                c.name = label.name;
                c.start = label.start;
            }
            Edit::Delete { molecule } => {
                state.molecule(molecule)?;
                let bound: Vec<_> = state
                    .bindings
                    .iter()
                    .filter(|(_, m)| **m == molecule)
                    .map(|(d, _)| d.to_string())
                    .collect();
                if !bound.is_empty() {
                    return Err(format!("Unbind these spectra first: {}", bound.join(", ")));
                }
                state.molecules.retain(|m| m.id != molecule);
            }
            Edit::ClearSpectrumAssignments { dataset } => {
                volume(&self.doc, dataset)?;
                state.assignments.retain(|a| a.dataset != dataset);
            }
            Edit::Bind { dataset, molecule } => {
                volume(&self.doc, dataset)?;
                if state.bindings.get(&dataset).copied() != molecule
                    && state.assignments.iter().any(|a| a.dataset == dataset)
                {
                    return Err(
                        "Clear this spectrum's assignments before changing its binding".into(),
                    );
                }
                if let Some(molecule) = molecule {
                    state.bindings.insert(dataset, molecule);
                } else {
                    state.bindings.remove(&dataset);
                }
            }
        }
        self.commit_protein(state)
    }

    pub fn edit_assignment(
        &mut self,
        dataset: DatasetId,
        peak: Peak3DId,
        edit: AssignmentEdit,
    ) -> Result<(), String> {
        let mut state = self.doc.protein.clone();
        let v = volume(&self.doc, dataset)?;
        if !v.peaks.marks.iter().any(|p| p.id == peak) {
            return Err("Unknown 3D peak".into());
        }
        let index = state
            .assignments
            .iter()
            .position(|a| a.dataset == dataset && a.peak == peak);
        if matches!(edit, AssignmentEdit::Clear) {
            if let Some(i) = index {
                state.assignments.remove(i);
            }
            return self.commit_protein(state);
        }
        let a = if let Some(i) = index {
            &mut state.assignments[i]
        } else if matches!(edit, AssignmentEdit::Add { .. }) {
            state.assignments.push(Assignment {
                dataset,
                peak,
                candidates: Vec::new(),
                confirmed: None,
            });
            state.assignments.last_mut().ok_or("Missing assignment")?
        } else {
            return Err("Peak has no assignments".into());
        };
        match edit {
            AssignmentEdit::Add { targets, note } => a.candidates.push(Candidate {
                id: CandidateId(Uuid::new_v4()),
                targets,
                note,
            }),
            AssignmentEdit::Update {
                candidate,
                targets,
                note,
            } => {
                let c = a
                    .candidates
                    .iter_mut()
                    .find(|c| c.id == candidate)
                    .ok_or("Unknown candidate")?;
                if let Some(targets) = targets {
                    if c.targets != targets && a.confirmed == Some(candidate) {
                        a.confirmed = None;
                    }
                    c.targets = targets;
                }
                if let Some(note) = note {
                    c.note = note;
                }
            }
            AssignmentEdit::Delete { candidate } => {
                let i = a
                    .candidates
                    .iter()
                    .position(|c| c.id == candidate)
                    .ok_or("Unknown candidate")?;
                a.candidates.remove(i);
                if a.confirmed == Some(candidate) {
                    a.confirmed = None;
                }
            }
            AssignmentEdit::Confirm { candidate } => a.confirmed = Some(candidate),
            AssignmentEdit::Withdraw => a.confirmed = None,
            AssignmentEdit::Clear => return Err("Expected a candidate operation".into()),
        }
        state.assignments.retain(|a| !a.candidates.is_empty());
        self.commit_protein(state)
    }

    pub(crate) fn commit_protein(&mut self, after: ProteinState) -> Result<(), String> {
        after.validate(&self.doc)?;
        if after != self.doc.protein {
            self.try_execute_action(self.protein_action(after))
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub(crate) fn protein_action(&self, after: ProteinState) -> crate::actions::Action {
        crate::actions::Action::SetProtein {
            before: self.doc.protein.clone(),
            after,
        }
    }
}

impl PlotiumApp {
    pub fn export_protein(
        &self,
        id: MoleculeId,
        confirmed_only: bool,
    ) -> Result<serde_json::Value, String> {
        let state = &self.doc.protein;
        let molecule = state.molecule(id)?;
        let mut spectra = Vec::new();
        for (&dataset, _) in state.bindings.iter().filter(|(_, m)| **m == id) {
            let mut assignments: Vec<_> = state
                .assignments
                .iter()
                .filter(|a| a.dataset == dataset)
                .filter(|a| !confirmed_only || a.confirmed.is_some())
                .cloned()
                .collect();
            if confirmed_only {
                for a in &mut assignments {
                    a.candidates.retain(|c| Some(c.id) == a.confirmed);
                }
            }
            let mut observations = self.peak_table_3d(dataset)?;
            if let Some(rows) = observations["peaks"].as_array_mut() {
                rows.retain(|row| {
                    row["id"]
                        .as_u64()
                        .is_some_and(|id| assignments.iter().any(|a| a.peak.0 == id))
                });
            }
            spectra.push(
                serde_json::json!({"assignments": assignments, "observations": observations}),
            );
        }
        Ok(
            serde_json::json!({"version":1,"molecule":molecule,"spectra":spectra,
                "assignment_semantics": {
                    "confirmed": "Manually accepted interpretation; not automatic chemical or experimental validation",
                    "group_meaning": {
                        "unspecified": "Single atom, or an unresolved group interpretation that cannot be confirmed",
                        "one_of": "One member contributes this observation; its identity within the set is unresolved",
                        "shared": "All members share this observed resonance; no assertion of chemical equivalence or stereospecific assignment"
                    },
                    "chain_model": "Standard linear peptide chains; terminal atoms are restricted by sequence position. Protonation and modifications are not inferred"
                }
            }),
        )
    }
}

#[cfg(test)]
#[path = "protein_tests.rs"]
mod tests;
