//! Real-valued one-dimensional signal operations shared by dataset pipelines.
use crate::{NormalizeMethod, SmoothMethod};

/// Gaussian smoothing for detection helpers that need a stable, symmetric
/// kernel but are not persisted processing steps.
pub fn gaussian_smooth(values: &[f64], sigma: f64) -> Option<Vec<f64>> {
    if values.is_empty()
        || !sigma.is_finite()
        || sigma <= 0.0
        || values.iter().any(|value| !value.is_finite())
    {
        return None;
    }
    let radius = (3.0 * sigma).ceil() as isize;
    Some(
        (0..values.len())
            .map(|index| {
                let mut weighted = 0.0;
                let mut total = 0.0;
                for offset in -radius..=radius {
                    let source =
                        (index as isize + offset).clamp(0, values.len() as isize - 1) as usize;
                    let weight = (-0.5 * (offset as f64 / sigma).powi(2)).exp();
                    weighted += values[source] * weight;
                    total += weight;
                }
                weighted / total
            })
            .collect(),
    )
}

/// Largest odd window of at least three points that fits in `len` samples.
fn fitted_window(len: usize, window: usize) -> Option<usize> {
    let fitted = (window.max(3) | 1).min(if len % 2 == 1 {
        len
    } else {
        len.saturating_sub(1)
    });
    (len >= 3 && fitted >= 3).then_some(fitted)
}

pub fn moving_average(values: &[f64], window: usize) -> Vec<f64> {
    let Some(window) = fitted_window(values.len(), window) else {
        return values.to_vec();
    };
    let half = window / 2;
    (0..values.len())
        .map(|index| {
            let low = index.saturating_sub(half);
            let high = (index + half + 1).min(values.len());
            values[low..high].iter().sum::<f64>() / (high - low) as f64
        })
        .collect()
}

/// How `savitzky_golay` treats points within half a window of either end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SavitzkyGolayEdges {
    /// Repeat the endpoint samples past each end of the signal.
    RepeatEndpoints,
    /// Evaluate the boundary window's polynomial off-center, so a polynomial
    /// signal of degree ≤ `order` is reproduced exactly everywhere.
    FitBoundaryWindow,
}

/// Least-squares polynomial smoothing: each point is replaced by the value of a
/// degree-`order` polynomial fitted over an odd `window` around it. `window`
/// must exceed `order` and fit inside `values`. Returns `None` when the fit is
/// singular.
pub fn savitzky_golay(
    values: &[f64],
    window: usize,
    order: usize,
    edges: SavitzkyGolayEdges,
) -> Option<Vec<f64>> {
    let n = values.len();
    let half = window / 2;
    let samples: Vec<Vec<f64>> = (0..window)
        .map(|sample| {
            let x = sample as f64 - half as f64;
            let mut powers = vec![1.0; order + 1];
            for k in 1..=order {
                powers[k] = powers[k - 1] * x;
            }
            powers
        })
        .collect();
    let gram = gram_matrix(&samples, order + 1);
    match edges {
        SavitzkyGolayEdges::RepeatEndpoints => {
            let solution = plotium_analysis::fit::solve_linear(&gram, &unit_vector(order + 1, 0))?;
            let weights: Vec<f64> = samples
                .iter()
                .map(|powers| dot(powers, &solution))
                .collect();
            Some(
                (0..n)
                    .map(|index| {
                        weights
                            .iter()
                            .enumerate()
                            .map(|(offset, weight)| {
                                weight * values[(index + offset).saturating_sub(half).min(n - 1)]
                            })
                            .sum()
                    })
                    .collect(),
            )
        }
        SavitzkyGolayEdges::FitBoundaryWindow => {
            let weights = boundary_weights(&gram, &samples)?;
            Some(
                (0..n)
                    .map(|index| {
                        let (start, position) = if index < half {
                            (0, index)
                        } else if index + half >= n {
                            (n - window, index - (n - window))
                        } else {
                            (index - half, half)
                        };
                        weights[position]
                            .iter()
                            .zip(&values[start..start + window])
                            .fold(0.0, |sum, (weight, value)| sum + value * weight)
                    })
                    .collect(),
            )
        }
    }
}

