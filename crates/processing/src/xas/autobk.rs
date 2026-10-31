//! AUTOBK background removal: a spline mu0(k) with knots limited by `rbkg` is
//! fitted so that chi(R) below `rbkg` is as small as possible.

use super::fourier::{XafsFft, XafsWindow, ft_window};
use super::spline::{BackgroundBasis, interpolate_not_a_knot};
use super::{
    KTOE, XasError, check_spectrum, check_transform, energy_to_k, index_nearest, index_of,
    is_positive,
};
use plotium_analysis::fit::{LmProblem, levenberg_marquardt_problem};
use std::f64::consts::PI;

const MIN_KNOTS: usize = 5;
const MAX_KNOTS: usize = 128;
const MAX_ITERATIONS: usize = 200;
/// Larch evaluates the fit residual at this k step whatever the output step,
/// which sets the balance between the low-R terms and the clamps.
const RESIDUAL_KSTEP: f64 = 0.05;
/// Relative step of the forward-difference Jacobian.
const JACOBIAN_STEP: f64 = 1e-3;

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutobkParams {
    pub rbkg: f64,
    pub nknots: Option<usize>,
    pub kmin: f64,
    /// `None` uses the end of the data.
    pub kmax: Option<f64>,
    pub kweight: i32,
    pub dk: f64,
    pub window: XafsWindow,
    pub nfft: usize,
    pub kstep: f64,
    pub nclamp: usize,
    pub clamp_lo: f64,
    pub clamp_hi: f64,
}

