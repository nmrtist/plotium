//! Edge energy search and pre-edge/post-edge normalization.

use super::{XasError, check_spectrum, index_nearest, index_of};
use crate::signal;
use plotium_analysis::fit::solve_linear;

const MAX_NNORM: usize = 5;
/// Limits the resampled grid when the search step is far below the data step.
const MAX_RESAMPLE_FACTOR: usize = 50;

/// Every range is relative to `e0` and left `None` for the automatic choice.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PreEdgeParams {
    pub e0: Option<f64>,
    pub step: Option<f64>,
    pub pre1: Option<f64>,
    pub pre2: Option<f64>,
    pub norm1: Option<f64>,
    pub norm2: Option<f64>,
    pub nnorm: Option<usize>,
    /// Victoreen exponent: the pre-edge line is fitted to `mu * E^nvict`.
    pub nvict: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreEdge {
    pub e0: f64,
    pub edge_step: f64,
    pub norm: Vec<f64>,
    pub flat: Vec<f64>,
    pub pre_edge: Vec<f64>,
    pub post_edge: Vec<f64>,
    pub pre1: f64,
    pub pre2: f64,
    pub norm1: f64,
    pub norm2: f64,
    pub nnorm: usize,
}

/// Edge energy as the steepest point of mu(E), skipping isolated glitches.
/// `energy` must be strictly increasing, see `prepare_spectrum`.
pub fn find_e0(energy: &[f64], mu: &[f64]) -> Result<f64, XasError> {
    check_spectrum(energy, mu)?;
    let count = energy.len();
    if count < 12 {
        return Err(XasError::TooFewPoints(format!(
            "edge search needs 12 points, have {count}"
        )));
    }
    let raw_step = energy_step(&energy[..count - 1]);
    let rough = steepest_point(energy, mu, None)?;
    let mut rough_e0 = rough.energy;
    let mut start = 3.max(rough.index.saturating_sub(75));
    let mut stop = (rough.index + 75).min(count - 3);
    // A maximum in the first 5% is almost always a glitch at the scan start.
    if (rough.index as f64) < 0.05 * count as f64 {
        rough_e0 = energy.iter().sum::<f64>() / count as f64;
        start = 3.max(rough.index.saturating_sub(20));
        stop = count - 3;
    }
    // The observed step can be vanishingly small, so average it with a value
    // scaled to the edge energy.
    let step = 0.5 * (raw_step.clamp(0.01, 1.0) + (rough_e0 / 25_000.0).clamp(0.01, 1.0));
    let refined = steepest_point(&energy[start..stop], &mu[start..stop], Some(step))?;
    Ok(if refined.index < 1 {
        energy[start + 2]
    } else {
        refined.energy
    })
}

/// Typical step: the mean of ten small steps after ignoring the smallest 1%.
fn energy_step(energy: &[f64]) -> f64 {
    let skip = energy.len() / 100;
    if skip == 0 {
        return 1.0;
    }
    let kept = &energy[..energy.len() - 1];
    let mut steps: Vec<f64> = kept[skip..kept.len() - skip]
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .collect();
    steps.sort_by(f64::total_cmp);
    let selected: Vec<f64> = steps.into_iter().skip(skip).take(10).collect();
    selected.iter().sum::<f64>() / selected.len().max(1) as f64
}

struct Steepest {
    energy: f64,
    index: usize,
}

fn steepest_point(
    energy: &[f64],
    mu: &[f64],
    smooth_step: Option<f64>,
) -> Result<Steepest, XasError> {
    let count = energy.len() - 1;
    let (energy, mu) = (&energy[..count], &mu[..count]);
    let mut slope: Vec<f64> = gradient(mu)
        .iter()
        .zip(gradient(energy))
        .map(|(rise, run)| rise / run)
        .collect();
    for value in &mut slope {
        if !value.is_finite() {
            *value = -1.0;
        }
    }
    if let Some(step) = smooth_step {
        slope = smoothed_on_uniform_grid(energy, &slope, step)?;
    }
    let margin = 3.max(count / 50);
    if count <= 2 * margin {
        return Ok(Steepest {
            energy: energy[0],
            index: 0,
        });
    }
    let inner = &slope[margin..count - margin];
    let low = inner.iter().copied().fold(f64::INFINITY, f64::min);
    let high = inner.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let range = (high - low).max(1e-10);
    let scaled: Vec<f64> = slope.iter().map(|value| (value - low) / range).collect();

    let mut threshold = if count > 20 { 0.6 } else { 0.3 };
    let above = |threshold: f64| scaled.iter().filter(|value| **value > threshold).count();
    if above(threshold) < 3 {
        for _ in 0..2 {
            if above(threshold) > 3 {
                break;
            }
            threshold *= 0.5;
        }
    }
    let accept_all = above(threshold) < 3;
    let is_high = |index: usize| accept_all || scaled[index] > threshold;
    let mut best = 0;
    let mut largest = 0.0;
    let candidates = scaled.iter().enumerate().take(count - margin + 1);
    for (index, value) in candidates.skip(margin) {
        if *value > largest && is_high(index) && is_high(index + 1) && is_high(index - 1) {
            best = index;
            largest = *value;
        }
    }
    Ok(Steepest {
        energy: energy[best],
        index: best,
    })
}

