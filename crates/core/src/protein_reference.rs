//! Absolute acquisition-axis reference settings, separate from phase processing.
use crate::state::{DatasetId, PlotiumApp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShiftReference {
    pub offsets_ppm: [f64; 3],
    pub source: String,
    pub confirmed: bool,
}
impl ShiftReference {
    pub fn validate(&self) -> Result<(), String> {
        if self.offsets_ppm.iter().any(|v| !v.is_finite()) {
            return Err("Enter finite reference offsets in ppm".into());
        }
        if (self.confirmed || self.offsets_ppm != [0.0; 3]) && self.source.trim().is_empty() {
            return Err("Record the reference standard or comparison source".into());
        }
        Ok(())
    }
}
impl PlotiumApp {
    pub fn set_shift_reference(
        &mut self,
        dataset: DatasetId,
        reference: ShiftReference,
    ) -> Result<(), String> {
        reference.validate()?;
        let di = self
            .doc
            .dataset_index(dataset)
            .ok_or("Select an existing spectrum")?;
        let mut after = self.doc.datasets[di].clone();
        let nmr = after.as_nmr2d_mut().ok_or("Select a 3D spectrum")?;
        let v = nmr.volume.as_mut().ok_or("Select a 3D spectrum")?;
        let delta =
            std::array::from_fn(|a| reference.offsets_ppm[a] - v.peaks.reference.offsets_ppm[a]);
        v.processed = v.processed.shifted(delta)?;
        for peak in &mut v.peaks.marks {
            for (coordinate, shift) in peak.ppm.iter_mut().zip(delta) {
                *coordinate += shift;
            }
        }
        v.peaks.reference = reference;
        v.refresh_peaks()?;
        let (axis, index) = (v.fixed_axis, v.index);
        nmr.select_volume_plane(axis, index)?;
        let full = crate::state::CanvasViewport::from_figure(&nmr.processed_figure);
        let remaining: Vec<_> = (0..3).filter(|a| *a != axis).collect();
        let mut actions = vec![self.volume_install_action(dataset, after)?];
        for (canvas, page) in self.doc.canvases.iter().enumerate() {
            for object in &page.objects {
                if let Some(plot) = object.plot().filter(|p| p.display_owner == Some(dataset)) {
                    let before = plot.viewport.clone();
                    let mut after = before.clone();
                    after.full_x = full.full_x;
                    after.full_y = full.full_y;
                    after.view_x = crate::state::AxisRange::new(
                        before.view_x.min + delta[remaining[1]],
                        before.view_x.max + delta[remaining[1]],
                    );
                    after.view_y = crate::state::AxisRange::new(
                        before.view_y.min + delta[remaining[0]],
                        before.view_y.max + delta[remaining[0]],
                    );
                    actions.push(crate::actions::Action::SetObjectViewport {
                        canvas,
                        object: object.id,
                        before,
                        after,
                    });
                }
            }
        }
        self.try_execute_action(crate::actions::Action::Composite(actions))
            .map_err(|e| e.to_string())
    }

    /// Compare every confirmed, unambiguous single-atom observation without averaging.
    pub fn reference_differences(
        &self,
        left: DatasetId,
        right: DatasetId,
    ) -> Result<Vec<ReferenceDifference>, String> {
        self.check_reference_comparison(left, right)?;
        let observations = |id| -> Result<Vec<_>, String> {
            let v = crate::protein::volume(&self.doc, id)?;
            let peaks: HashMap<_, _> = v.peaks.marks.iter().map(|p| (p.id, p)).collect();
            let mut rows = Vec::new();
            for a in self
                .doc
                .protein
                .assignments
                .iter()
                .filter(|a| a.dataset == id)
            {
                let Some(c) = a.candidates.iter().find(|c| Some(c.id) == a.confirmed) else {
                    continue;
                };
                let peak = peaks.get(&a.peak).ok_or("Assignment peak is missing")?;
                for (axis, target) in c.targets.iter().enumerate() {
                    if let Some(target) = target.as_ref().filter(|t| t.members.len() == 1) {
                        rows.push((
                            a.peak,
                            axis,
                            target.clone(),
                            peak.ppm[axis],
                            v.processed.axes()[axis].nucleus().map(str::to_owned),
                        ));
                    }
                }
            }
            Ok(rows)
        };
        let mut differences = Vec::new();
        let left = observations(left)?;
        let right = observations(right)?;
        let mut by_atom = HashMap::<_, Vec<_>>::new();
        for b in &right {
            if let Some(nucleus) = &b.4 {
                by_atom
                    .entry((b.2.residue, b.2.members.clone(), nucleus.clone()))
                    .or_default()
                    .push(b);
            }
        }
        for a in left {
            let Some(nucleus) = &a.4 else {
                continue;
            };
            let Some(matches) = by_atom.get(&(a.2.residue, a.2.members.clone(), nucleus.clone()))
            else {
                continue;
            };
            for b in matches {
                if a.2 == b.2 && a.4.is_some() && a.4 == b.4 {
                    differences.push(ReferenceDifference {
                        left_peak: a.0,
                        right_peak: b.0,
                        left_axis: a.1,
                        right_axis: b.1,
                        delta_ppm: a.3 - b.3,
                    });
                }
            }
        }
        Ok(differences)
    }

    /// Explicit sample identity is required even when names/conditions appear identical.
    pub fn check_reference_comparison(
        &self,
        left: DatasetId,
        right: DatasetId,
    ) -> Result<(), String> {
        let context = &self.doc.protein.context;
        context.validate(&self.doc)?;
        let a = context
            .experiments
            .get(&left)
            .ok_or("Record sample conditions for both spectra")?;
        let b = context
            .experiments
            .get(&right)
            .ok_or("Record sample conditions for both spectra")?;
        if a.sample != b.sample {
            return Err("Compare spectra from the same sample conditions; different samples cannot be merged".into());
        }
        for id in [left, right] {
            let v = crate::protein::volume(&self.doc, id)?;
            if !v.peaks.reference.confirmed {
                return Err("Review and confirm both spectrum references first".into());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReferenceDifference {
    pub left_peak: crate::state::Peak3DId,
    pub right_peak: crate::state::Peak3DId,
    pub left_axis: usize,
    pub right_axis: usize,
    /// Left corrected ppm minus right corrected ppm; no automatic verdict.
    pub delta_ppm: f64,
}
