//! X-ray absorption analysis: mu(E) construction, edge-step normalization,
//! AUTOBK background removal, and the XAFS Fourier transform. Defaults and
//! numerical choices follow Larch so results can be compared with it directly.

mod autobk;
mod edges;
mod fourier;
mod normalize;
mod reduction;
mod spline;

pub use autobk::{Autobk, AutobkParams, autobk};
pub use edges::{edge_energy, nearest_edge};
pub use fourier::{ChiR, XafsWindow, XftfParams, ft_window, xftf};
pub use normalize::{PreEdge, PreEdgeParams, find_e0, pre_edge};
pub use reduction::{Exafs, XasPreset, XasProcessing, XasReduction, reduce};

/// hbar^2 / (2 m_e) in eV Å^2, the CODATA value Larch uses.
const KTOE: f64 = 3.809_982_110_968_584_7;
const ETOK: f64 = 1.0 / KTOE;
/// Rows closer than this repeat the same energy.
const TINY_ENERGY: f64 = 0.0005;
const MAX_NFFT: usize = 1 << 20;

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum XasError {
    #[error("arrays have different lengths ({0} and {1})")]
    LengthMismatch(usize, usize),
    #[error("not enough points: {0}")]
    TooFewPoints(String),
    #[error("invalid parameter: {0}")]
    InvalidParameter(String),
    #[error("fit failed: {0}")]
    FitFailed(String),
}

fn check_transform(kstep: f64, nfft: usize) -> Result<(), XasError> {
    if is_positive(kstep) && (2..=MAX_NFFT).contains(&nfft) {
        Ok(())
    } else {
        Err(XasError::InvalidParameter(format!(
            "kstep must be positive and nfft in 2..={MAX_NFFT}"
        )))
    }
}

/// `ln(|i0 / it|)`, the transmission geometry.
pub fn transmission_mu(i0: &[f64], it: &[f64]) -> Result<Vec<f64>, XasError> {
    same_length(i0, it)?;
    Ok(i0
        .iter()
        .zip(it)
        .map(|(i0, it)| (i0 / it).abs().ln())
        .collect())
}

/// `ln(|it / iref|)`: the reference foil sits behind the sample, so the sample
/// chamber is its incident signal.
pub fn reference_mu(it: &[f64], iref: &[f64]) -> Result<Vec<f64>, XasError> {
    transmission_mu(it, iref)
}

/// `signal / i0`, the fluorescence geometry.
pub fn fluorescence_mu(signal: &[f64], i0: &[f64]) -> Result<Vec<f64>, XasError> {
    same_length(signal, i0)?;
    Ok(signal
        .iter()
        .zip(i0)
        .map(|(signal, i0)| signal / i0)
        .collect())
}

fn same_length(left: &[f64], right: &[f64]) -> Result<(), XasError> {
    if left.len() == right.len() {
        Ok(())
    } else {
        Err(XasError::LengthMismatch(left.len(), right.len()))
    }
}

/// Drops non-finite rows, sorts by energy, and keeps the first row of each
/// repeated energy so later steps can rely on a strictly increasing axis. Spreading
/// repeats apart instead would turn their noise into a steep false slope.
pub fn prepare_spectrum(energy: &[f64], mu: &[f64]) -> Result<(Vec<f64>, Vec<f64>), XasError> {
    same_length(energy, mu)?;
    let mut rows: Vec<(f64, f64)> = energy
        .iter()
        .copied()
        .zip(mu.iter().copied())
        .filter(|(energy, mu)| energy.is_finite() && mu.is_finite())
        .collect();
    rows.sort_by(|left, right| left.0.total_cmp(&right.0));
    let mut last = f64::NEG_INFINITY;
    rows.retain(|(energy, _)| {
        let distinct = energy - last >= TINY_ENERGY;
        if distinct {
            last = *energy;
        }
        distinct
    });
    Ok(rows.into_iter().unzip())
}

/// The checks `prepare_spectrum` output passes, for the steps that need them.
fn check_spectrum(energy: &[f64], mu: &[f64]) -> Result<(), XasError> {
    same_length(energy, mu)?;
    let increasing = energy.windows(2).all(|pair| pair[1] > pair[0]);
    if !increasing || !energy.iter().chain(mu).all(|value| value.is_finite()) {
        return Err(XasError::InvalidParameter(
            "energy must increase strictly and all values must be finite".into(),
        ));
    }
    Ok(())
}

fn is_positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

/// Photoelectron wavenumber, negative below `e0`.
pub fn energy_to_k(energy: &[f64], e0: f64) -> Vec<f64> {
    energy
        .iter()
        .map(|energy| {
            let above = energy - e0;
            above.signum() * (ETOK * above.abs()).sqrt()
        })
        .collect()
}

/// `k^weight * chi`.
pub fn k_weighted(k: &[f64], chi: &[f64], weight: i32) -> Result<Vec<f64>, XasError> {
    same_length(k, chi)?;
    Ok(k.iter()
        .zip(chi)
        .map(|(k, chi)| chi * k.powi(weight))
        .collect())
}

/// Index of the last value at or below `value`, 0 when none is.
fn index_of(sorted: &[f64], value: f64) -> usize {
    sorted.partition_point(|x| *x <= value).saturating_sub(1)
}

fn index_nearest(sorted: &[f64], value: f64) -> usize {
    let right = sorted.partition_point(|x| *x < value);
    if right == 0 {
        0
    } else if right == sorted.len() || value - sorted[right - 1] <= sorted[right] - value {
        right - 1
    } else {
        right
    }
}

#[cfg(test)]
mod tests;
