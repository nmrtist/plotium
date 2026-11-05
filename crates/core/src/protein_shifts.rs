//! Reviewable chemical shifts derived from acquisition-axis observations, never averaged.
use crate::protein::{AtomTarget, CandidateId};
use crate::protein_context::{SampleCondition, SampleConditionId};
use crate::protein_reference::ShiftReference;
use crate::state::{DatasetId, Peak3DId, PlotiumApp};
use serde::{Deserialize, Serialize};

/// Stable source identity; axis is an acquisition F1/F2/F3, never a display axis.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservationId {
    pub dataset: DatasetId,
    pub peak: Peak3DId,
    pub candidate: CandidateId,
    pub axis: usize,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShiftObservation {
    pub id: ObservationId,
    /// Full conditions are evidence: editing a record in place invalidates review.
    pub sample: SampleCondition,
    pub target: AtomTarget,
    pub ppm: f64,
    pub nucleus: Option<String>,
    pub reference: ShiftReference,
    pub assignment_confirmed: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShiftDecision {
    Pending,
    Selected,
    Excluded,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShiftReview {
    pub observation: ShiftObservation,
    /// Unknown uncertainty stays None; it is never inferred from peak height.
    pub uncertainty_ppm: Option<f64>,
    pub decision: ShiftDecision,
    pub note: String,
}
pub(crate) fn validate_reviews(reviews: &[ShiftReview]) -> Result<(), String> {
    for (i, r) in reviews.iter().enumerate() {
        if r.observation.id.axis >= 3
            || !r.observation.ppm.is_finite()
            || r.uncertainty_ppm.is_some_and(|v| !v.is_finite() || v < 0.)
            || reviews[..i]
                .iter()
                .any(|a| a.observation.id == r.observation.id)
        {
            return Err("Invalid or duplicate chemical-shift review".into());
        }
        r.observation.reference.validate()?;
    }
    Ok(())
}
/// One target (including its group meaning) under one explicitly identified condition.
#[derive(Clone, Debug)]
pub struct Resonance {
    pub sample: SampleConditionId,
    pub target: AtomTarget,
    pub nucleus: Option<String>,
    pub observations: Vec<ShiftObservation>,
}
impl PlotiumApp {
    pub fn chemical_shifts(&self) -> Result<Vec<Resonance>, String> {
        let mut rows: Vec<Resonance> = Vec::new();
        for a in &self.doc.protein.assignments {
            let experiment = self.doc.protein.context.experiments.get(&a.dataset).ok_or(
                "Record sample conditions for every assigned spectrum before listing shifts",
            )?;
            let sample = self
                .doc
                .protein
                .context
                .samples
                .iter()
                .find(|s| s.id == experiment.sample)
                .ok_or("Sample conditions are missing")?;
            let v = crate::protein::volume(&self.doc, a.dataset)?;
            let peak = v
                .peaks
                .marks
                .iter()
                .find(|p| p.id == a.peak)
                .ok_or("Assignment peak is missing")?;
            for c in &a.candidates {
                for (axis, target) in c.targets.iter().enumerate() {
                    let Some(target) = target else { continue };
                    let nucleus = v.processed.axes()[axis].nucleus().map(str::to_owned);
                    let observation = ShiftObservation {
                        id: ObservationId {
                            dataset: a.dataset,
                            peak: a.peak,
                            candidate: c.id,
                            axis,
                        },
                        sample: sample.clone(),
                        target: target.clone(),
                        ppm: peak.ppm[axis],
                        nucleus: nucleus.clone(),
                        reference: v.peaks.reference.clone(),
                        assignment_confirmed: a.confirmed == Some(c.id),
                    };
                    let index = rows.iter().position(|r| {
                        r.sample == experiment.sample && r.target == *target && r.nucleus == nucleus
                    });
                    if let Some(i) = index {
                        rows[i].observations.push(observation);
                    } else {
                        rows.push(Resonance {
                            sample: experiment.sample,
                            target: target.clone(),
                            nucleus,
                            observations: vec![observation],
                        });
                    }
                }
            }
        }
        Ok(rows)
    }
    /// A review applies only while its full observation snapshot still matches.
    pub fn shift_review(&self, observation: &ShiftObservation) -> Option<&ShiftReview> {
        self.doc
            .protein
            .shift_reviews
            .iter()
            .find(|r| r.observation == *observation)
    }
    pub fn review_shift(&mut self, review: ShiftReview) -> Result<(), String> {
        if !self
            .chemical_shifts()?
            .iter()
            .flat_map(|r| &r.observations)
            .any(|o| *o == review.observation)
        {
            return Err(
                "Observation changed or was deleted; reload the chemical-shift list".into(),
            );
        }
        if review.decision == ShiftDecision::Selected
            && (!review.observation.assignment_confirmed || !review.observation.reference.confirmed)
        {
            return Err(
                "Confirm the assignment and spectrum reference before selecting a shift".into(),
            );
        }
        let mut state = self.doc.protein.clone();
        state
            .shift_reviews
            .retain(|r| r.observation.id != review.observation.id);
        state.shift_reviews.push(review);
        self.commit_protein(state)
    }
}
