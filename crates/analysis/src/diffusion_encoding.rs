//! Sequence-specific diffusion encoding, with all durations in seconds.

/// Rectangular bipolar-gradient double stimulated echo (JEOL Delta V6).
/// `delta` is the total encoding width, twice the individual lobe width.
pub fn jeol_bpp_dste_time(
    delta: f64,
    big_delta: f64,
    tau_1: f64,
    tau_2: f64,
) -> Result<f64, String> {
    if [delta, big_delta, tau_1, tau_2]
        .iter()
        .any(|v| !v.is_finite())
        || delta <= 0.0
        || big_delta <= 0.0
        || tau_1 < 0.0
        || tau_2 < 0.0
    {
        return Err("Enter finite, positive diffusion durations and nonnegative intervals.".into());
    }
    let time = big_delta - 2.0 * delta / 3.0 - (tau_1 + tau_2) / 2.0;
    if time <= 0.0 {
        return Err("Enter parameters giving a positive diffusion encoding time.".into());
    }
    Ok(time)
}

pub fn rectangular_b_values(
    gamma: f64,
    delta: f64,
    time: f64,
    gradients: &[f64],
) -> Result<Vec<f64>, String> {
    if !gamma.is_finite()
        || gamma == 0.0
        || !delta.is_finite()
        || delta <= 0.0
        || !time.is_finite()
        || time <= 0.0
        || gradients.iter().any(|g| !g.is_finite())
    {
        return Err("Confirm finite gradient values and valid encoding parameters.".into());
    }
    let values: Vec<_> = gradients
        .iter()
        .map(|g| (gamma * delta * g).powi(2) * time)
        .collect();
    validate_b_values(&values, gradients.len())?;
    Ok(values)
}

pub fn validate_b_values(values: &[f64], count: usize) -> Result<(), String> {
    if values.len() != count || count < 3 {
        return Err(format!(
            "Provide one b value per increment ({count} required, at least three)."
        ));
    }
    if values.iter().any(|b| !b.is_finite() || *b < 0.0) {
        return Err("Provide finite, nonnegative b values in s/m².".into());
    }
    let mut unique = values.to_vec();
    unique.sort_by(f64::total_cmp);
    unique.dedup();
    if unique.len() < 3 {
        return Err("Provide at least three distinct b values.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rectangular_bpp_dste_reference() {
        let c = jeol_bpp_dste_time(0.0035, 0.1, 0.001, 0.001).unwrap();
        assert!((c * 1e3 - 96.6666666667).abs() < 1e-9);
        let b = rectangular_b_values(267.52218744e6, 0.0035, c, &[0.02, 0.04, 0.28]).unwrap();
        assert!((b[0] / 3.38994e7 - 1.0).abs() < 1e-5);
        assert!((b[2] / 6.64429e9 - 1.0).abs() < 1e-5);
    }
    #[test]
    fn b_validation_accepts_unsorted_and_rejects_invalid_inputs() {
        assert!(validate_b_values(&[3., 0., 1., 1.], 4).is_ok());
        for values in [
            vec![0., 1.],
            vec![0., 1., f64::NAN],
            vec![-1., 0., 1.],
            vec![1.; 3],
        ] {
            assert!(validate_b_values(&values, 3).is_err());
        }
    }
}
