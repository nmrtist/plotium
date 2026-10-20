//! Structured validation of the immutable diffusion encoding value.
use crate::{DiffusionIncrement, DiffusionInput, DiffusionModel, IncrementId};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffusionReadiness {
    Executable,
    MissingParameters,
    EvidenceRequired,
    InvalidInput,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiffusionIssue {
    pub code: String,
    pub path: String,
    pub expected_unit: Option<String>,
    pub actual: Value,
    pub message: String,
    pub evidence: String,
    pub remedy: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiffusionInputReport {
    pub status: DiffusionReadiness,
    pub executable: bool,
    pub b_unit: String,
    pub diffusion_unit: String,
    pub input_unit: Option<String>,
    pub conversion_factor: Option<f64>,
    pub source: String,
    pub rule_version: u32,
    pub program_sha256: Option<String>,
    pub vendor_coefficient_seconds: Option<f64>,
    /// Sorted by acquired IncrementId, with all values in s/m².
    pub b_values: Vec<DiffusionIncrement>,
    pub issues: Vec<DiffusionIssue>,
}

impl DiffusionInput {
    pub fn inspect(&self, count: usize) -> DiffusionInputReport {
        let mut issues = Vec::new();
        let mut issue = |code: &str,
                         path: String,
                         unit: Option<&str>,
                         actual: Value,
                         message: &str,
                         remedy: &str| {
            issues.push(DiffusionIssue {
                code: code.into(),
                path,
                expected_unit: unit.map(str::to_owned),
                actual,
                message: message.into(),
                evidence: self.source.clone(),
                remedy: remedy.into(),
            });
        };
        if !self.confirmed {
            issue(
                "evidence_required",
                "confirmed".into(),
                None,
                json!(false),
                "Confirm the sequence evidence and parameters or provide b values.",
                "Review the pulse program identity, gradient shape, timing and vendor coefficient; explicitly confirm the model or provide direct b values.",
            );
        }
        if self.rule_version != 1 {
            issue(
                "invalid_rule_version",
                "rule_version".into(),
                None,
                json!(self.rule_version),
                "The diffusion encoding rule version must be 1.",
                "Use the v1 diffusion encoding rule.",
            );
        }
        let (input_unit, conversion_factor) = match &self.model {
            DiffusionModel::DirectB { unit } => {
                let factor = match unit.as_str() {
                    "s/m²" => Some(1.),
                    "s/cm²" => Some(1e4),
                    "s/mm²" => Some(1e6),
                    _ => None,
                };
                if factor.is_none() {
                    issue(
                        "invalid_unit",
                        "model.unit".into(),
                        Some("s/m²"),
                        json!(unit),
                        "Unsupported b-value input unit.",
                        "Choose s/m², s/cm² or s/mm²; stored increments must already be converted to s/m².",
                    );
                }
                (Some(unit.clone()), factor)
            }
            _ => (Some("T/m".into()), None),
        };
        if count < 3 {
            issue(
                "insufficient_increments",
                "increments".into(),
                None,
                json!(count),
                "At least three acquired increments are required.",
                "Select an acquisition with at least three increments.",
            );
        }
        if self.increments.len() != count {
            issue(
                "increment_count_mismatch",
                "increments".into(),
                None,
                json!(self.increments.len()),
                "Provide one b value per acquired increment.",
                "Bind one b value to every acquired IncrementId.",
            );
        }
        let mut ids = BTreeSet::new();
        for (position, entry) in self.increments.iter().enumerate() {
            if !ids.insert(entry.id) {
                issue(
                    "duplicate_increment",
                    format!("increments[{position}].id"),
                    None,
                    json!(entry.id),
                    "An acquired increment occurs more than once.",
                    "Remove the duplicate IncrementId binding.",
                );
            }
            if entry.id.0 >= count as u64 {
                issue(
                    "unknown_increment",
                    format!("increments[{position}].id"),
                    None,
                    json!(entry.id),
                    "The increment does not belong to this acquisition.",
                    "Use acquired IncrementIds in the range 0..count.",
                );
            }
            if !entry.b.is_finite() || entry.b < 0. {
                issue(
                    "invalid_b_value",
                    format!("increments[{position}].b"),
                    Some("s/m²"),
                    json!(entry.b),
                    "Provide finite, nonnegative b values in s/m².",
                    "Replace the invalid b value after converting its input unit.",
                );
            }
        }
        for id in 0..count as u64 {
            if !ids.contains(&IncrementId(id)) {
                issue(
                    "missing_increment",
                    "increments".into(),
                    Some("s/m²"),
                    json!(id),
                    "An acquired increment has no b value.",
                    "Add the missing IncrementId binding.",
                );
            }
        }
        let mut unique: Vec<_> = self
            .increments
            .iter()
            .map(|entry| entry.b)
            .filter(|b| b.is_finite() && *b >= 0.)
            .collect();
        unique.sort_by(f64::total_cmp);
        unique.dedup();
        if unique.len() < 3 {
            issue(
                "insufficient_distinct_b_values",
                "increments".into(),
                Some("s/m²"),
                json!(unique.len()),
                "Provide at least three distinct b values.",
                "Check the gradient list and encoding parameters.",
            );
        }
        if let Some(gradients) = &self.gradients {
            if gradients.len() != count {
                issue(
                    "gradient_count_mismatch",
                    "gradients".into(),
                    Some("T/m"),
                    json!(gradients.len()),
                    "Provide a gradient for every increment.",
                    "Align the gradient list with the acquired IncrementIds.",
                );
            }
            for (index, gradient) in gradients.iter().enumerate() {
                if !gradient.is_finite() {
                    issue(
                        "invalid_gradient",
                        format!("gradients[{index}]"),
                        Some("T/m"),
                        json!(gradient),
                        "Gradient values must be finite.",
                        "Replace the invalid gradient value.",
                    );
                }
            }
        }
        // Preserve the scientific model's validation, including the effective
        // encoding time, rather than guessing sequence semantics from a label.
        let model_check = match &self.model {
            DiffusionModel::RectangularPgse { .. } | DiffusionModel::JeolBppDste { .. } => Some(
                Self::from_model(
                    self.model.clone(),
                    self.gradients.as_deref().unwrap_or(&[0., 1., 2.]),
                )
                .map(|_| ()),
            ),
            DiffusionModel::StejskalTanner {
                gamma,
                delta,
                big_delta,
                tau,
                shape_factor,
            } => {
                let time = big_delta - shape_factor * delta - tau / 2.;
                Some(
                    if gamma.is_finite()
                        && *gamma != 0.
                        && delta.is_finite()
                        && *delta > 0.
                        && big_delta.is_finite()
                        && tau.is_finite()
                        && *tau >= 0.
                        && shape_factor.is_finite()
                        && *shape_factor > 0.
                        && time > 0.
                    {
                        Ok(())
                    } else {
                        Err("Confirm finite gradient values and valid encoding parameters.".into())
                    },
                )
            }
            DiffusionModel::DirectB { .. } => None,
        };
        if let Some(Err(message)) = model_check {
            issue(
                "invalid_model_parameters",
                "model".into(),
                None,
                json!(self.model),
                &message,
                "Correct the model parameters or provide direct b values.",
            );
        }
        if let Some(gradients) = self
            .gradients
            .as_ref()
            .filter(|g| g.len() == count && g.iter().all(|v| v.is_finite()))
        {
            let expected = match &self.model {
                DiffusionModel::StejskalTanner {
                    gamma,
                    delta,
                    big_delta,
                    tau,
                    shape_factor,
                } => Some(
                    gradients
                        .iter()
                        .map(|g| {
                            (gamma * delta * g).powi(2)
                                * (big_delta - shape_factor * delta - tau / 2.)
                        })
                        .collect::<Vec<_>>(),
                ),
                DiffusionModel::RectangularPgse { .. } | DiffusionModel::JeolBppDste { .. } => {
                    match Self::from_model(self.model.clone(), gradients) {
                        Ok(input) => Some(input.increments.into_iter().map(|v| v.b).collect()),
                        // The model_check above already records this validation
                        // failure; skip the dependent consistency comparison.
                        Err(_) => None,
                    }
                }
                _ => None,
            };
            if let Some(expected) = expected {
                for entry in &self.increments {
                    if let Some(value) = usize::try_from(entry.id.0)
                        .ok()
                        .and_then(|id| expected.get(id))
                        && entry.b.is_finite()
                        && value.is_finite()
                        && (entry.b - value).abs() > 1e-6 + value.abs() * 1e-10
                    {
                        issue(
                            "conflicting_b_value",
                            format!("increments[{}].b", entry.id.0),
                            Some("s/m²"),
                            json!(entry.b),
                            "The b value conflicts with the retained gradient and model parameters.",
                            "Recompute b values from the model or use a direct-b input without conflicting model evidence.",
                        );
                    }
                }
            }
        }
        if let (
            Some(actual),
            DiffusionModel::JeolBppDste {
                delta,
                big_delta,
                tau_1,
                tau_2,
                ..
            },
        ) = (self.vendor_coefficient_seconds, &self.model)
            && let Ok(expected) = plotium_analysis::diffusion_encoding::jeol_bpp_dste_time(
                *delta, *big_delta, *tau_1, *tau_2,
            )
            && (!actual.is_finite() || (actual - expected).abs() > 5e-8 + expected.abs() * 1e-6)
        {
            issue(
                "conflicting_vendor_coefficient",
                "vendor_coefficient_seconds".into(),
                Some("s"),
                json!(actual),
                "The vendor coefficient conflicts with the confirmed timing model.",
                "Review pulse-program timing and the vendor coefficient evidence.",
            );
        }
        let mut b_values = self.increments.clone();
        b_values.sort_by_key(|entry| entry.id);
        let status = if issues.iter().any(|issue| issue.code != "evidence_required") {
            DiffusionReadiness::InvalidInput
        } else if !self.confirmed {
            DiffusionReadiness::EvidenceRequired
        } else {
            DiffusionReadiness::Executable
        };
        DiffusionInputReport {
            executable: status == DiffusionReadiness::Executable,
            status,
            b_unit: "s/m²".into(),
            diffusion_unit: "m²/s".into(),
            input_unit,
            conversion_factor,
            source: self.source.clone(),
            rule_version: self.rule_version,
            program_sha256: self.program_sha256.clone(),
            vendor_coefficient_seconds: self.vendor_coefficient_seconds,
            b_values,
            issues,
        }
    }
}

#[cfg(test)]
#[path = "diffusion_report_tests.rs"]
mod tests;
