//! Signed 3D frequency spectra and indexed slices for supported Bruker inputs.
use plotium_io::nmr_view::NmrSource;
use std::sync::Arc;

use nmr::ExecutionContext;
use nmr::axis::{AxisDomain, AxisUnit};
use nmr::processed::{ProcessedAxis, ProcessedDataset};
use nmr::processing::{
    DelaySource, DigitalFilterCorrection, FourierExponentSign, FourierTransform, FrequencyFrame,
    PhaseCorrection, ProcessingOperation, ProcessingOptions, ProcessingPlan, ReferenceSource,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum Axis3d {
    F1 = 0,
    F2 = 1,
    F3 = 2,
}

impl Axis3d {
    pub const ALL: [Self; 3] = [Self::F1, Self::F2, Self::F3];
}

#[derive(Clone, Debug)]
pub struct NmrVolume {
    source: NmrSource,
    shape: [usize; 3],
}

pub struct NmrPlane {
    pub fixed_axis: Axis3d,
    pub fixed_index: usize,
    pub fixed_ppm: f64,
    /// In F1/F2/F3 order with the fixed axis omitted.
    pub axes: [Axis3d; 2],
    pub ppm: [Vec<f64>; 2],
    /// Row major, second axis varies fastest.
    pub signed_values: Vec<f64>,
}

impl NmrVolume {
    pub fn process_source(source: &NmrSource) -> Result<Self, String> {
        Self::process(
            source,
            [VolumePhase::default(); 3],
            ProcessingOptions::default(),
            &mut ExecutionContext::default(),
        )
    }

    pub fn process(
        source: &NmrSource,
        phases: [VolumePhase; 3],
        options: ProcessingOptions,
        context: &mut ExecutionContext<'_>,
    ) -> Result<Self, String> {
        if source.axes().len() != 3 {
            return Err("3D processing requires three axes".into());
        }
        let mut operations = if let Some(raw) = source.dataset().as_raw() {
            recipe(raw)
        } else {
            Vec::new()
        };
        for (axis, phase) in phases.iter().enumerate() {
            PhaseCorrection::new(phase.p0_degrees, phase.p1_degrees, phase.pivot_fraction)
                .map_err(|e| e.to_string())?;
            if phase.p0_degrees != 0.0 || phase.p1_degrees != 0.0 {
                operations.push(ProcessingOperation::PhaseCorrection {
                    axis,
                    correction: PhaseCorrection::new(
                        phase.p0_degrees,
                        phase.p1_degrees,
                        phase.pivot_fraction,
                    )
                    .map_err(|e| e.to_string())?,
                });
            }
        }
        let output = ProcessingPlan::new(operations)
            .and_then(|plan| plan.apply_with_context(source.dataset(), options, context))
            .map_err(|e| e.to_string())?;
        Self::from_processed(NmrSource::new(Arc::new(output)).map_err(|e| e.to_string())?)
    }

    pub fn from_processed(source: NmrSource) -> Result<Self, String> {
        let spectrum = source
            .dataset()
            .as_processed()
            .ok_or("expected processed 3D spectrum")?;
        let shape: [usize; 3] = spectrum
            .descriptor()
            .logical_shape()
            .try_into()
            .map_err(|_| "expected three axes")?;
        if spectrum.descriptor().axes().iter().any(|axis| {
            axis.domain() != AxisDomain::Frequency || axis.unit() != Some(AxisUnit::Ppm)
        }) {
            return Err("expected three frequency axes in ppm".into());
        }
        Ok(Self { source, shape })
    }

    pub fn source(&self) -> &NmrSource {
        &self.source
    }
    pub fn processed(&self) -> &ProcessedDataset {
        self.source
            .dataset()
            .as_processed()
            .expect("validated volume")
    }
    pub fn axes(&self) -> &[ProcessedAxis] {
        self.processed().descriptor().axes()
    }

    pub fn plane_source(&self, fixed_axis: usize, index: usize) -> Result<NmrSource, String> {
        if fixed_axis >= 3 || index >= self.shape[fixed_axis] {
            return Err("plane is outside the spectrum".into());
        }
        // nmr 0.1.3 native reductions accept rank two only. This checked host
        // boundary records the fixed real component and surviving axis lineage.
        let remaining: Vec<_> = (0..3).filter(|axis| *axis != fixed_axis).collect();
        let descriptor = nmr::processed::ProcessedDescriptor::new(
            remaining
                .iter()
                .map(|axis| self.axes()[*axis].clone())
                .collect(),
        )
        .map_err(|e| e.to_string())?;
        let mut samples = Vec::new();
        let mut position = [0; 3];
        position[fixed_axis] = index;
        let mut component = [0; 3];
        for row in 0..self.shape[remaining[0]] {
            position[remaining[0]] = row;
            for rlane in 0..self.axes()[remaining[0]].component_count() {
                component[remaining[0]] = rlane;
                for col in 0..self.shape[remaining[1]] {
                    position[remaining[1]] = col;
                    for clane in 0..self.axes()[remaining[1]].component_count() {
                        component[remaining[1]] = clane;
                        samples.push(
                            self.processed()
                                .data()
                                .get(&position, &component)
                                .map_err(|e| e.to_string())?,
                        );
                    }
                }
            }
        }
        let axes = remaining
            .iter()
            .map(|axis| {
                nmr::external::ExternalAxisSource::Parent(nmr::provenance::InputAxisRef::new(
                    nmr::provenance::InputSlot::new(0),
                    *axis,
                ))
            })
            .collect();
        let declaration = nmr::external::ExternalAlgorithmDeclaration::new(
            "plotium.volume-plane",
            "1",
            "Indexed plane retaining surviving Cartesian components; fixed component zero",
        )
        .and_then(|d| {
            d.with_parameters(
                "application/json;version=1",
                format!("{{\"axis\":{fixed_axis},\"index\":{index},\"component\":0}}").into_bytes(),
            )
        })
        .map_err(|e| e.to_string())?;
        let output = self
            .source
            .dataset()
            .derive_external_processed(descriptor, samples, axes, declaration)
            .map_err(|e| e.to_string())?;
        NmrSource::new(Arc::new(output)).map_err(|e| e.to_string())
    }

    pub fn axis_coordinate(&self, axis: usize, index: usize) -> Result<f64, String> {
        self.axes()
            .get(axis)
            .ok_or("invalid axis")?
            .coordinate(index)
            .map_err(|e| e.to_string())
    }

    pub fn nearest(&self, axis: usize, ppm: f64) -> Result<usize, String> {
        if axis >= 3 || !ppm.is_finite() {
            return Err("invalid plane coordinate".into());
        }
        let coordinates = self.source.axes()[axis]
            .coordinate_values()
            .map_err(|e| e.to_string())?;
        let low = coordinates.iter().copied().fold(f64::INFINITY, f64::min);
        let high = coordinates
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        if ppm < low || ppm > high {
            return Err(format!("Enter a coordinate between {low} and {high} ppm"));
        }
        coordinates
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| (**a - ppm).abs().total_cmp(&(**b - ppm).abs()))
            .map(|(i, _)| i)
            .ok_or("empty axis".into())
    }

    pub fn shape(&self) -> [usize; 3] {
        self.shape
    }

    pub fn signed_value(&self, indices: [usize; 3]) -> Result<f64, String> {
        self.processed()
            .data()
            .get(&indices, &[0, 0, 0])
            .map_err(|e| e.to_string())
    }

    pub fn ppm(&self, axis: Axis3d, index: usize) -> Result<f64, String> {
        self.axes()[axis as usize]
            .coordinate(index)
            .map_err(|e| e.to_string())
    }

    pub fn slice(&self, fixed_axis: Axis3d, fixed_index: usize) -> Result<NmrPlane, String> {
        let fixed_ppm = self.ppm(fixed_axis, fixed_index)?;
        let axes = match fixed_axis {
            Axis3d::F1 => [Axis3d::F2, Axis3d::F3],
            Axis3d::F2 => [Axis3d::F1, Axis3d::F3],
            Axis3d::F3 => [Axis3d::F1, Axis3d::F2],
        };
        let [first, second] = axes.map(|a| a as usize);
        let mut indices = [0; 3];
        indices[fixed_axis as usize] = fixed_index;
        let mut signed_values = Vec::with_capacity(self.shape[first] * self.shape[second]);
        for i in 0..self.shape[first] {
            indices[first] = i;
            for j in 0..self.shape[second] {
                indices[second] = j;
                signed_values.push(self.signed_value(indices)?);
            }
        }
        let coordinates = |axis| {
            (0..self.shape[axis as usize])
                .map(|i| self.ppm(axis, i))
                .collect::<Result<Vec<_>, _>>()
        };
        Ok(NmrPlane {
            fixed_axis,
            fixed_index,
            fixed_ppm,
            axes,
            ppm: [coordinates(axes[0])?, coordinates(axes[1])?],
            signed_values,
        })
    }
}

