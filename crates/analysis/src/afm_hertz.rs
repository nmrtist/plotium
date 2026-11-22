//! Manual, nonadhesive spherical Hertz fit for a calibrated AFM approach curve.

use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HertzInput {
    pub spring_constant_n_per_m: f64,
    pub radius_nm: f64,
    /// Inclusive endpoints in increasing relative Z, on the noncontact side.
    pub baseline_z_nm: [f64; 2],
    pub contact_z_nm: f64,
    /// Inclusive endpoints in increasing relative Z, on the compression side.
    pub fit_z_nm: [f64; 2],
}

#[derive(Clone, Debug, PartialEq)]
pub struct HertzResult {
    pub reduced_modulus_mpa: f64,
    /// Only the selected fit window, in input order.
    pub rows: Vec<HertzRow>,
    pub baseline_rms_nn: f64,
    pub baseline_points: usize,
    pub baseline_slope_nn_per_nm: f64,
    pub baseline_force_at_contact_nn: f64,
    pub residual_rms_nn: f64,
    pub max_indentation_over_radius: f64,
    pub r_squared: Option<f64>,
    pub contact_sensitivity: Option<HertzContactSensitivity>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HertzRow {
    pub z_nm: f64,
    pub indentation_nm: f64,
    pub corrected_force_nn: f64,
    pub predicted_force_nn: f64,
}

impl HertzRow {
    pub fn residual_nn(&self) -> f64 {
        self.corrected_force_nn - self.predicted_force_nn
    }
}

/// A contact-location perturbation, not a confidence interval.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HertzContactSensitivity {
    pub shift_nm: f64,
    pub lower_contact_modulus_mpa: Option<f64>,
    pub higher_contact_modulus_mpa: Option<f64>,
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum HertzError {
    #[error("A finite, positive spring constant is required.")]
    SpringConstant,
    #[error("Enter a finite, positive spherical tip radius.")]
    Radius,
    #[error("Enter a finite contact Z and distinct, ascending interval bounds.")]
    Intervals,
    #[error("The baseline must be above contact in relative Z and the fit range must be below it.")]
    IntervalOrder,
    #[error("The approach curve must contain finite relative Z and force samples.")]
    Samples,
    #[error("Both intervals must stay within the recorded approach curve.")]
    OutsideCurve,
    #[error("The baseline interval needs at least three samples and distinct Z values.")]
    BaselineSamples,
    #[error("The fit interval needs at least three samples and distinct Z values.")]
    FitSamples,
    #[error(
        "The fit interval contains nonpositive baseline-corrected force. Check the baseline and fit range."
    )]
    NonCompressiveForce,
    #[error(
        "The fit interval contains nonpositive indentation. Check the contact Z and fit range."
    )]
    NonpositiveIndentation,
    #[error("The fit interval needs distinct indentation values.")]
    IndentationSpan,
    #[error("The Hertz calculation did not produce finite, positive fit parameters.")]
    Numerical,
}

