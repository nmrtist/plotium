//! Shared undoable edits for a three-dimensional spectrum's displayed plane.
use super::*;
use plotium_processing::nmr_volume::VolumePhase;

impl PlotiumApp {
    /// Preserve other spectra's contour layers in a disposable phase display.
    pub fn volume_preview_companion_contours(
        &mut self,
        series: &[SeriesBinding],
    ) -> Option<Vec<plotium_figure::Contour>> {
        let mut contours = Vec::new();
        for series in series.iter().filter(|s| {
            s.visible && matches!(s.encoding, plotium_figure::SeriesEncoding::Contour(_))
        }) {
            self.prepare_contour_series(series);
            let binding = DataBinding {
                coordinates: None,
                series: vec![series.clone()],
            };
            if self.binding_contours_pending(&binding) {
                return None;
            }
            contours.extend(self.build_encoded_series_figure(series)?.contours);
        }
        Some(contours)
    }
    /// Resolve thresholds against committed data once, before preview rotation.
    pub fn volume_preview_contour_key(
        &mut self,
        series: &SeriesBinding,
    ) -> Result<ContourGeometryCacheKey, String> {
        let plotium_figure::SeriesEncoding::Contour(spec) = &series.encoding else {
            return Err("Choose a contour display for phase preview".into());
        };
        let dataset = self
            .doc
            .dataset_by_id(series.source.resource)
            .ok_or("Spectrum was removed")?;
        let field = FieldRef {
            resource: series.source.resource,
            field: series.source.field,
        };
        let version = self
            .session
            .compute
            .field_version_for(field)
            .map_err(|e| format!("{e:?}"))?;
        let snapshot = dataset
            .field_snapshot(field.field, version, None)
            .ok_or("Preview field is unavailable")?;
        let summary = snapshot.summary.ok_or("Preview summary is unavailable")?;
        let grid = std::sync::Arc::new(
            snapshot
                .payload
                .scalar_grid()
                .ok_or("Preview needs a plane")?
                .clone(),
        );
        if let ContourResolution::Pending(keys) =
            resolve_contour_levels(snapshot.source, spec, summary, |key| {
                self.session.compute.estimate_for(key).cloned()
            })
        {
            for key in keys {
                let estimate =
                    super::compute::compute_field::run_estimate_field(key.clone(), grid.clone())?;
                if !self
                    .session
                    .compute
                    .finish_estimate(key, estimate, Some(version))
                {
                    return Err("Preview estimate became stale".into());
                }
            }
        }
        let ContourResolution::Ready { levels, .. } =
            resolve_contour_levels(snapshot.source, spec, summary, |key| {
                self.session.compute.estimate_for(key).cloned()
            })
        else {
            return Err("Contour thresholds are unavailable".into());
        };
        Ok(ContourGeometryCacheKey {
            source: snapshot.source,
            levels,
        })
    }
    pub fn select_nmr_volume_plane(
        &mut self,
        dataset: DatasetId,
        axis: usize,
        index: usize,
    ) -> Result<(), String> {
        let di = self
            .doc
            .dataset_index(dataset)
            .ok_or("Select an existing spectrum")?;
        let mut after = self.doc.datasets[di].clone();
        after
            .as_nmr2d_mut()
            .ok_or("Select a three-dimensional spectrum")?
            .select_volume_plane(axis, index)?;
        let action = self.volume_install_action(dataset, after)?;
        self.try_execute_action(action).map_err(|e| e.to_string())?;
        self.session.ui.inspect_cursor_pin = None;
        self.session.ui.delta_cursor_anchor = None;
        self.session.ui.delta_cursor_pin = None;
        self.session.ui.slice = None;
        Ok(())
    }
    pub(crate) fn volume_install_action(
        &self,
        dataset: DatasetId,
        after: Dataset,
    ) -> Result<crate::actions::Action, String> {
        let di = self
            .doc
            .dataset_index(dataset)
            .ok_or("Select an existing spectrum")?;
        let axis = after
            .as_nmr2d()
            .and_then(|n| n.volume.as_ref())
            .ok_or("Select a three-dimensional spectrum")?
            .fixed_axis;
        let old_axis = self.doc.datasets[di]
            .as_nmr2d()
            .and_then(|n| n.volume.as_ref())
            .ok_or("Select a three-dimensional spectrum")?
            .fixed_axis;
        let mut changes = Vec::new();
        if old_axis != axis {
            let old_axes: Vec<_> = (0..3).filter(|a| *a != old_axis).collect();
            let new_axes: Vec<_> = (0..3).filter(|a| *a != axis).collect();
            let full = CanvasViewport::from_figure(&after.as_nmr2d().unwrap().processed_figure);
            for (ci, canvas) in self.doc.canvases.iter().enumerate() {
                for object in &canvas.objects {
                    let Some(plot) = object.plot().filter(|p| p.display_owner == Some(dataset))
                    else {
                        continue;
                    };
                    let before = plot.viewport.clone();
                    let mut next = before.clone();
                    let ranges = [before.view_y, before.view_x];
                    next.full_x = full.full_x;
                    next.full_y = full.full_y;
                    next.view_x = old_axes
                        .iter()
                        .position(|a| *a == new_axes[1])
                        .map(|i| ranges[i])
                        .unwrap_or(full.full_x);
                    next.view_y = old_axes
                        .iter()
                        .position(|a| *a == new_axes[0])
                        .map(|i| ranges[i])
                        .unwrap_or(full.full_y);
                    changes.push(crate::actions::Action::SetObjectViewport {
                        canvas: ci,
                        object: object.id,
                        before,
                        after: next,
                    });
                }
            }
        }
        changes.insert(
            0,
            crate::actions::Action::InstallScientificState {
                before: vec![self.doc.datasets[di].clone()],
                after: vec![after],
            },
        );
        Ok(crate::actions::Action::Composite(changes))
    }
    pub fn set_nmr_volume_phases(
        &mut self,
        dataset: DatasetId,
        phases: [VolumePhase; 3],
    ) -> Result<(), String> {
        let di = self
            .doc
            .dataset_index(dataset)
            .ok_or("Select an existing spectrum")?;
        let mut result = self.doc.datasets[di]
            .as_nmr2d()
            .ok_or("Select a three-dimensional spectrum")?
            .clone();
        let expected = result
            .volume
            .as_ref()
            .ok_or("Select a three-dimensional spectrum")?
            .phases;
        result.set_volume_phases(phases)?;
        self.install_nmr_volume_result(dataset, expected, result)
    }
    pub fn install_nmr_volume_result(
        &mut self,
        dataset: DatasetId,
        expected_phases: [VolumePhase; 3],
        mut result: Nmr2DDataset,
    ) -> Result<(), String> {
        let di = self
            .doc
            .dataset_index(dataset)
            .ok_or("The target spectrum was removed")?;
        let current = self.doc.datasets[di]
            .as_nmr2d()
            .ok_or("The target spectrum changed")?;
        let volume = current
            .volume
            .as_ref()
            .ok_or("The target is no longer three-dimensional")?;
        let incoming = result
            .volume
            .as_ref()
            .ok_or("Processing produced no three-dimensional result")?;
        if volume.phases != expected_phases
            || !std::sync::Arc::ptr_eq(volume.source.dataset(), incoming.source.dataset())
        {
            return Err(
                "Discarded an outdated phase result; apply the current settings again".into(),
            );
        }
        result.select_volume_plane(volume.fixed_axis, volume.index)?;
        // Navigation may have advanced while this phase job was running.
        if let Some(incoming) = result.volume.as_mut() {
            incoming.anchor = volume.anchor;
            let delta = std::array::from_fn(|a| {
                volume.peaks.reference.offsets_ppm[a] - incoming.peaks.reference.offsets_ppm[a]
            });
            incoming.processed = incoming.processed.shifted(delta)?;
            incoming.peaks = volume.peaks.clone();
            incoming.refresh_peaks()?;
        }
        result.select_volume_plane(volume.fixed_axis, volume.index)?;
        // Preserve edits made while the numerical worker was running.
        let mut replacement = current.clone();
        replacement.volume = result.volume;
        replacement.data = result.data;
        replacement.native_base = result.native_base;
        replacement.native_processed = result.native_processed;
        replacement.base = result.base;
        replacement.processed = result.processed;
        replacement.processed_figure = result.processed_figure;
        self.try_execute_action(crate::actions::Action::InstallScientificState {
            before: vec![self.doc.datasets[di].clone()],
            after: vec![Dataset::Nmr2D(Box::new(replacement))],
        })
        .map_err(|e| e.to_string())?;
        Ok(())
    }
}
