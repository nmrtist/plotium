//! Conservative screening for a manual spherical Hertz fit, not a contact or modulus estimator.

use super::afm_hertz::{HertzInput, LinearBaseline};
use thiserror::Error;

const MIN_SAMPLES: usize = 32;
const MIN_BASELINE_SAMPLES: usize = 8;
const SUSTAINED_SAMPLES: usize = 5;
const MIN_FIT_SAMPLES: usize = 8;
const MAX_INDENTATION_RADIUS_RATIO: f64 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HertzSuggestion {
    pub input: HertzInput,
    /// RMS of the preliminary linear force baseline.
    pub baseline_rms_nn: f64,
    /// Baseline-corrected force at the start of the sustained loading run.
    pub onset_force_nn: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum SuggestionError {
    #[error(
        "Use a valid force calibration, positive spring constant, and positive spherical probe radius."
    )]
    Calibration,
    #[error("Use manual picks: the approach curve has too few finite samples.")]
    Samples,
    #[error("Use manual picks: relative Z is not a single regularly sampled ramp.")]
    Ramp,
    #[error("Use manual picks: the far-field baseline is unstable.")]
    Baseline,
    #[error("Use manual picks: no clear sustained loading onset was found.")]
    Onset,
    #[error("Use manual picks: an isolated excursion or possible adhesion obscures the onset.")]
    Excursion,
    #[error("Use manual picks: no sustained shallow compressive fit window was found.")]
    Window,
}

/// Propose one candidate from a verified loading branch with compression-positive force.
///
/// Each point is `[relative actuator Z in nm, signed force in nN]`; larger Z
/// is farther from the sample. Array order may be reversed for display.
/// `force_increment_nn` is the magnitude of one detector count in nN, or
/// zero for unquantized inputs.
/// This screening only supports well-sampled curves with a quiet pre-contact
/// tail. It does not fit a modulus or test whether Hertz is the right model.
pub fn suggest_hertz(
    points: &[[f64; 2]],
    spring_constant_n_per_m: f64,
    radius_nm: f64,
    force_increment_nn: f64,
) -> Result<HertzSuggestion, SuggestionError> {
    if !spring_constant_n_per_m.is_finite()
        || spring_constant_n_per_m <= 0.0
        || !radius_nm.is_finite()
        || radius_nm <= 0.0
        || !force_increment_nn.is_finite()
        || force_increment_nn < 0.0
    {
        return Err(SuggestionError::Calibration);
    }
    let ordered = ordered_ramp(points)?;
    // Fix the baseline support before looking for an onset. Fitting through a
    // detected onset could instead absorb early loading into the force baseline.
    let n_baseline = (ordered.len() / 5).max(MIN_BASELINE_SAMPLES);
    let baseline_z_nm = [ordered[n_baseline - 1][0], ordered[0][0]];
    let baseline = LinearBaseline::fit(&ordered[..n_baseline], baseline_z_nm)
        .map_err(|_| SuggestionError::Baseline)?;
    // Refuse a tail with a slope comparable to the calibrated cantilever
    // stiffness; a small linear detector drift is still permitted.
    if baseline.slope_nn_per_nm.abs() > 0.1 * spring_constant_n_per_m {
        return Err(SuggestionError::Baseline);
    }
    let corrected: Vec<_> = ordered
        .iter()
        .map(|&[z, force]| [z, force - baseline.at(z)])
        .collect();
    if corrected.iter().any(|point| !point[1].is_finite()) {
        return Err(SuggestionError::Baseline);
    }
    let noise = noise_scale(
        &ordered[..n_baseline],
        &corrected[..n_baseline],
        force_increment_nn,
    )?;
    let (contact, onset) = find_onset(&corrected, n_baseline, noise)?;
    let contact_z_nm = corrected[contact][0];
    let fit_z_nm = fit_window(
        &corrected[onset..],
        contact_z_nm,
        spring_constant_n_per_m,
        radius_nm,
        noise,
    )?;
    Ok(HertzSuggestion {
        input: HertzInput {
            spring_constant_n_per_m,
            radius_nm,
            baseline_z_nm,
            contact_z_nm,
            fit_z_nm,
        },
        baseline_rms_nn: baseline.rms_nn,
        onset_force_nn: corrected[onset][1],
    })
}

