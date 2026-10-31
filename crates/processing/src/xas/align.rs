use super::normalize::{gradient, interpolate};

/// How far a scan may sit from another and still be the same edge, in eV.
pub const MAX_ALIGNMENT_SHIFT: f64 = 20.0;
/// Half-width of the comparison around the edge, in eV.
const EDGE_WINDOW: f64 = 30.0;
const GRID_STEP: f64 = 0.1;
const SEARCH_STEP: f64 = 0.25;
const SHIFT_TOLERANCE: f64 = 1e-3;
const GOLDEN: f64 = 0.618_033_988_749_894_9;

fn derivative(energy: &[f64], mu: &[f64]) -> Vec<f64> {
    gradient(mu)
        .iter()
        .zip(gradient(energy))
        .map(|(rise, run)| rise / run)
        .collect()
}

/// The energy to add to `moving` so its edge lies on `fixed`'s, to a fraction of
/// the data step. The derivatives are compared around `center` with a free
/// scale, because two foils rarely have the same absorption.
pub fn alignment_shift(
    fixed: (&[f64], &[f64]),
    moving: (&[f64], &[f64]),
    center: f64,
) -> Option<f64> {
    let count = (2.0 * EDGE_WINDOW / GRID_STEP) as usize;
    let grid: Vec<f64> = (0..=count)
        .map(|index| center - EDGE_WINDOW + index as f64 * GRID_STEP)
        .collect();
    let target = interpolate(fixed.0, &derivative(fixed.0, fixed.1), &grid);
    let slope = derivative(moving.0, moving.1);
    let cost = |shift: f64| {
        let at: Vec<f64> = grid.iter().map(|energy| energy - shift).collect();
        let values = interpolate(moving.0, &slope, &at);
        let cross: f64 = target.iter().zip(&values).map(|(a, b)| a * b).sum();
        let power: f64 = values.iter().map(|value| value * value).sum();
        let scale = if power > 0.0 { cross / power } else { 0.0 };
        target
            .iter()
            .zip(&values)
            .map(|(a, b)| (a - scale * b).powi(2))
            .sum::<f64>()
    };
    let steps = (MAX_ALIGNMENT_SHIFT / SEARCH_STEP) as i32;
    let best = (-steps..=steps)
        .map(|step| f64::from(step) * SEARCH_STEP)
        .min_by(|left, right| cost(*left).total_cmp(&cost(*right)))?;
    let (mut low, mut high) = (best - SEARCH_STEP, best + SEARCH_STEP);
    while high - low > SHIFT_TOLERANCE {
        let left = high - GOLDEN * (high - low);
        let right = low + GOLDEN * (high - low);
        if cost(left) < cost(right) {
            high = right;
        } else {
            low = left;
        }
    }
    let shift = 0.5 * (low + high);
    shift.is_finite().then_some(shift)
}
