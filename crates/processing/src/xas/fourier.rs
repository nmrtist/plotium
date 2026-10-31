//! XAFS Fourier transform from chi(k) to chi(R), with its window functions.

use super::normalize::interpolate;
use super::{MAX_NFFT, XasError, check_transform};
use num_complex::Complex64;
use rustfft::{Fft, FftPlanner};
use std::f64::consts::PI;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum XafsWindow {
    #[default]
    Hanning,
    KaiserBessel,
    Parzen,
    Welch,
    Gaussian,
    Sine,
}

/// Window over `x`, a uniform grid starting at zero. `dx` and `dx2` are the
/// tapering widths at the two ends; the Kaiser-Bessel `dx` is its shape.
pub fn ft_window(
    x: &[f64],
    xmin: f64,
    xmax: f64,
    dx: f64,
    dx2: f64,
    window: XafsWindow,
) -> Vec<f64> {
    let count = x.len();
    let last = count.saturating_sub(1);
    if count < 2 {
        return vec![1.0; count];
    }
    let step = (x[last] - x[0]) / last as f64;
    let eps = 1e-4 * step;
    let index = |value: f64| (((value + eps) / step) as isize).max(0) as usize;
    let mut i1 = index((xmin - dx / 2.0).max(x[0])).min(last);
    let i2 = index(xmin + dx / 2.0 + eps).min(last);
    let mut i3 = index(xmax - dx2 / 2.0 - eps).min(last);
    let i4 = index((xmax + dx2 / 2.0).min(x[last])).min(last);
    if i2 == i1 {
        i1 = i2.saturating_sub(1);
    }
    if i4 == i3 {
        i3 = i2.max(i4.saturating_sub(1));
    }
    let (x1, mut x2, x3, mut x4) = (x[i1], x[i2], x[i3], x[i4]);
    if x1 == x2 {
        x2 += eps;
    }
    if x3 == x4 {
        x4 += eps;
    }
    let mut values = vec![0.0; count];
    if i3 > i2 {
        values[i2..i3].fill(1.0);
    }
    let rise = |value: f64| (value - x1) / (x2 - x1);
    let fall = |value: f64| (value - x3) / (x4 - x3);
    match window {
        XafsWindow::Hanning => {
            for index in i1..=i2 {
                values[index] = (PI / 2.0 * rise(x[index])).sin().powi(2);
            }
            for index in i3..=i4 {
                values[index] = (PI / 2.0 * fall(x[index])).cos().powi(2);
            }
        }
        XafsWindow::Parzen => {
            for index in i1..=i2 {
                values[index] = rise(x[index]);
            }
            for index in i3..=i4 {
                values[index] = 1.0 - fall(x[index]);
            }
        }
        XafsWindow::Welch => {
            for index in i1..=i2 {
                values[index] = 1.0 - ((x[index] - x2) / (x2 - x1)).powi(2);
            }
            for index in i3..=i4 {
                values[index] = 1.0 - fall(x[index]).powi(2);
            }
        }
        XafsWindow::KaiserBessel => {
            let center = (x4 + x1) / 2.0;
            let width = (x4 - x1) / 2.0;
            let scale = (bessel_i0(dx) - 1.0).max(1e-10);
            for (value, x) in values.iter_mut().zip(x) {
                let argument = (1.0 - ((x - center) / width).powi(2)).max(0.0);
                *value = (bessel_i0(dx * argument.sqrt()) - 1.0) / scale;
            }
        }
        XafsWindow::Sine => {
            for index in i1..=i4 {
                values[index] = (PI * (x4 - x[index]) / (x4 - x1)).sin();
            }
        }
        XafsWindow::Gaussian => {
            let center = (x4 + x1) / 2.0;
            let sigma = dx.max(eps);
            for (value, x) in values.iter_mut().zip(x) {
                *value = (-(x - center).powi(2) / (2.0 * sigma * sigma)).exp();
            }
        }
    }
    values
}

/// Modified Bessel function of order zero, by its power series.
fn bessel_i0(x: f64) -> f64 {
    let quarter = 0.25 * x * x;
    let mut term = 1.0;
    let mut sum = 1.0;
    for n in 1..200 {
        term *= quarter / (n * n) as f64;
        sum += term;
        if term < 1e-17 * sum {
            break;
        }
    }
    sum
}