fn ordered_ramp(points: &[[f64; 2]]) -> Result<Vec<[f64; 2]>, SuggestionError> {
    if points.len() < MIN_SAMPLES || !points.iter().flatten().all(|value| value.is_finite()) {
        return Err(SuggestionError::Samples);
    }
    let reverse = points[0][0] < points[1][0];
    let (mut smallest, mut largest) = (f64::INFINITY, 0.0_f64);
    for pair in points.windows(2) {
        let step = if reverse {
            pair[1][0] - pair[0][0]
        } else {
            pair[0][0] - pair[1][0]
        };
        if !step.is_finite() || step <= 0.0 {
            return Err(SuggestionError::Ramp);
        }
        smallest = smallest.min(step);
        largest = largest.max(step);
    }
    // Don't interpret a discontinuity or a highly undersampled interval as an onset.
    if largest / smallest > 4.0 {
        return Err(SuggestionError::Ramp);
    }
    let mut ordered = points.to_vec();
    if reverse {
        ordered.reverse();
    }
    Ok(ordered)
}

fn noise_scale(
    raw: &[[f64; 2]],
    corrected: &[[f64; 2]],
    force_increment_nn: f64,
) -> Result<f64, SuggestionError> {
    let mut residuals: Vec<_> = corrected.iter().map(|point| point[1]).collect();
    let center = median(&mut residuals);
    for residual in &mut residuals {
        *residual = (*residual - center).abs();
    }
    let mad = median(&mut residuals);
    // The Gaussian scaling provides a useful screening scale without letting
    // one spike inflate an RMS estimate. A flat baseline can have zero MAD
    // even when the detector resolves only whole counts.
    let force_scale = raw.iter().map(|point| point[1].abs()).fold(0.0, f64::max);
    let noise = (1.4826 * mad)
        .max(force_increment_nn)
        .max(64.0 * f64::EPSILON * force_scale.max(f64::MIN_POSITIVE));
    if !noise.is_finite() || residuals.iter().any(|&value| value > 5.0 * noise) {
        return Err(SuggestionError::Baseline);
    }
    Ok(noise)
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_unstable_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len().is_multiple_of(2) {
        values[middle - 1].midpoint(values[middle])
    } else {
        values[middle]
    }
}

fn find_onset(
    points: &[[f64; 2]],
    n_baseline: usize,
    noise: f64,
) -> Result<(usize, usize), SuggestionError> {
    let threshold = 5.0 * noise;
    let tail = &points[n_baseline..];
    let onset = tail
        .iter()
        .position(|point| point[1] > threshold)
        .ok_or(SuggestionError::Onset)?;
    if tail[..onset].iter().any(|point| point[1] < -threshold)
        || tail
            .get(onset..onset + SUSTAINED_SAMPLES)
            .is_none_or(|run| run.iter().any(|point| point[1] <= threshold))
    {
        return Err(SuggestionError::Excursion);
    }
    let onset = n_baseline + onset;
    let contact = (n_baseline..onset)
        .rfind(|&index| points[index][1].abs() <= 1.5 * noise)
        .ok_or(SuggestionError::Onset)?;
    // A long transition between the last quiet sample and the sustained rise
    // makes the contact too ambiguous to propose.
    if onset - contact > MIN_FIT_SAMPLES {
        return Err(SuggestionError::Onset);
    }
    Ok((contact, onset))
}

fn fit_window(
    points: &[[f64; 2]],
    contact_z_nm: f64,
    k: f64,
    radius_nm: f64,
    noise: f64,
) -> Result<[f64; 2], SuggestionError> {
    let mut previous_force = 0.0_f64;
    let mut previous_indentation = 0.0_f64;
    let mut end = 0;
    for &[z, force] in points {
        let indentation = contact_z_nm - z - force / k;
        if !indentation.is_finite()
            || indentation <= previous_indentation
            || indentation > MAX_INDENTATION_RADIUS_RATIO * radius_nm
            || force <= 0.0
            || force < previous_force - 2.0 * noise
        {
            break;
        }
        previous_force = force;
        previous_indentation = indentation;
        end += 1;
    }
    if end < MIN_FIT_SAMPLES || previous_force - points[0][1] < 5.0 * noise {
        return Err(SuggestionError::Window);
    }
    Ok([points[end - 1][0], points[0][0]])
}

#[cfg(test)]
#[path = "afm_hertz_suggestion_tests.rs"]
mod tests;