/// Fit force as a function of indentation, with zero force at zero indentation.
///
/// Input points are `[relative Z in nm, signed force in nN]`. Relative Z must
/// decrease toward the sample, and compression force must be positive after
/// subtracting a linear force baseline estimated from the noncontact window.
pub fn fit_hertz(points: &[[f64; 2]], input: HertzInput) -> Result<HertzResult, HertzError> {
    validate(points, input)?;
    let baseline = LinearBaseline::fit(points, input.baseline_z_nm)?;
    let baseline_force_at_contact_nn = baseline.at(input.contact_z_nm);
    if !baseline_force_at_contact_nn.is_finite() {
        return Err(HertzError::Numerical);
    }
    let selected: Vec<_> = points
        .iter()
        .filter(|[z, _]| contains(input.fit_z_nm, *z))
        .map(|&[z, force]| [z, force - baseline.at(z)])
        .collect();
    if selected.len() < 3 || selected.iter().all(|p| p[0] == selected[0][0]) {
        return Err(HertzError::FitSamples);
    }

    let (rows, reduced_modulus_mpa) = fit_at_contact(&selected, input, input.contact_z_nm)?;
    let n = rows.len() as f64;
    let mean_force = rows.iter().map(|row| row.corrected_force_nn).sum::<f64>() / n;
    let centered_sum = rows
        .iter()
        .map(|row| (row.corrected_force_nn - mean_force).powi(2))
        .sum::<f64>();
    let residual_sum = rows
        .iter()
        .map(|row| row.residual_nn().powi(2))
        .sum::<f64>();
    let residual_rms_nn = (residual_sum / n).sqrt();
    let max_indentation_over_radius = rows
        .iter()
        .map(|row| row.indentation_nm)
        .fold(0.0, f64::max)
        / input.radius_nm;
    if !residual_rms_nn.is_finite() || !max_indentation_over_radius.is_finite() {
        return Err(HertzError::Numerical);
    }
    let r_squared = (centered_sum.is_finite() && centered_sum > 0.0)
        .then(|| 1.0 - residual_sum / centered_sum)
        .filter(|value| value.is_finite());
    let contact_sensitivity = contact_spacing(points, input.contact_z_nm).map(|shift_nm| {
        let modulus_at = |contact: f64| {
            if contact <= input.fit_z_nm[1] || contact >= input.baseline_z_nm[0] {
                return None;
            }
            fit_at_contact(&selected, input, contact)
                .ok()
                .map(|(_, modulus)| modulus)
        };
        HertzContactSensitivity {
            shift_nm,
            lower_contact_modulus_mpa: modulus_at(input.contact_z_nm - shift_nm),
            higher_contact_modulus_mpa: modulus_at(input.contact_z_nm + shift_nm),
        }
    });
    Ok(HertzResult {
        reduced_modulus_mpa,
        rows,
        baseline_rms_nn: baseline.rms_nn,
        baseline_points: baseline.samples,
        baseline_slope_nn_per_nm: baseline.slope_nn_per_nm,
        baseline_force_at_contact_nn,
        residual_rms_nn,
        max_indentation_over_radius,
        r_squared,
        contact_sensitivity,
    })
}

fn validate(points: &[[f64; 2]], input: HertzInput) -> Result<(), HertzError> {
    if !positive_finite(input.spring_constant_n_per_m) {
        return Err(HertzError::SpringConstant);
    }
    if !positive_finite(input.radius_nm) {
        return Err(HertzError::Radius);
    }
    let [lo_baseline, hi_baseline] = input.baseline_z_nm;
    let [lo_fit, hi_fit] = input.fit_z_nm;
    let contact = input.contact_z_nm;
    if [lo_baseline, hi_baseline, lo_fit, hi_fit, contact]
        .into_iter()
        .any(|value| !value.is_finite())
        || lo_baseline >= hi_baseline
        || lo_fit >= hi_fit
    {
        return Err(HertzError::Intervals);
    }
    if hi_fit >= contact || lo_baseline <= contact {
        return Err(HertzError::IntervalOrder);
    }
    if points.is_empty() || points.iter().flatten().any(|value| !value.is_finite()) {
        return Err(HertzError::Samples);
    }
    let (lo, hi) = points
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &[z, _]| {
            (lo.min(z), hi.max(z))
        });
    if lo_fit < lo || hi_baseline > hi {
        return Err(HertzError::OutsideCurve);
    }
    Ok(())
}

pub(super) struct LinearBaseline {
    center_z_nm: f64,
    center_force_nn: f64,
    pub(super) slope_nn_per_nm: f64,
    pub(super) rms_nn: f64,
    samples: usize,
}

impl LinearBaseline {
    pub(super) fn at(&self, z: f64) -> f64 {
        self.center_force_nn + self.slope_nn_per_nm * (z - self.center_z_nm)
    }

