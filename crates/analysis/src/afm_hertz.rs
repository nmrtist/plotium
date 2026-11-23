//! Nonadhesive spherical Hertz fits for calibrated AFM approach curves.

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HertzMethod {
    ManualContact(f64),
    Linearized,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HertzWindow {
    RelativeZ([f64; 2]),
    ForceNn([f64; 2]),
    ForcePercent([f64; 2]),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HertzReference {
    Zero,
    Automatic,
    Manual(f64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HertzOptions {
    pub spring_constant_n_per_m: f64,
    pub radius_nm: f64,
    pub baseline_z_nm: [f64; 2],
    pub method: HertzMethod,
    pub window: HertzWindow,
    pub reference: HertzReference,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HertzSelection {
    pub corrected: Vec<[f64; 2]>,
    /// Indices in the supplied approach curve.
    pub indices: Vec<usize>,
    pub reference_nn: f64,
    pub force_bounds_nn: Option<[f64; 2]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HertzResult {
    pub reduced_modulus_mpa: f64,
    pub options: HertzOptions,
    pub reference_nn: f64,
    pub force_bounds_nn: Option<[f64; 2]>,
    pub extrapolated_zero_z_nm: Option<f64>,
    pub linearized_slope: Option<f64>,
    pub sample_indices: Vec<usize>,
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
    #[error("Enter finite, ascending window bounds. Force percentages must be between 0 and 100.")]
    Window,
    #[error("Enter a finite reference force in nN.")]
    Reference,
    #[error("No positive-slope turn was found. Enter the reference force manually.")]
    ReferenceTurn,
    #[error("The peak corrected force must be above the reference force for a percentage window.")]
    ForceSpan,
    #[error("The linearized Hertz slope must be positive.")]
    LinearizedSlope,
    #[error("The Hertz calculation did not produce finite, positive fit parameters.")]
    Numerical,
}

/// Fit force as a function of indentation, with zero force at zero indentation.
///
/// Input points are `[relative Z in nm, signed force in nN]`. Relative Z must
/// decrease toward the sample, and compression force must be positive after
/// subtracting a linear force baseline estimated from the noncontact window.
pub fn fit_hertz(points: &[[f64; 2]], input: HertzInput) -> Result<HertzResult, HertzError> {
    fit_hertz_with_options(
        points,
        HertzOptions {
            spring_constant_n_per_m: input.spring_constant_n_per_m,
            radius_nm: input.radius_nm,
            baseline_z_nm: input.baseline_z_nm,
            method: HertzMethod::ManualContact(input.contact_z_nm),
            window: HertzWindow::RelativeZ(input.fit_z_nm),
            reference: HertzReference::Zero,
        },
    )
}

pub fn select_hertz_points(
    points: &[[f64; 2]],
    options: HertzOptions,
) -> Result<HertzSelection, HertzError> {
    prepare(points, options).map(|(_, selection)| selection)
}

pub fn fit_hertz_with_options(
    points: &[[f64; 2]],
    options: HertzOptions,
) -> Result<HertzResult, HertzError> {
    let (baseline, selection) = prepare(points, options)?;
    let selected: Vec<_> = selection
        .indices
        .iter()
        .map(|&i| selection.corrected[i])
        .collect();
    if selected.len() < 3 {
        return Err(HertzError::FitSamples);
    }
    let mut extrapolated_zero_z_nm = None;
    let mut linearized_slope = None;
    let mut contact_sensitivity = None;
    let (rows, reduced_modulus_mpa, zero_z) = match options.method {
        HertzMethod::ManualContact(contact) => {
            if selected.iter().all(|p| p[0] == selected[0][0]) {
                return Err(HertzError::FitSamples);
            }
            let (rows, modulus) =
                fit_at_contact(&selected, options, contact, selection.reference_nn)?;
            contact_sensitivity = contact_spacing(points, contact).map(|shift_nm| {
                let modulus_at = |shifted| {
                    if selected.iter().any(|p| p[0] >= shifted)
                        || shifted >= options.baseline_z_nm[0]
                    {
                        return None;
                    }
                    if let HertzWindow::RelativeZ([_, high]) = options.window
                        && shifted <= high
                    {
                        return None;
                    }
                    fit_at_contact(&selected, options, shifted, selection.reference_nn)
                        .ok()
                        .map(|(_, modulus)| modulus)
                };
                HertzContactSensitivity {
                    shift_nm,
                    lower_contact_modulus_mpa: modulus_at(contact - shift_nm),
                    higher_contact_modulus_mpa: modulus_at(contact + shift_nm),
                }
            });
            (rows, modulus, contact)
        }
        HertzMethod::Linearized => {
            let (rows, modulus, zero, slope) =
                fit_linearized(&selected, options, selection.reference_nn)?;
            extrapolated_zero_z_nm = Some(zero);
            linearized_slope = Some(slope);
            (rows, modulus, zero)
        }
    };
    let baseline_force_at_contact_nn = baseline.at(zero_z);
    if !baseline_force_at_contact_nn.is_finite() {
        return Err(HertzError::Numerical);
    }
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
        / options.radius_nm;
    if !residual_rms_nn.is_finite() || !max_indentation_over_radius.is_finite() {
        return Err(HertzError::Numerical);
    }
    let r_squared = (centered_sum.is_finite() && centered_sum > 0.0)
        .then(|| 1.0 - residual_sum / centered_sum)
        .filter(|value| value.is_finite());
    Ok(HertzResult {
        options,
        reference_nn: selection.reference_nn,
        force_bounds_nn: selection.force_bounds_nn,
        extrapolated_zero_z_nm,
        linearized_slope,
        sample_indices: selection.indices,
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

fn prepare(
    points: &[[f64; 2]],
    options: HertzOptions,
) -> Result<(LinearBaseline, HertzSelection), HertzError> {
    if !positive_finite(options.spring_constant_n_per_m) {
        return Err(HertzError::SpringConstant);
    }
    if !positive_finite(options.radius_nm) {
        return Err(HertzError::Radius);
    }
    let [baseline_low, baseline_high] = options.baseline_z_nm;
    if !valid_bounds(options.baseline_z_nm) {
        return Err(HertzError::Intervals);
    }
    let z_bounds = match options.window {
        HertzWindow::RelativeZ(bounds) => {
            if !valid_bounds(bounds) {
                return Err(HertzError::Intervals);
            }
            Some(bounds)
        }
        HertzWindow::ForceNn(bounds) | HertzWindow::ForcePercent(bounds) => {
            if !valid_bounds(bounds)
                || (matches!(options.window, HertzWindow::ForcePercent(_))
                    && (bounds[0] < 0.0 || bounds[1] > 100.0))
            {
                return Err(HertzError::Window);
            }
            None
        }
    };
    if let HertzMethod::ManualContact(contact) = options.method {
        if !contact.is_finite() {
            return Err(HertzError::Intervals);
        }
        if baseline_low <= contact || z_bounds.is_some_and(|[_, high]| high >= contact) {
            return Err(HertzError::IntervalOrder);
        }
    }
    if points.is_empty() || points.iter().flatten().any(|value| !value.is_finite()) {
        return Err(HertzError::Samples);
    }
    let (lo, hi) = points
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &[z, _]| {
            (lo.min(z), hi.max(z))
        });
    if baseline_low < lo
        || baseline_high > hi
        || z_bounds.is_some_and(|[low, high]| low < lo || high > hi)
    {
        return Err(HertzError::OutsideCurve);
    }
    let baseline = LinearBaseline::fit(points, options.baseline_z_nm)?;
    let corrected: Vec<_> = points
        .iter()
        .map(|&[z, f]| [z, f - baseline.at(z)])
        .collect();
    if corrected.iter().any(|p| !p[1].is_finite()) {
        return Err(HertzError::Numerical);
    }
    let reference_nn = match options.reference {
        HertzReference::Zero => 0.0,
        HertzReference::Manual(force) if force.is_finite() => force,
        HertzReference::Manual(_) => return Err(HertzError::Reference),
        HertzReference::Automatic => {
            automatic_reference(&corrected, options.spring_constant_n_per_m)?
        }
    };
    let force_bounds_nn = match options.window {
        HertzWindow::RelativeZ(_) => None,
        HertzWindow::ForceNn(bounds) => Some(bounds),
        HertzWindow::ForcePercent(bounds) => {
            let peak = corrected
                .iter()
                .map(|p| p[1])
                .fold(f64::NEG_INFINITY, f64::max);
            let span = peak - reference_nn;
            if !positive_finite(span) {
                return Err(HertzError::ForceSpan);
            }
            Some(bounds.map(|p| match p {
                0.0 => reference_nn,
                100.0 => peak,
                _ => reference_nn + (p / 100.0) * span,
            }))
        }
    };
    let indices = corrected
        .iter()
        .enumerate()
        .filter_map(|(i, &[z, f])| {
            let in_window = z_bounds.is_some_and(|bounds| contains(bounds, z))
                || force_bounds_nn.is_some_and(|bounds| contains(bounds, f));
            let valid = match options.method {
                HertzMethod::Linearized => f >= reference_nn,
                HertzMethod::ManualContact(contact) => {
                    z_bounds.is_some()
                        || (f > reference_nn
                            && indentation(
                                contact,
                                z,
                                f,
                                options.spring_constant_n_per_m,
                                reference_nn,
                            ) > 0.0)
                }
            };
            (in_window && valid).then_some(i)
        })
        .collect();
    Ok((
        baseline,
        HertzSelection {
            corrected,
            indices,
            reference_nn,
            force_bounds_nn,
        },
    ))
}

fn valid_bounds([low, high]: [f64; 2]) -> bool {
    low.is_finite() && high.is_finite() && low < high
}

fn automatic_reference(points: &[[f64; 2]], spring: f64) -> Result<f64, HertzError> {
    let mut separation: Vec<_> = points.iter().map(|&[z, f]| [z + f / spring, f]).collect();
    if separation.iter().any(|p| !p[0].is_finite()) {
        return Err(HertzError::Numerical);
    }
    // The center sample locates each local slope estimate on the curve.
    separation.sort_by(|a, b| a[0].total_cmp(&b[0]));
    let width = separation.len().div_ceil(20).max(3);
    separation
        .windows(width)
        .find(|window| regression(window).is_ok_and(|(_, _, slope)| slope > 0.0))
        .map(|window| window[width / 2][1])
        .ok_or(HertzError::ReferenceTurn)
}

fn regression(points: &[[f64; 2]]) -> Result<(f64, f64, f64), HertzError> {
    if points.iter().all(|p| p[0] == points[0][0]) {
        return Err(HertzError::IndentationSpan);
    }
    let n = points.len() as f64;
    let (x, y) = points
        .iter()
        .fold((0.0, 0.0), |(x, y), p| (x + p[0], y + p[1]));
    let (x, y) = (x / n, y / n);
    let (xx, xy) = points.iter().fold((0.0, 0.0), |(xx, xy), p| {
        let dx = p[0] - x;
        (xx + dx * dx, xy + dx * (p[1] - y))
    });
    let slope = xy / xx;
    if !slope.is_finite() {
        return Err(HertzError::Numerical);
    }
    Ok((x, y, slope))
}

fn fit_linearized(
    points: &[[f64; 2]],
    options: HertzOptions,
    reference: f64,
) -> Result<(Vec<HertzRow>, f64, f64, f64), HertzError> {
    let spring = options.spring_constant_n_per_m;
    // Use a local Z origin to retain precision when the whole curve is translated.
    let origin = points[0][0];
    let linearized: Vec<_> = points
        .iter()
        .map(|&[z, f]| {
            [
                (origin - z) - (f - reference) / spring,
                (f - reference).cbrt().powi(2),
            ]
        })
        .collect();
    let (x, y, slope) = regression(&linearized)?;
    if slope <= 0.0 {
        return Err(HertzError::LinearizedSlope);
    }
    let zero = x - y / slope;
    let modulus = 750.0 * slope.powf(1.5) / options.radius_nm.sqrt();
    if !positive_finite(modulus) || !zero.is_finite() {
        return Err(HertzError::Numerical);
    }
    let rows = points
        .iter()
        .zip(linearized)
        .map(|(&[z, f], [x, _])| {
            let indentation_nm = x - zero;
            HertzRow {
                z_nm: z,
                indentation_nm,
                corrected_force_nn: f,
                predicted_force_nn: reference + (slope * indentation_nm).max(0.0).powf(1.5),
            }
        })
        .collect();
    Ok((rows, modulus, origin - zero, slope))
}

fn indentation(contact: f64, z: f64, force: f64, spring: f64, reference: f64) -> f64 {
    // 1 nN / (1 N/m) = 1 nm of cantilever deflection.
    (contact - z) - (force - reference) / spring
}

struct LinearBaseline {
    center_z_nm: f64,
    center_force_nn: f64,
    slope_nn_per_nm: f64,
    rms_nn: f64,
    samples: usize,
}

impl LinearBaseline {
    fn at(&self, z: f64) -> f64 {
        self.center_force_nn + self.slope_nn_per_nm * (z - self.center_z_nm)
    }

    fn fit(points: &[[f64; 2]], interval: [f64; 2]) -> Result<Self, HertzError> {
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
    input: HertzOptions,
    contact_z_nm: f64,
    reference: f64,
) -> Result<(Vec<HertzRow>, f64), HertzError> {
    let mut rows = Vec::with_capacity(points.len());
    let (mut xy, mut xx) = (0.0, 0.0);
    for &[z_nm, corrected_force_nn] in points {
        let indentation_nm = indentation(
            contact_z_nm,
            z_nm,
            corrected_force_nn,
            input.spring_constant_n_per_m,
            reference,
        );
        if !corrected_force_nn.is_finite() || !indentation_nm.is_finite() {
            return Err(HertzError::Numerical);
        }
        if corrected_force_nn <= reference {
            return Err(HertzError::NonCompressiveForce);
        }
        if indentation_nm <= 0.0 {
            return Err(HertzError::NonpositiveIndentation);
        }
        let feature = indentation_nm * indentation_nm.sqrt();
        xy += feature * (corrected_force_nn - reference);
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
        row.predicted_force_nn = reference + row.predicted_force_nn * coefficient;
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
