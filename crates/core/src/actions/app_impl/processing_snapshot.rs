//! Install scientific source and linked table snapshots without rerunning solvers.
use super::*;
impl PlotiumApp {
    pub(super) fn install_processing_snapshots(
        &mut self,
        snapshots: &[Dataset],
    ) -> Result<(), ActionApplyError> {
        let mut versions = Vec::new();
        for snapshot in snapshots {
            for field in snapshot.field_descriptors() {
                let version = self.session.compute.reserve_field_version().map_err(|e| {
                    ActionApplyError::InvalidValue(format!(
                        "Cannot version processing field: {e:?}"
                    ))
                })?;
                versions.push(crate::state::VersionedFieldRef {
                    field: crate::state::FieldRef {
                        resource: snapshot.resource_id(),
                        field: field.id,
                    },
                    version,
                });
            }
        }
        for snapshot in snapshots {
            let index = self
                .doc
                .dataset_index(snapshot.resource_id())
                .ok_or_else(|| ActionApplyError::StaleTarget(snapshot.resource_id().to_string()))?;
            let mut next = snapshot.clone();
            match (&self.doc.datasets[index], &mut next) {
                (Dataset::Nmr(current), Dataset::Nmr(next)) => {
                    next.next_step_id = next.next_step_id.max(current.next_step_id)
                }
                (Dataset::Nmr2D(current), Dataset::Nmr2D(next)) => {
                    next.next_step_id = next.next_step_id.max(current.next_step_id)
                }
                (Dataset::Xps(current), Dataset::Xps(next)) => {
                    next.next_step_id = next.next_step_id.max(current.next_step_id)
                }
                _ => {}
            }
            for kind in [
                crate::state::ComputeKind::Dosy,
                crate::state::ComputeKind::Ilt,
            ] {
                self.session.compute.cancel(snapshot.resource_id(), kind);
            }
            self.doc.datasets[index] = next;
        }
        for source in versions {
            self.session.compute.promote_field_version(source, None);
        }
        for snapshot in snapshots {
            if let Some(index) = self.doc.dataset_index(snapshot.resource_id()) {
                self.rebuild_canvases_for(index);
            }
        }
        Ok(())
    }
}
