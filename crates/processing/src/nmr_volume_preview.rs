//! Phase is pointwise: retain every component of a one-index slab, then let
//! the reference engine rotate it. No FFT or full-volume copy is needed.
use super::*;
use nmr::axis::AxisCoordinates;

impl NmrVolume {
    pub fn supports_local_phase_preview(&self) -> bool {
        self.axes().iter().all(|axis| {
            matches!(
                axis.component_basis(),
                nmr::processed::ComponentBasis::Scalar | nmr::processed::ComponentBasis::Cartesian
            )
        })
    }
    pub fn phase_preview_plane(
        &self,
        fixed: usize,
        index: usize,
        baseline: [VolumePhase; 3],
        draft: [VolumePhase; 3],
        context: &mut ExecutionContext<'_>,
    ) -> Result<NmrSource, String> {
        if !self.supports_local_phase_preview() {
            return Err("Local phase preview requires independent Cartesian components; shared or encoded bases are unsupported".into());
        }
        if fixed >= 3 || index >= self.shape()[fixed] {
            return Err("Preview plane is outside the spectrum".into());
        }
        let mut axes = self.axes().to_vec();
        let old = &axes[fixed];
        axes[fixed] = ProcessedAxis::new(
            old.role(),
            old.domain(),
            old.unit(),
            1,
            AxisCoordinates::Uniform {
                start: old.coordinate(index).map_err(|e| e.to_string())?,
                step: 1.0,
            },
            old.component_basis().clone(),
        )
        .map_err(|e| e.to_string())?;
        let descriptor =
            nmr::processed::ProcessedDescriptor::new(axes).map_err(|e| e.to_string())?;
        let mut shape = self.shape();
        shape[fixed] = 1;
        let mut samples = Vec::new();
        for a in 0..shape[0] {
            context.check_cancelled().map_err(|e| e.to_string())?;
            for ca in 0..self.axes()[0].component_count() {
                for b in 0..shape[1] {
                    for cb in 0..self.axes()[1].component_count() {
                        for c in 0..shape[2] {
                            let mut position = [a, b, c];
                            position[fixed] = index;
                            for cc in 0..self.axes()[2].component_count() {
                                samples.push(
                                    self.processed()
                                        .data()
                                        .get(&position, &[ca, cb, cc])
                                        .map_err(|e| e.to_string())?,
                                );
                            }
                        }
                    }
                }
            }
        }
        let lineage = (0..3)
            .map(|axis| {
                nmr::external::ExternalAxisSource::Parent(nmr::provenance::InputAxisRef::new(
                    nmr::provenance::InputSlot::new(0),
                    axis,
                ))
            })
            .collect();
        let declaration = nmr::external::ExternalAlgorithmDeclaration::new(
            "plotium.phase-preview-slab",
            "1",
            "All components at one fixed acquisition index",
        )
        .map_err(|e| e.to_string())?;
        let slab = self
            .source()
            .dataset()
            .derive_external_processed(descriptor, samples, lineage, declaration)
            .map_err(|e| e.to_string())?;
        let mut operations = Vec::new();
        for axis in 0..3 {
            baseline[axis].validate()?;
            draft[axis].validate()?;
            // R(draft) R(baseline)^-1. Folding each pivot into the intercept
            // avoids applying the committed correction a second time.
            let intercept = |p: VolumePhase| p.p0_degrees - p.p1_degrees * p.pivot_fraction;
            let mut p0 = intercept(draft[axis]) - intercept(baseline[axis]);
            let mut p1 = draft[axis].p1_degrees - baseline[axis].p1_degrees;
            if axis == fixed {
                p0 += p1 * index as f64 / self.shape()[axis] as f64;
                p1 = 0.0;
            }
            if p0 != 0.0 || p1 != 0.0 {
                if !self.supports_phase(axis) {
                    return Err(format!(
                        "F{} needs complex frequency-domain components",
                        axis + 1
                    ));
                }
                operations.push(ProcessingOperation::PhaseCorrection {
                    axis,
                    correction: PhaseCorrection::new(p0, p1, 0.0).map_err(|e| e.to_string())?,
                });
            }
        }
        if operations.is_empty() {
            return Self::from_processed(
                NmrSource::new(Arc::new(slab)).map_err(|e| e.to_string())?,
            )?
            .plane_source(fixed, 0);
        }
        let output = ProcessingPlan::new(operations)
            .and_then(|plan| plan.apply_with_context(&slab, ProcessingOptions::default(), context))
            .map_err(|e| e.to_string())?;
        Self::from_processed(NmrSource::new(Arc::new(output)).map_err(|e| e.to_string())?)?
            .plane_source(fixed, 0)
    }
}