fn recipe(raw: &nmr::raw::RawDataset) -> Vec<ProcessingOperation> {
    let mut operations = Vec::new();
    for (axis, description) in raw.descriptor().axes().iter().enumerate() {
        if matches!(
            description.kind(),
            nmr::raw::RawAxisKind::Indirect(nmr::raw::IndirectComponents::Encoded(_))
        ) {
            operations.push(ProcessingOperation::ComponentTransform { axis });
        }
    }
    for axis in (0..3).rev() {
        operations.push(ProcessingOperation::FourierTransform {
            axis,
            transform: FourierTransform::new(FourierExponentSign::Negative),
        });
        if axis == 2
            && matches!(
                raw.descriptor().axes()[axis].group_delay(),
                nmr::acquisition::GroupDelayState::Pending(delay) if delay.delay_points() > 0.0
            )
        {
            operations.push(ProcessingOperation::DigitalFilterCorrection {
                axis,
                correction: DigitalFilterCorrection::FrequencyDomainPhaseRampV1(
                    DelaySource::AxisEvidence,
                ),
            });
        }
        operations.push(ProcessingOperation::ResolveFrequencyFrame {
            axis,
            frame: FrequencyFrame::Ppm(ReferenceSource::AxisEvidence),
        });
    }
    operations
}

/// Phase convention follows nmr: exp(+i phase), position i/N.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VolumePhase {
    pub p0_degrees: f64,
    pub p1_degrees: f64,
    pub pivot_fraction: f64,
}
impl Default for VolumePhase {
    fn default() -> Self {
        Self {
            p0_degrees: 0.0,
            p1_degrees: 0.0,
            pivot_fraction: 0.5,
        }
    }
}