/// Central differences per array index, one-sided at both ends.
fn gradient(values: &[f64]) -> Vec<f64> {
    let last = values.len() - 1;
    (0..=last)
        .map(|index| {
            let low = index.saturating_sub(1);
            let high = (index + 1).min(last);
            (values[high] - values[low]) / (high - low) as f64
        })
        .collect()
}

/// Energy steps vary by orders of magnitude across a scan, so smoothing has to
/// happen on a uniform grid to act over a fixed width in eV.
fn smoothed_on_uniform_grid(x: &[f64], y: &[f64], step: f64) -> Result<Vec<f64>, XasError> {
    let span = x[x.len() - 1] - x[0];
    let points = ((span / step) as usize)
        .saturating_add(2)
        .min(MAX_RESAMPLE_FACTOR * x.len());
    let spacing = span / (points - 1) as f64;
    let uniform_x: Vec<f64> = (0..points).map(|i| x[0] + i as f64 * spacing).collect();
    let uniform_y = interpolate(x, y, &uniform_x);
    let smoothed = signal::gaussian_smooth(&uniform_y, 1.0)
        .ok_or_else(|| XasError::FitFailed("edge derivative is not finite".into()))?;
    Ok(interpolate(&uniform_x, &smoothed, x))
}

/// Piecewise linear interpolation, holding the end values past the data.
pub(super) fn interpolate(x: &[f64], y: &[f64], at: &[f64]) -> Vec<f64> {
    at.iter()
        .map(|&position| {
            let right = x.partition_point(|value| *value < position);
            let right = right.clamp(1, x.len() - 1);
            let left = right - 1;
            let fraction = ((position - x[left]) / (x[right] - x[left])).clamp(0.0, 1.0);
            y[left] + fraction * (y[right] - y[left])
        })
        .collect()
}

/// Polynomial fitted on `x` mapped to [-1, 1]: edge energies are near 10^4 eV,
/// where raw powers would make the normal equations singular.
struct Polynomial {
    center: f64,
    half_width: f64,
    coefficients: Vec<f64>,
}

impl Polynomial {
    fn fit(x: &[f64], y: &[f64], degree: usize) -> Result<Self, XasError> {
        let (first, last) = (x[0], x[x.len() - 1]);
        let center = 0.5 * (first + last);
        let half_width = (0.5 * (last - first)).max(f64::MIN_POSITIVE);
        let size = degree + 1;
        let mut gram = vec![vec![0.0; size]; size];
        let mut moments = vec![0.0; size];
        for (x, y) in x.iter().zip(y) {
            let mapped = (x - center) / half_width;
            let powers = powers(mapped, size);
            for (row, row_power) in gram.iter_mut().zip(&powers) {
                for (value, column_power) in row.iter_mut().zip(&powers) {
                    *value += row_power * column_power;
                }
            }
            for (moment, power) in moments.iter_mut().zip(&powers) {
                *moment += power * y;
            }
        }
        let coefficients = solve_linear(&gram, &moments)
            .ok_or_else(|| XasError::FitFailed("singular polynomial fit".into()))?;
        Ok(Self {
            center,
            half_width,
            coefficients,
        })
    }

    fn eval(&self, x: f64) -> f64 {
        let mapped = (x - self.center) / self.half_width;
        self.coefficients
            .iter()
            .rev()
            .fold(0.0, |sum, coefficient| sum * mapped + coefficient)
    }
}

fn powers(x: f64, count: usize) -> Vec<f64> {
    let mut power = 1.0;
    (0..count)
        .map(|_| {
            let value = power;
            power *= x;
            value
        })
        .collect()
}

