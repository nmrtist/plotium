//! Cubic splines for AUTOBK: a B-spline basis for the background and a
//! not-a-knot interpolant for resampling chi(k). Both reproduce FITPACK's
//! interpolating splines (`s = 0`) and extrapolate with the end polynomials.

use super::XasError;
use plotium_analysis::fit::solve_linear;

const ORDER: usize = 3;

/// The basis of the not-a-knot cubic spline through `sites`, one coefficient
/// per site, tabulated at fixed evaluation points.
pub(super) struct BackgroundBasis {
    knots: Vec<f64>,
    /// First coefficient and the four non-zero basis values at each point.
    rows: Vec<(usize, [f64; 4])>,
}

impl BackgroundBasis {
    pub(super) fn new(sites: &[f64], points: &[f64]) -> Self {
        let count = sites.len();
        let mut knots = vec![sites[0]; ORDER + 1];
        knots.extend_from_slice(&sites[2..count - 2]);
        knots.extend(std::iter::repeat_n(sites[count - 1], ORDER + 1));
        let rows = points.iter().map(|x| basis_row(&knots, *x)).collect();
        Self { knots, rows }
    }

    /// Coefficients of the spline through `(sites, values)`.
    pub(super) fn interpolate(&self, sites: &[f64], values: &[f64]) -> Result<Vec<f64>, XasError> {
        let size = sites.len();
        let mut matrix = vec![vec![0.0; size]; size];
        for (row, site) in matrix.iter_mut().zip(sites) {
            let (first, basis) = basis_row(&self.knots, *site);
            row[first..first + basis.len()].copy_from_slice(&basis);
        }
        solve_linear(&matrix, values)
            .ok_or_else(|| XasError::FitFailed("singular background spline".into()))
    }

    pub(super) fn eval(&self, coefficients: &[f64]) -> Vec<f64> {
        self.rows
            .iter()
            .map(|(first, basis)| {
                basis
                    .iter()
                    .zip(&coefficients[*first..])
                    .map(|(basis, coefficient)| basis * coefficient)
                    .sum()
            })
            .collect()
    }
}

/// Cox-de Boor recursion on the span holding `x`, or the end span outside the
/// knots so the end polynomial continues.
fn basis_row(knots: &[f64], x: f64) -> (usize, [f64; 4]) {
    let last_span = knots.len() - ORDER - 2;
    let span = knots[..=last_span]
        .partition_point(|knot| *knot <= x)
        .clamp(ORDER + 1, last_span + 1)
        - 1;
    let mut values = [0.0; 4];
    values[0] = 1.0;
    let mut left = [0.0; 4];
    let mut right = [0.0; 4];
    for degree in 1..=ORDER {
        left[degree] = x - knots[span + 1 - degree];
        right[degree] = knots[span + degree] - x;
        let mut saved = 0.0;
        for index in 0..degree {
            let term = values[index] / (right[index + 1] + left[degree - index]);
            values[index] = saved + right[index + 1] * term;
            saved = left[degree - index] * term;
        }
        values[degree] = saved;
    }
    (span - ORDER, values)
}

/// Evaluates the not-a-knot cubic spline through `(x, y)` at `at`.
pub(super) fn interpolate_not_a_knot(x: &[f64], y: &[f64], at: &[f64]) -> Vec<f64> {
    let curvature = second_derivatives(x, y);
    at.iter()
        .map(|&position| {
            let right = x
                .partition_point(|value| *value <= position)
                .clamp(1, x.len() - 1);
            let left = right - 1;
            let width = x[right] - x[left];
            let to_right = (x[right] - position) / width;
            let to_left = (position - x[left]) / width;
            to_right * y[left]
                + to_left * y[right]
                + ((to_right.powi(3) - to_right) * curvature[left]
                    + (to_left.powi(3) - to_left) * curvature[right])
                    * width
                    * width
                    / 6.0
        })
        .collect()
}

/// Solves the tridiagonal continuity system. The not-a-knot rows give each end
/// curvature from its two neighbours, which folds them into the first and last
/// interior equations. `x` needs at least four points.
fn second_derivatives(x: &[f64], y: &[f64]) -> Vec<f64> {
    let count = x.len();
    let widths: Vec<f64> = x.windows(2).map(|pair| pair[1] - pair[0]).collect();
    let slopes: Vec<f64> = y
        .windows(2)
        .zip(&widths)
        .map(|(pair, width)| (pair[1] - pair[0]) / width)
        .collect();
    let interior = count - 2;
    let mut lower: Vec<f64> = widths[..interior].to_vec();
    let mut diagonal: Vec<f64> = (0..interior)
        .map(|row| 2.0 * (widths[row] + widths[row + 1]))
        .collect();
    let mut upper: Vec<f64> = widths[1..].to_vec();
    let mut rhs: Vec<f64> = (0..interior)
        .map(|row| 6.0 * (slopes[row + 1] - slopes[row]))
        .collect();

    let (h0, h1) = (widths[0], widths[1]);
    diagonal[0] += h0 * (h0 + h1) / h1;
    upper[0] -= h0 * h0 / h1;
    let (hn, hm) = (widths[count - 2], widths[count - 3]);
    diagonal[interior - 1] += hn * (hn + hm) / hm;
    lower[interior - 1] -= hn * hn / hm;

    for row in 1..interior {
        let factor = lower[row] / diagonal[row - 1];
        diagonal[row] -= factor * upper[row - 1];
        rhs[row] -= factor * rhs[row - 1];
    }
    let mut curvature = vec![0.0; count];
    curvature[interior] = rhs[interior - 1] / diagonal[interior - 1];
    for row in (0..interior - 1).rev() {
        curvature[row + 1] = (rhs[row] - upper[row] * curvature[row + 2]) / diagonal[row];
    }
    curvature[0] = ((h0 + h1) * curvature[1] - h0 * curvature[2]) / h1;
    curvature[count - 1] = ((hn + hm) * curvature[count - 2] - hn * curvature[count - 3]) / hm;
    curvature
}