impl Default for AutobkParams {
    fn default() -> Self {
        Self {
            rbkg: 1.0,
            nknots: None,
            kmin: 0.0,
            kmax: None,
            kweight: 1,
            dk: 0.1,
            window: XafsWindow::Hanning,
            nfft: 2048,
            kstep: 0.05,
            nclamp: 3,
            clamp_lo: 0.0,
            clamp_hi: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Autobk {
    pub k: Vec<f64>,
    pub chi: Vec<f64>,
    /// mu0(E) above the edge; equal to mu outside the fitted range.
    pub background: Vec<f64>,
    /// `(mu - background) / edge_step` on the energy axis.
    pub chi_energy: Vec<f64>,
    /// `rbkg` after enforcing the transform's resolution.
    pub rbkg: f64,
    pub knot_count: usize,
}

/// `energy` must be strictly increasing, see `prepare_spectrum`. `e0` and
/// `edge_step` usually come from `pre_edge`.
pub fn autobk(
    energy: &[f64],
    mu: &[f64],
    e0: f64,
    edge_step: f64,
    params: &AutobkParams,
) -> Result<Autobk, XasError> {
    check_spectrum(energy, mu)?;
    check_transform(params.kstep, params.nfft)?;
    if energy.len() < 2 || !(energy[0]..=energy[energy.len() - 1]).contains(&e0) {
        return Err(XasError::InvalidParameter(format!(
            "e0 = {e0} eV is outside the scan"
        )));
    }
    if edge_step == 0.0 || !edge_step.is_finite() {
        return Err(XasError::InvalidParameter(format!(
            "edge step {edge_step} cannot scale chi"
        )));
    }
    if !params.rbkg.is_finite() || !params.kmin.is_finite() {
        return Err(XasError::InvalidParameter(
            "rbkg and kmin must be finite".into(),
        ));
    }
    let edge_index = index_of(energy, e0);
    let rgrid = PI / (params.kstep * params.nfft as f64);
    let rbkg = params.rbkg.max(2.0 * rgrid);

    let k_raw = energy_to_k(&energy[edge_index..], e0);
    let k_end = k_raw[k_raw.len() - 1];
    let kmax = match params.kmax {
        Some(kmax) if kmax >= 0.0 => kmax.min(k_end),
        _ => k_end,
    };
    let span = kmax - params.kmin;
    if !is_positive(span) {
        return Err(XasError::InvalidParameter(format!(
            "kmin = {} is above the {kmax} Å⁻¹ data range",
            params.kmin
        )));
    }
    let k_points = (1.01 + kmax / params.kstep) as usize;
    if k_points > params.nfft {
        return Err(XasError::InvalidParameter(format!(
            "{k_points} k points do not fit in nfft = {}",
            params.nfft
        )));
    }
    let k_out: Vec<f64> = (0..k_points)
        .map(|index| params.kstep * index as f64)
        .collect();
    let fit_end = (2 + index_of(energy, e0 + kmax * kmax * KTOE)).min(energy.len()) - 1;
    let k_fit = &k_raw[..=fit_end - edge_index];
    let mu_fit = &mu[edge_index..=fit_end];
    if k_fit.len() < 4 || k_out.len() < 4 {
        return Err(XasError::TooFewPoints(format!(
            "{} points above the edge for {} k points",
            k_fit.len(),
            k_out.len()
        )));
    }

    let natural_knots = ((2.0 * rbkg * span / PI) as usize).saturating_add(1);
    let low_r_points = (1.0 + (natural_knots - 1) as f64 * PI / (2.0 * rgrid * span)) as usize;
    let knot_count = params
        .nknots
        .unwrap_or(natural_knots)
        .clamp(MIN_KNOTS, MAX_KNOTS);

    let mut sites = Vec::with_capacity(knot_count);
    let mut initial = Vec::with_capacity(knot_count);
    for knot in 0..knot_count {
        let target = params.kmin + knot as f64 * span / (knot_count - 1) as f64;
        let index = index_nearest(&k_raw, target);
        let above = (index + 5).min(k_raw.len() - 1);
        let below = index.saturating_sub(5);
        sites.push(k_raw[index]);
        initial.push(
            (2.0 * mu[index + edge_index] + mu[above + edge_index] + mu[below + edge_index]) / 4.0,
        );
    }
    if sites.windows(2).any(|pair| pair[1] <= pair[0]) {
        return Err(XasError::TooFewPoints(
            "data are too sparse for the requested spline knots".into(),
        ));
    }

    let basis = BackgroundBasis::new(&sites, k_fit);
    let coefficients = basis.interpolate(&sites, &initial)?;
    let window = ft_window(
        &k_out,
        params.kmin,
        kmax,
        params.dk,
        params.dk,
        params.window,
    );
    let weight: Vec<f64> = k_out
        .iter()
        .zip(&window)
        .map(|(k, window)| k.powi(params.kweight) * window)
        .collect();
    let mut problem = LowRProblem {
        basis: &basis,
        k_fit,
        mu_fit,
        k_out: &k_out,
        weight,
        transform: XafsFft::new(params.nfft, RESIDUAL_KSTEP),
        low_r_points: low_r_points.min(params.nfft / 2),
        nclamp: params.nclamp.min(k_out.len()),
        clamp_lo: params.clamp_lo.abs(),
        clamp_hi: params.clamp_hi.abs(),
    };
    let (coefficients, _) =
        levenberg_marquardt_problem(&mut problem, &coefficients, MAX_ITERATIONS)
            .ok_or_else(|| XasError::FitFailed("background fit did not converge".into()))?;

    let fitted = basis.eval(&coefficients);
    let residual: Vec<f64> = mu_fit
        .iter()
        .zip(&fitted)
        .map(|(mu, bkg)| mu - bkg)
        .collect();
    let chi: Vec<f64> = interpolate_not_a_knot(k_fit, &residual, &k_out)
        .into_iter()
        .map(|value| value / edge_step)
        .collect();
    if !chi.iter().all(|value| value.is_finite()) {
        return Err(XasError::FitFailed("background fit diverged".into()));
    }
    let mut background = mu.to_vec();
    background[edge_index..edge_index + fitted.len()].copy_from_slice(&fitted);
    let chi_energy = mu
        .iter()
        .zip(&background)
        .map(|(mu, bkg)| (mu - bkg) / edge_step)
        .collect();
    Ok(Autobk {
        k: k_out,
        chi,
        background,
        chi_energy,
        rbkg,
        knot_count,
    })
}

struct LowRProblem<'a> {
    basis: &'a BackgroundBasis,
    k_fit: &'a [f64],
    mu_fit: &'a [f64],
    k_out: &'a [f64],
    weight: Vec<f64>,
    transform: XafsFft,
    low_r_points: usize,
    nclamp: usize,
    clamp_lo: f64,
    clamp_hi: f64,
}

impl LowRProblem<'_> {
    /// Low-R real and imaginary parts, then the end-point clamps on chi(k).
    fn residuals(&self, coefficients: &[f64]) -> Vec<f64> {
        let background = self.basis.eval(coefficients);
        let difference: Vec<f64> = self
            .mu_fit
            .iter()
            .zip(&background)
            .map(|(mu, background)| mu - background)
            .collect();
        let chi = interpolate_not_a_knot(self.k_fit, &difference, self.k_out);
        let spectrum = self.transform.forward(
            chi.iter()
                .zip(&self.weight)
                .map(|(chi, weight)| chi * weight),
        );
        let mut residuals = Vec::with_capacity(2 * self.low_r_points + 2 * self.nclamp);
        for value in &spectrum[..self.low_r_points] {
            residuals.push(value.re);
            residuals.push(value.im);
        }
        if self.nclamp > 0 {
            let mean = residuals.iter().map(|value| value * value).sum::<f64>()
                / residuals.len().max(1) as f64;
            let scale = 0.1 + 10.0 * mean;
            let tail = chi.len() - self.nclamp;
            residuals.extend(chi[..self.nclamp].iter().map(|v| self.clamp_lo * scale * v));
            residuals.extend(chi[tail..].iter().map(|v| self.clamp_hi * scale * v));
        }
        residuals
    }
}

impl LmProblem for LowRProblem<'_> {
    fn cost(&mut self, coefficients: &[f64]) -> f64 {
        let residuals = self.residuals(coefficients);
        residuals.iter().map(|value| value * value).sum()
    }

    fn normal_equations(&mut self, p: &[f64], jtj: &mut [Vec<f64>], jtr: &mut [f64]) {
        let residuals = self.residuals(p);
        let mut shifted = p.to_vec();
        let columns: Vec<Vec<f64>> = (0..p.len())
            .map(|index| {
                let step = if p[index] == 0.0 {
                    JACOBIAN_STEP
                } else {
                    JACOBIAN_STEP * p[index].abs()
                };
                shifted[index] = p[index] + step;
                let moved = self.residuals(&shifted);
                shifted[index] = p[index];
                moved
                    .iter()
                    .zip(&residuals)
                    .map(|(moved, base)| (moved - base) / step)
                    .collect()
            })
            .collect();
        for (a, column) in columns.iter().enumerate() {
            // `lm_core` adds this vector to the parameters.
            jtr[a] = -dot(column, &residuals);
            for (b, other) in columns.iter().enumerate() {
                jtj[a][b] = dot(column, other);
            }
        }
    }
}

fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter().zip(right).map(|(a, b)| a * b).sum()
}
