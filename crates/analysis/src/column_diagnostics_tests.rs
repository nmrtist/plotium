use crate::{
    SpectrumStack,
    diffusion::diffusion_map_b_progress,
    ilt::{ilt_map, log_grid},
};
use num_complex::Complex64;
use std::cell::RefCell;
struct Stack {
    ppm: Vec<f64>,
    traces: Vec<Vec<Complex64>>,
}
impl SpectrumStack for Stack {
    fn coordinates(&self) -> &[f64] {
        &self.ppm
    }
    fn traces(&self) -> &[Vec<Complex64>] {
        &self.traces
    }
}
#[test]
fn native_column_diagnostics_explain_acceptance_rejection_and_progress() {
    let b = (0..8).map(|i| i as f64 * 4e8).collect::<Vec<_>>();
    let stack = Stack {
        ppm: vec![1., 2., 3., 4.],
        traces: b
            .iter()
            .map(|b| {
                vec![
                    100. * (-1.2e-9 * b).exp(),
                    0.,
                    (-1.2e-9 * b).exp(),
                    10. * (1.2e-9 * b).exp(),
                ]
                .into_iter()
                .map(|v| Complex64::new(v, 0.))
                .collect()
            })
            .collect(),
    };
    let events = RefCell::new(Vec::new());
    let result = diffusion_map_b_progress(&stack, &b, 0.05, &|| false, &|done, total| {
        events.borrow_mut().push((done, total))
    })
    .unwrap();
    assert!((result.d[0] - 1.2e-9).abs() < 1e-11);
    let first = &result.diagnostics[0];
    assert!(first.accepted);
    assert_eq!(first.effective_points, 8);
    assert!(first.r_squared.unwrap() > 0.999);
    assert!(first.residual_rmse.unwrap() < 1e-6);
    assert_eq!(result.diagnostics[1].reason.as_deref(), Some("low_signal"));
    assert_eq!(result.diagnostics[2].reason.as_deref(), Some("low_signal"));
    assert_eq!(
        result.diagnostics[3].reason.as_deref(),
        Some("degenerate_or_nonpositive_d")
    );
    assert_eq!(events.borrow().first(), Some(&(0, 4)));
    assert_eq!(events.borrow().last(), Some(&(4, 4)));
}
#[test]
fn ilt_records_reconstruction_and_termination_without_mono_exp_uncertainty() {
    let b = (0..16).map(|i| i as f64 * 2e9 / 15.).collect::<Vec<_>>();
    let stack = Stack {
        ppm: vec![1., 2.],
        traces: b
            .iter()
            .map(|b| {
                vec![
                    Complex64::new(100. * (-1e-9 * b).exp(), 0.),
                    Complex64::new(0., 0.),
                ]
            })
            .collect(),
    };
    let result = ilt_map(&stack, &b, &log_grid(1e-10, 1e-8, 21), 0.01);
    let diagnostic = &result.diagnostics[0];
    assert!(diagnostic.accepted);
    assert!(result.amp[0].iter().all(|v| *v >= 0.));
    assert!(diagnostic.residual_rmse.unwrap() < 2.);
    assert_eq!(
        diagnostic.solver_termination.as_deref(),
        Some("optimality_tolerance")
    );
    assert!(diagnostic.solver_iterations.unwrap() > 0);
    assert!(diagnostic.sigma_d.is_none());
    assert!(diagnostic.r_squared.is_none());
    assert_eq!(result.diagnostics[1].reason.as_deref(), Some("low_signal"));
}