    pub(super) fn fit(points: &[[f64; 2]], interval: [f64; 2]) -> Result<Self, HertzError> {
        let selected = points.iter().filter(|[z, _]| contains(interval, *z));
        let (z_sum, force_sum, samples) = selected
            .clone()
            .fold((0.0, 0.0, 0), |(zs, fs, n), &[z, f]| {
                (zs + z, fs + f, n + 1)
            });
        if samples < 3 {
            return Err(HertzError::BaselineSamples);
        }
        // Test the sampled coordinates directly: rounding their mean can make
        // identical Z values appear to have nonzero variance.
        let mut z_values = selected.clone().map(|&[z, _]| z);
        if z_values
            .next()
            .is_none_or(|first| z_values.all(|z| z == first))
        {
            return Err(HertzError::BaselineSamples);
        }
        let (center_z_nm, center_force_nn) = (z_sum / samples as f64, force_sum / samples as f64);
        let (zz, zf) = selected.clone().fold((0.0, 0.0), |(zz, zf), &[z, f]| {
            let dz = z - center_z_nm;
            (zz + dz * dz, zf + dz * (f - center_force_nn))
        });
        if zz == 0.0 {
            return Err(HertzError::BaselineSamples);
        }
        if !zz.is_finite() || !zf.is_finite() {
            return Err(HertzError::Numerical);
        }
        let mut baseline = Self {
            center_z_nm,
            center_force_nn,
            slope_nn_per_nm: zf / zz,
            rms_nn: 0.0,
            samples,
        };
        baseline.rms_nn = (selected
            .map(|&[z, f]| (f - baseline.at(z)).powi(2))
            .sum::<f64>()
            / samples as f64)
            .sqrt();
        if !baseline.rms_nn.is_finite() {
            return Err(HertzError::Numerical);
        }
        Ok(baseline)
    }
}

fn fit_at_contact(
    points: &[[f64; 2]],
    input: HertzInput,
    contact_z_nm: f64,
) -> Result<(Vec<HertzRow>, f64), HertzError> {
    let mut rows = Vec::with_capacity(points.len());
    let (mut xy, mut xx) = (0.0, 0.0);
    for &[z_nm, corrected_force_nn] in points {
        // 1 nN / (1 N/m) = 1 nm of cantilever deflection.
        let indentation_nm =
            (contact_z_nm - z_nm) - corrected_force_nn / input.spring_constant_n_per_m;
        if !corrected_force_nn.is_finite() || !indentation_nm.is_finite() {
            return Err(HertzError::Numerical);
        }
        if corrected_force_nn <= 0.0 {
            return Err(HertzError::NonCompressiveForce);
        }
        if indentation_nm <= 0.0 {
            return Err(HertzError::NonpositiveIndentation);
        }
        let feature = indentation_nm * indentation_nm.sqrt();
        xy += feature * corrected_force_nn;
        xx += feature * feature;
        rows.push(HertzRow {
            z_nm,
            indentation_nm,
            corrected_force_nn,
            predicted_force_nn: feature,
        });
    }
    if !rows.split_first().is_some_and(|(first, remaining)| {
        remaining
            .iter()
            .any(|row| row.indentation_nm != first.indentation_nm)
    }) {
        return Err(HertzError::IndentationSpan);
    }
    let coefficient = xy / xx;
    // F = (4/3) E* sqrt(R) delta^(3/2); 1 nN/nm² = 1000 MPa.
    let modulus_mpa = 750.0 * coefficient / input.radius_nm.sqrt();
    if !positive_finite(modulus_mpa) {
        return Err(HertzError::Numerical);
    }
    for row in &mut rows {
        row.predicted_force_nn *= coefficient;
        if !row.residual_nn().is_finite() {
            return Err(HertzError::Numerical);
        }
    }
    Ok((rows, modulus_mpa))
}

fn contact_spacing(points: &[[f64; 2]], contact: f64) -> Option<f64> {
    let (mut below, mut above) = (f64::INFINITY, f64::INFINITY);
    let mut exact = false;
    for &[z, _] in points {
        if z < contact {
            below = below.min(contact - z);
        } else if z > contact {
            above = above.min(z - contact);
        } else {
            exact = true;
        }
    }
    let spacing = if exact {
        below.min(above)
    } else {
        below + above
    };
    positive_finite(spacing).then_some(spacing)
}

fn contains([low, high]: [f64; 2], value: f64) -> bool {
    (low..=high).contains(&value)
}

fn positive_finite(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

#[cfg(test)]
#[path = "afm_hertz_tests.rs"]
mod tests;
