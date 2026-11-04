//! Persistent observations in acquisition-axis order, independent of display orientation.
use super::*;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Peak3DId(pub u64);
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Peak3DMark {
    pub id: Peak3DId,
    pub ppm: [f64; 3],
    pub intensity: f64,
    pub note: String,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Peak3DSet {
    pub marks: Vec<Peak3DMark>,
    pub next_id: u64,
}
impl NmrVolumeState {
    pub fn peak_indices(&self, ppm: [f64; 3]) -> Result<[usize; 3], String> {
        Ok([
            self.processed.nearest(0, ppm[0])?,
            self.processed.nearest(1, ppm[1])?,
            self.processed.nearest(2, ppm[2])?,
        ])
    }
    pub fn refresh_peaks(&mut self) -> Result<(), String> {
        let mut ids = std::collections::HashSet::new();
        let mut peaks = self.peaks.clone();
        for peak in &mut peaks.marks {
            if !ids.insert(peak.id) {
                return Err("Duplicate 3D peak ID".into());
            }
            peak.intensity = self.processed.signed_value(self.peak_indices(peak.ppm)?)?;
        }
        let minimum = peaks
            .marks
            .iter()
            .map(|p| p.id.0.checked_add(1).ok_or("3D peak ID exhausted"))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .max()
            .unwrap_or(0);
        peaks.next_id = peaks.next_id.max(minimum);
        self.peaks = peaks;
        Ok(())
    }
}
impl PlotiumApp {
    /// A standalone observation table with acquisition and processing provenance.
    pub fn peak_table_3d(&self, dataset: DatasetId) -> Result<serde_json::Value, String> {
        let nmr = self
            .doc
            .dataset_index(dataset)
            .and_then(|i| self.doc.datasets[i].as_nmr2d())
            .ok_or("Select a three-dimensional spectrum")?;
        let volume = nmr
            .volume
            .as_ref()
            .ok_or("Select a three-dimensional spectrum")?;
        let axes = volume
            .processed
            .axes()
            .iter()
            .enumerate()
            .map(|(axis, a)| {
                serde_json::json!({"axis":axis,"dimension":format!("F{}", axis + 1),
                "nucleus":a.nucleus(),"unit":"ppm"})
            })
            .collect::<Vec<_>>();
        let digest = |source: &plotium_io::nmr_view::NmrSource| -> String {
            source
                .dataset()
                .canonical_digests()
                .dataset()
                .as_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect()
        };
        Ok(serde_json::json!({
            "version":1, "dataset":dataset, "dataset_name":nmr.name,
            "source": {
                "identity":nmr.acquisition_identity,
                "canonical_dataset_sha256":digest(&volume.source)
            },
            "processing": {
                "phases":volume.phases,
                "canonical_dataset_sha256":digest(volume.processed.source())
            },
            "axes":axes, "peaks":volume.peaks.marks,
            "position":"manual observation in acquisition-axis order; not a fitted centre",
            "intensity":"signed nearest-grid sample; not integrated volume"
        }))
    }

    pub fn add_peak_3d(&mut self, dataset: DatasetId, ppm: [f64; 3]) -> Result<Peak3DId, String> {
        let mut created = Peak3DId(0);
        self.edit_volume_peaks(dataset, |v| {
            let intensity = v.processed.signed_value(v.peak_indices(ppm)?)?;
            created = Peak3DId(v.peaks.next_id);
            v.peaks.next_id = v
                .peaks
                .next_id
                .checked_add(1)
                .ok_or("3D peak ID exhausted")?;
            v.peaks.marks.push(Peak3DMark {
                id: created,
                ppm,
                intensity,
                note: String::new(),
            });
            Ok(())
        })?;
        Ok(created)
    }
    pub fn edit_peak_3d(
        &mut self,
        dataset: DatasetId,
        id: Peak3DId,
        ppm: [f64; 3],
        note: String,
    ) -> Result<(), String> {
        self.edit_volume_peaks(dataset, |v| {
            let intensity = v.processed.signed_value(v.peak_indices(ppm)?)?;
            let mark = v
                .peaks
                .marks
                .iter_mut()
                .find(|p| p.id == id)
                .ok_or("Select an existing 3D peak")?;
            *mark = Peak3DMark {
                id,
                ppm,
                intensity,
                note,
            };
            Ok(())
        })
    }
    pub fn delete_peak_3d(&mut self, dataset: DatasetId, id: Peak3DId) -> Result<(), String> {
        self.edit_volume_peaks(dataset, |v| {
            let i = v
                .peaks
                .marks
                .iter()
                .position(|p| p.id == id)
                .ok_or("Select an existing 3D peak")?;
            v.peaks.marks.remove(i);
            Ok(())
        })
    }
    pub fn locate_peak_3d(&mut self, dataset: DatasetId, id: Peak3DId) -> Result<(), String> {
        let di = self
            .doc
            .dataset_index(dataset)
            .ok_or("Select an existing spectrum")?;
        let v = self.doc.datasets[di]
            .as_nmr2d()
            .and_then(|n| n.volume.as_ref())
            .ok_or("Select a three-dimensional spectrum")?;
        let peak = v
            .peaks
            .marks
            .iter()
            .find(|p| p.id == id)
            .ok_or("Select an existing 3D peak")?;
        let ppm = peak.ppm;
        let axis = v.fixed_axis;
        let index = v.peak_indices(ppm)?[axis];
        let remaining: Vec<_> = (0..3).filter(|a| *a != axis).collect();
        let mut after = self.doc.datasets[di].clone();
        after
            .as_nmr2d_mut()
            .ok_or("Select a three-dimensional spectrum")?
            .select_volume_plane(axis, index)?;
        let mut actions = vec![self.volume_install_action(dataset, after)?];
        for (ci, canvas) in self.doc.canvases.iter().enumerate() {
            for object in &canvas.objects {
                let Some(plot) = object.plot().filter(|p| p.display_owner == Some(dataset)) else {
                    continue;
                };
                let before = plot.viewport.clone();
                let mut after = before.clone();
                let center = |range: AxisRange, full: AxisRange, value: f64| {
                    let half = range.span() * 0.5;
                    AxisRange::new(value - half, value + half).clamp_to(full)
                };
                after.view_x = center(before.view_x, before.full_x, ppm[remaining[1]]);
                after.view_y = center(before.view_y, before.full_y, ppm[remaining[0]]);
                after.auto_y = false;
                actions.push(crate::actions::Action::SetObjectViewport {
                    canvas: ci,
                    object: object.id,
                    before,
                    after,
                });
            }
        }
        self.try_execute_action(crate::actions::Action::Composite(actions))
            .map_err(|e| e.to_string())?;
        self.session.ui.selected_peak_3d = Some((dataset, id));
        self.session.ui.inspect_cursor_pin = None;
        self.session.ui.delta_cursor_anchor = None;
        self.session.ui.delta_cursor_pin = None;
        self.session.ui.slice = None;
        Ok(())
    }
    pub(crate) fn edit_volume_peaks(
        &mut self,
        dataset: DatasetId,
        edit: impl FnOnce(&mut NmrVolumeState) -> Result<(), String>,
    ) -> Result<(), String> {
        let di = self
            .doc
            .dataset_index(dataset)
            .ok_or("Select an existing spectrum")?;
        let before = self.doc.datasets[di].clone();
        let mut after = before.clone();
        let v = after
            .as_nmr2d_mut()
            .and_then(|n| n.volume.as_mut())
            .ok_or("Select a three-dimensional spectrum")?;
        edit(v)?;
        self.try_execute_action(crate::actions::Action::InstallScientificState {
            before: vec![before],
            after: vec![after],
        })
        .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "peaks3d_tests.rs"]
mod tests;
