use super::*;

pub(crate) fn diagnose_noise(
    values: &[f32],
    rows: usize,
    cols: usize,
    provenance: EstimateProvenance,
) -> Result<EstimateResult, String> {
    if rows == 0 || cols == 0 || rows.checked_mul(cols) != Some(values.len()) {
        return Err("Noise estimate requires a nonempty matching scalar plane".into());
    }
    let (location, sigma) = plotium_analysis::robust::amplitude_location_scale(values)
        .ok_or("Noise estimate requires finite samples")?;
    let scale = EstimatedScale::new(sigma).ok_or("Noise estimate returned an invalid scale")?;
    let baseline = location.abs();
    Ok(EstimateResult::Noise(NoiseEstimate {
        scale,
        baseline,
        provenance,
    }))
}