/// Forward transform with the XAFS normalization, reused across evaluations.
pub(super) struct XafsFft {
    plan: Arc<dyn Fft<f64>>,
    size: usize,
    scale: f64,
}

impl XafsFft {
    pub(super) fn new(size: usize, kstep: f64) -> Self {
        Self {
            plan: FftPlanner::new().plan_fft_forward(size),
            size,
            scale: kstep / PI.sqrt(),
        }
    }

    /// Zero-padded to the transform size; returns the non-negative R half.
    pub(super) fn forward(&self, values: impl IntoIterator<Item = f64>) -> Vec<Complex64> {
        let mut buffer = vec![Complex64::new(0.0, 0.0); self.size];
        for (slot, value) in buffer.iter_mut().zip(values) {
            slot.re = value;
        }
        self.plan.process(&mut buffer);
        buffer.truncate(self.size / 2);
        for value in &mut buffer {
            *value *= self.scale;
        }
        buffer
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XftfParams {
    pub kmin: f64,
    pub kmax: f64,
    pub kweight: i32,
    pub dk: f64,
    pub dk2: f64,
    pub window: XafsWindow,
    pub rmax_out: f64,
    pub nfft: usize,
    pub kstep: f64,
}

impl Default for XftfParams {
    fn default() -> Self {
        Self {
            kmin: 0.0,
            kmax: 20.0,
            kweight: 2,
            dk: 1.0,
            dk2: 1.0,
            window: XafsWindow::KaiserBessel,
            rmax_out: 10.0,
            nfft: 2048,
            kstep: 0.05,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChiR {
    pub r: Vec<f64>,
    pub re: Vec<f64>,
    pub im: Vec<f64>,
    pub magnitude: Vec<f64>,
    /// Unwrapped phase.
    pub phase: Vec<f64>,
    /// The k window on the transform's uniform grid.
    pub window: Vec<f64>,
}

/// Transforms chi(k), resampling it onto `params.kstep` when needed.
pub fn xftf(k: &[f64], chi: &[f64], params: &XftfParams) -> Result<ChiR, XasError> {
    if k.len() != chi.len() {
        return Err(XasError::LengthMismatch(k.len(), chi.len()));
    }
    if k.len() < 2 {
        return Err(XasError::TooFewPoints("chi(k) needs 2 points".into()));
    }
    check_transform(params.kstep, params.nfft)?;
    let kstep = params.kstep;
    let k_last = k[k.len() - 1];
    let span = k_last.max(params.kmax + params.dk2);
    let grid_points = (1.01 + span / kstep) as usize;
    let points = (1.01 + k_last / kstep) as usize;
    if grid_points > MAX_NFFT || points > params.nfft {
        return Err(XasError::InvalidParameter(format!(
            "{points} chi(k) points on a {grid_points} point grid do not fit in nfft = {}",
            params.nfft
        )));
    }
    let grid: Vec<f64> = (0..grid_points).map(|index| kstep * index as f64).collect();
    let mut window = ft_window(
        &grid,
        params.kmin,
        params.kmax,
        params.dk,
        params.dk2,
        params.window,
    );
    window.truncate(points);
    let resampled = interpolate(k, chi, &grid[..points]);
    let transform = XafsFft::new(params.nfft, kstep);
    let spectrum = transform.forward(
        (0..points)
            .map(|index| resampled[index] * grid[index].powi(params.kweight) * window[index]),
    );

    let rstep = PI / (kstep * params.nfft as f64);
    let count = ((1.01 + params.rmax_out / rstep) as usize).min(params.nfft / 2);
    let kept = &spectrum[..count];
    Ok(ChiR {
        r: (0..count).map(|index| rstep * index as f64).collect(),
        re: kept.iter().map(|value| value.re).collect(),
        im: kept.iter().map(|value| value.im).collect(),
        magnitude: kept.iter().map(|value| value.norm()).collect(),
        phase: unwrapped_phase(kept),
        window,
    })
}

fn unwrapped_phase(values: &[Complex64]) -> Vec<f64> {
    let mut turns = 0.0;
    let mut previous = None;
    values
        .iter()
        .map(|value| {
            let phase = value.arg();
            if let Some(last) = previous {
                let jump: f64 = (phase - last) / PI;
                turns += jump.abs().round_ties_even() * jump.signum();
            }
            previous = Some(phase);
            phase - PI * turns
        })
        .collect()
}
