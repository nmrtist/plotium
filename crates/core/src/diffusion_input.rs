//! One immutable encoding value shared by settings, invocation and provenance.
use plotium_analysis::diffusion_encoding::{
    jeol_bpp_dste_time, rectangular_b_values, validate_b_values,
};
use plotium_io::{DiffusionMeta, PseudoKind, nmr_series::NmrSeriesSource};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Identity in the immutable acquired increment sequence, never a display row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct IncrementId(pub u64);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiffusionIncrement {
    pub id: IncrementId,
    pub b: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "model", rename_all = "snake_case")]
pub enum DiffusionModel {
    DirectB {
        unit: String,
    },
    RectangularPgse {
        gamma: f64,
        delta: f64,
        big_delta: f64,
        tau: f64,
    },
    JeolBppDste {
        gamma: f64,
        delta: f64,
        big_delta: f64,
        tau_1: f64,
        tau_2: f64,
    },
    /// Explicit pre-existing model parameters; no inference from a sequence name.
    StejskalTanner {
        gamma: f64,
        delta: f64,
        big_delta: f64,
        tau: f64,
        shape_factor: f64,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiffusionInput {
    pub model: DiffusionModel,
    pub rule_version: u32,
    pub source: String,
    pub confirmed: bool,
    pub increments: Vec<DiffusionIncrement>,
    pub gradients: Option<Vec<f64>>,
    pub vendor_coefficient_seconds: Option<f64>,
    pub program_sha256: Option<String>,
}

impl DiffusionInput {
    pub fn direct(values: &[f64], unit: &str) -> Result<Self, String> {
        let factor = match unit {
            "s/m²" => 1.0,
            "s/cm²" => 1e4,
            "s/mm²" => 1e6,
            _ => return Err("Choose s/m², s/cm² or s/mm² for b values.".into()),
        };
        let values: Vec<_> = values.iter().map(|b| b * factor).collect();
        validate_b_values(&values, values.len())?;
        Ok(Self::new(
            DiffusionModel::DirectB { unit: unit.into() },
            values,
            None,
            "User-provided b values",
        ))
    }

    fn new(
        model: DiffusionModel,
        values: Vec<f64>,
        gradients: Option<Vec<f64>>,
        source: &str,
    ) -> Self {
        Self {
            model,
            rule_version: 1,
            source: source.into(),
            confirmed: true,
            increments: values
                .into_iter()
                .enumerate()
                .map(|(id, b)| DiffusionIncrement {
                    id: IncrementId(id as u64),
                    b,
                })
                .collect(),
            gradients,
            vendor_coefficient_seconds: None,
            program_sha256: None,
        }
    }

    pub fn from_meta(values: &[f64], meta: &DiffusionMeta) -> Self {
        Self::new(
            DiffusionModel::StejskalTanner {
                gamma: meta.gamma,
                delta: meta.delta,
                big_delta: meta.big_delta,
                tau: meta.tau,
                shape_factor: meta.shape_factor,
            },
            values.iter().map(|g| meta.b_factor(*g)).collect(),
            Some(values.to_vec()),
            "Acquisition diffusion parameters",
        )
    }

    pub fn from_model(model: DiffusionModel, gradients: &[f64]) -> Result<Self, String> {
        let (gamma, delta, time) = match model {
            DiffusionModel::JeolBppDste {
                gamma,
                delta,
                big_delta,
                tau_1,
                tau_2,
            } => (
                gamma,
                delta,
                jeol_bpp_dste_time(delta, big_delta, tau_1, tau_2)?,
            ),
            DiffusionModel::RectangularPgse {
                gamma,
                delta,
                big_delta,
                tau,
            } => {
                if !big_delta.is_finite() || !tau.is_finite() || tau < 0.0 {
                    return Err("Confirm finite durations and a nonnegative interval.".into());
                }
                (gamma, delta, big_delta - delta / 3.0 - tau / 2.0)
            }
            _ => {
                return Err(
                    "Choose a supported rectangular sequence model or provide b values.".into(),
                );
            }
        };
        let values = rectangular_b_values(gamma, delta, time, gradients)?;
        Ok(Self::new(
            model,
            values,
            Some(gradients.to_vec()),
            "User-confirmed rectangular sequence parameters",
        ))
    }

    pub fn b_values(&self, count: usize) -> Result<Vec<f64>, String> {
        let report = self.inspect(count);
        if !report.executable {
            return Err(report
                .issues
                .iter()
                .map(|issue| issue.message.as_str())
                .collect::<Vec<_>>()
                .join(" "));
        }
        Ok(report.b_values.into_iter().map(|entry| entry.b).collect())
    }
}

pub(crate) fn automatic_input(data: &NmrSeriesSource) -> Result<DiffusionInput, String> {
    let axis = data
        .pseudo_axis
        .as_ref()
        .filter(|axis| axis.kind == PseudoKind::Gradient && axis.unit == "T/m")
        .ok_or_else(|| {
            "Confirm model parameters and a gradient axis, or provide b values.".to_owned()
        })?;
    if let Some(facts) = &data.jeol_diffusion {
        // JEOL sequences must pass their own adapter; never silently fall through
        // to the generic formula for an unknown or modified pulse program.
        let missing =
            || "Confirm the JEOL sequence model and parameters, or provide b values.".to_owned();
        let experiment = facts.text("experiment").ok_or_else(missing)?;
        if !matches!(
            experiment,
            "bpp_dste_led_dif" | "bpp_dste_led_diffusion" | "bpp_dste_led_diffusion.jxp"
        ) || facts.text("grad_shape") != Some("SQUARE")
        {
            return Err(missing());
        }
        let delta = facts.seconds("delta").ok_or_else(missing)?;
        let big_delta = facts.seconds("delta_large").ok_or_else(missing)?;
        let tau_1 = facts.seconds("tau_1").ok_or_else(missing)?;
        let tau_2 = facts.seconds("tau_2").ok_or_else(missing)?;
        let gamma = plotium_io::gyromagnetic_ratio(&data.direct.nucleus).ok_or_else(missing)?;
        let mut input = DiffusionInput::from_model(
            DiffusionModel::JeolBppDste {
                gamma,
                delta,
                big_delta,
                tau_1,
                tau_2,
            },
            &axis.values,
        )?;
        let c = jeol_bpp_dste_time(delta, big_delta, tau_1, tau_2)?;
        let close = |a: f64, b: f64| (a - b).abs() <= 5e-8 + b.abs() * 1e-6;
        let program = facts.embedded_program().unwrap_or("");
        let digest = format!("{:x}", Sha256::digest(program.as_bytes()));
        // The whitelist covers the complete pulse section, including gradient
        // polarity and timing, so a renamed or edited sequence needs confirmation.
        let known_program = digest == KNOWN_JEOL_BPP_DSTE_PROGRAM
            && facts.has_full_program_identity()
            && facts.gradient_axis == Some(0);
        input.confirmed = known_program
            && facts
                .seconds("diffusion_time")
                .is_some_and(|v| close(v, big_delta))
            && facts
                .seconds("grad_1")
                .is_some_and(|v| close(v, delta / 2.0))
            && facts
                .seconds("tau_e1")
                .is_some_and(|v| close(v, delta / 2.0 + tau_1))
            && facts
                .seconds("tau_e2")
                .is_some_and(|v| close(v, delta / 2.0 + tau_2))
            && facts.seconds("st_coef").is_some_and(|v| close(v, c));
        input.vendor_coefficient_seconds = facts.seconds("st_coef");
        input.program_sha256 = Some(digest);
        input.source = "JEOL JDF: delta, delta_large, tau_1, tau_2, grad_shape, g; st_coef is vendor-program calculated evidence".into();
        return Ok(input);
    }
    let meta = data
        .diffusion
        .as_ref()
        .ok_or_else(|| "Confirm model parameters or provide b values.".to_owned())?;
    let input = DiffusionInput::from_meta(&axis.values, meta);
    input.b_values(data.rows)?;
    Ok(input)
}

const KNOWN_JEOL_BPP_DSTE_PROGRAM: &str =
    "365f98cf0dad5fb8d5d63d9c4ded88a068fb12a36c296f9bb8243cb486d7a21d";

#[cfg(test)]
#[path = "diffusion_input_tests.rs"]
mod tests;
