//! Warnings about a reduction that ran but may mislead: a wrong edge, too little
//! post-edge to normalize on, or noise where the transform reads its signal.
use super::XasDataset;
use super::xas::XasCalibrationError;
use plotium_processing::xas::{edge_energy, nearest_edge};
use serde_json::{Value, json};

/// Chemical shifts stay well inside this, in eV.
const MAX_E0_OFFSET: f64 = 30.0;
const MIN_POST_EDGE_RANGE: f64 = 150.0;
const NOISE_WINDOW: f64 = 3.0;
const MAX_NOISE_RATIO: f64 = 0.5;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct XasCheck {
    pub code: &'static str,
    pub message: String,
}

impl XasDataset {
    pub fn checks(&self) -> Vec<XasCheck> {
        let mut checks = Vec::new();
        if let Some(edge) = &self.processed.edge {
            if let Some((element, name, energy)) = self.expected_edge(edge.e0)
                && (edge.e0 - energy).abs() > MAX_E0_OFFSET
            {
                checks.push(XasCheck {
                    code: "e0_far_from_table",
                    message: format!(
                        "E0 {:.1} eV is {:.1} eV from {element} {name}.",
                        edge.e0,
                        (edge.e0 - energy).abs()
                    ),
                });
            }
            if edge.norm2 - edge.norm1 < MIN_POST_EDGE_RANGE {
                checks.push(XasCheck {
                    code: "short_post_edge",
                    message: format!(
                        "The post-edge range is {:.0} eV wide, so the edge step is uncertain.",
                        edge.norm2 - edge.norm1
                    ),
                });
            }
        }
        if let Some(ratio) = self.high_k_noise()
            && ratio > MAX_NOISE_RATIO
        {
            checks.push(XasCheck {
                code: "noisy_high_k",
                message: format!(
                    "Point-to-point noise is {:.0}% of the k-weighted signal in the last \
                     {NOISE_WINDOW:.0} 1/Å of the transform range; lower its upper k.",
                    ratio * 100.0
                ),
            });
        }
        checks
    }

    /// The edge the file declares, otherwise the one closest to `e0`.
    fn expected_edge(&self, e0: f64) -> Option<(&str, &str, f64)> {
        if let (Some(element), Some(edge)) = (&self.scan.element, &self.scan.edge)
            && let Some(energy) = edge_energy(element, edge)
        {
            return Some((element, edge, energy));
        }
        nearest_edge(e0)
    }

    /// Noise from second differences, which remove the slow EXAFS oscillation,
    /// relative to the RMS of k-weighted chi(k) just below the transform's end.
    pub(super) fn high_k_noise(&self) -> Option<f64> {
        let exafs = self.processed.exafs.as_ref()?;
        let end = self.params.transform.kmax.min(*exafs.k.last()?);
        let values: Vec<f64> = exafs
            .k
            .iter()
            .zip(&exafs.chi_weighted)
            .filter(|(k, _)| (end - NOISE_WINDOW..=end).contains(*k))
            .map(|(_, value)| *value)
            .collect();
        let rms = |sum: f64, count: usize| (sum / count as f64).sqrt();
        let curvature: f64 = values
            .windows(3)
            .map(|w| (w[0] - 2.0 * w[1] + w[2]).powi(2))
            .sum();
        let signal = rms(values.iter().map(|v| v * v).sum(), values.len());
        (values.len() > 3 && signal > 0.0).then(|| rms(curvature / 6.0, values.len() - 2) / signal)
    }

    pub fn calibration_summary(&self) -> Value {
        match self.calibration() {
            Ok(target) => json!({
                "available":true,"element":target.element,"edge":target.edge,
                "energy":target.energy,"energy_shift":target.shift
            }),
            Err(XasCalibrationError::NoReference) => {
                json!({"available":false,"reason":"The scan has no reference channel."})
            }
            Err(XasCalibrationError::NoReferenceEdge) => {
                json!({"available":false,"reason":"The reference channel shows no edge."})
            }
            Err(XasCalibrationError::CoarseReference(step)) => json!({
                "available":false,
                "reason":format!("The reference steps {step:.1} eV at its edge.")
            }),
        }
    }
}