fn gram_matrix(samples: &[Vec<f64>], size: usize) -> Vec<Vec<f64>> {
    let mut gram = vec![vec![0.0; size]; size];
    for powers in samples {
        for (row, row_power) in gram.iter_mut().zip(powers) {
            for (value, column_power) in row.iter_mut().zip(powers) {
                *value += row_power * column_power;
            }
        }
    }
    gram
}

/// `weights[p][i]`: contribution of sample `i` to the fitted polynomial
/// evaluated at window position `p`.
fn boundary_weights(gram: &[Vec<f64>], samples: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let size = gram.len();
    let columns = (0..size)
        .map(|k| plotium_analysis::fit::solve_linear(gram, &unit_vector(size, k)))
        .collect::<Option<Vec<_>>>()?;
    let projection: Vec<Vec<f64>> = (0..size)
        .map(|row| {
            let inverse_row: Vec<f64> = columns.iter().map(|column| column[row]).collect();
            samples
                .iter()
                .map(|powers| dot(&inverse_row, powers))
                .collect()
        })
        .collect();
    Some(
        samples
            .iter()
            .map(|powers| {
                (0..samples.len())
                    .map(|sample| {
                        powers
                            .iter()
                            .zip(&projection)
                            .map(|(power, row)| power * row[sample])
                            .sum()
                    })
                    .collect()
            })
            .collect(),
    )
}

fn unit_vector(size: usize, index: usize) -> Vec<f64> {
    let mut unit = vec![0.0; size];
    unit[index] = 1.0;
    unit
}

fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter().zip(right).map(|(a, b)| a * b).sum()
}

/// Asymmetric background estimate: SNIP clipping on the fourth root of the
/// signal, with the clipping width growing to `iterations` points.
pub fn snip_background(input: &[f64], iterations: u16) -> Vec<f64> {
    let mut transformed: Vec<_> = input
        .iter()
        .map(|value| value.max(0.0).sqrt().sqrt())
        .collect();
    let max_iteration = usize::from(iterations).min(input.len().saturating_sub(1) / 2);
    for offset in 1..=max_iteration {
        let previous = transformed.clone();
        for index in offset..input.len() - offset {
            transformed[index] =
                previous[index].min((previous[index - offset] + previous[index + offset]) * 0.5);
        }
    }
    transformed.into_iter().map(|value| value.powi(4)).collect()
}

/// Largest value, floored at zero.
pub fn maximum(values: &[f64]) -> f64 {
    values.iter().copied().fold(0.0, f64::max)
}

/// Signed trapezoidal integral of `values` along `x`.
pub fn trapezoid_area(x: &[f64], values: &[f64]) -> f64 {
    x.windows(2)
        .zip(values.windows(2))
        .map(|(x, y)| (x[1] - x[0]) * (y[0] + y[1]) * 0.5)
        .sum()
}

pub fn normalize(axis: &[f64], values: &mut [f64], method: NormalizeMethod) {
    let scale = match method {
        NormalizeMethod::MaxPeak => values.iter().map(|c| c.abs()).fold(0.0, f64::max),
        NormalizeMethod::TotalArea => values.iter().map(|c| c.abs()).sum::<f64>() * axis_step(axis),
        NormalizeMethod::Constant { divisor } => divisor,
    };
    if scale.is_finite() && scale.abs() > f64::MIN_POSITIVE {
        for c in values {
            *c /= scale;
        }
    }
}

pub fn axis_step(ppm: &[f64]) -> f64 {
    if ppm.len() < 2 {
        return 1.0;
    }
    let span = (ppm[ppm.len() - 1] - ppm[0]).abs();
    if span > 0.0 {
        span / (ppm.len() - 1) as f64
    } else {
        1.0
    }
}

pub fn smooth(values: &[f64], method: SmoothMethod) -> Vec<f64> {
    match method {
        SmoothMethod::MovingAverage { window } => moving_average(values, usize::from(window)),
        SmoothMethod::SavitzkyGolay { window, poly_order } => {
            let Some(window) = fitted_window(values.len(), usize::from(window)) else {
                return values.to_vec();
            };
            let order = usize::from(poly_order).clamp(1, window - 1);
            savitzky_golay(values, window, order, SavitzkyGolayEdges::FitBoundaryWindow)
                .unwrap_or_else(|| values.to_vec())
        }
    }
}

#[cfg(test)]
#[path = "signal_tests.rs"]
mod tests;