/// Fits the pre-edge line and post-edge polynomial, and takes the edge step as
/// their difference at `e0`. `energy` must be strictly increasing, see
/// `prepare_spectrum`.
pub fn pre_edge(energy: &[f64], mu: &[f64], params: &PreEdgeParams) -> Result<PreEdge, XasError> {
    check_spectrum(energy, mu)?;
    let count = energy.len();
    if count < 12 {
        return Err(XasError::TooFewPoints(format!(
            "normalization needs 12 points, have {count}"
        )));
    }
    let (first, last) = (energy[0], energy[count - 1]);
    let e0 = match params.e0 {
        Some(e0) if e0 >= energy[1] && e0 <= energy[count - 2] => e0,
        _ => find_e0(energy, mu)?,
    };
    let edge_index = index_nearest(energy, e0);

    let rounding = if edge_index > 20 { 5.0 } else { 2.0 };
    let pre1 = params
        .pre1
        .unwrap_or_else(|| rounding * ((energy[1] - e0) / rounding).round_ties_even())
        .max(first - e0);
    let pre2 = params.pre2.unwrap_or(0.5 * pre1);
    let (pre1, pre2) = (pre1.min(pre2), pre1.max(pre2));

    let mut norm2 = params
        .norm2
        .unwrap_or_else(|| 5.0 * ((last - e0) / 5.0).round_ties_even());
    if norm2 < 0.0 {
        norm2 = last - e0 - norm2;
    }
    norm2 = norm2.min(last - e0);
    let norm1 = params
        .norm1
        .unwrap_or_else(|| (5.0 * (norm2 / 15.0).round_ties_even()).min(25.0));
    let (norm1, norm2) = (norm1.min(norm2), norm1.max(norm2));
    let norm1 = norm1.min(norm2 - 2.0);
    let wanted_nnorm = params.nnorm.unwrap_or(if norm2 - norm1 < 30.0 {
        0
    } else if norm2 - norm1 < 300.0 {
        1
    } else {
        2
    });
    let mut nnorm = wanted_nnorm.min(MAX_NNORM);

    let pre_curve = pre_edge_curve(energy, mu, e0 + pre1, e0 + pre2, params.nvict)?;

    let mut low = index_of(energy, norm1 + e0).min(count - 3);
    let high = index_nearest(energy, norm2 + e0);
    if high < low + 2 {
        low = low.saturating_sub(2);
        nnorm = 0;
    } else if high < low + 5 {
        nnorm = nnorm.min(1);
    }
    let high = high.max(low + 1).min(count);
    let residual: Vec<f64> = (low..high).map(|i| mu[i] - pre_curve[i]).collect();
    let post_fit = Polynomial::fit(&energy[low..high], &residual, nnorm)?;
    let post_curve: Vec<f64> = energy
        .iter()
        .zip(&pre_curve)
        .map(|(energy, pre)| pre + post_fit.eval(*energy))
        .collect();

    let edge_step = params
        .step
        .unwrap_or(post_curve[edge_index] - pre_curve[edge_index])
        .abs()
        .max(1e-12);
    let norm: Vec<f64> = mu
        .iter()
        .zip(&pre_curve)
        .map(|(mu, pre)| (mu - pre) / edge_step)
        .collect();
    let curvature = |index: usize| (post_curve[index] - pre_curve[index]) / edge_step;
    let flat = norm
        .iter()
        .enumerate()
        .map(|(index, value)| {
            if index < edge_index {
                *value
            } else {
                value - curvature(index) + curvature(edge_index)
            }
        })
        .collect();
    Ok(PreEdge {
        e0,
        edge_step,
        norm,
        flat,
        pre_edge: pre_curve,
        post_edge: post_curve,
        pre1,
        pre2,
        norm1,
        norm2,
        nnorm,
    })
}

fn pre_edge_curve(
    energy: &[f64],
    mu: &[f64],
    from: f64,
    to: f64,
    nvict: i32,
) -> Result<Vec<f64>, XasError> {
    let low = index_of(energy, from);
    let high = index_nearest(energy, to);
    // Too few points for a slope, so the pre-edge level is a constant.
    if index_of(energy, to) < low + 3 {
        let high = high.max(low + 1).min(energy.len());
        let mean = mu[low..high].iter().sum::<f64>() / (high - low) as f64;
        return Ok(vec![mean; energy.len()]);
    }
    let high = high.max(low + 2).min(energy.len());
    let weighted: Vec<f64> = (low..high)
        .map(|index| mu[index] * energy[index].powi(nvict))
        .collect();
    let line = Polynomial::fit(&energy[low..high], &weighted, 1)?;
    Ok(energy
        .iter()
        .map(|energy| line.eval(*energy) * energy.powi(-nvict))
        .collect())
}
