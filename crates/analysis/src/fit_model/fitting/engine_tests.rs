use crate::fit_model::{FitDataset, FitOptions, fit_model};
use crate::models::{DIFFUSION_B_ID, builtin_model};
use std::collections::BTreeMap;

#[test]
fn noiseless_and_nearly_noiseless_diffusion_fits_converge_at_physical_scale() {
    for noise in [0., 1e-11] {
        let b: Vec<f64> = (0..14).map(|i| i as f64 * 1e8).collect();
        let observed = b
            .iter()
            .enumerate()
            .map(|(i, b)| 1000. * (-1.2e-9 * b).exp() + if i % 2 == 0 { noise } else { -noise })
            .collect();
        let data = FitDataset {
            id: "original-decay".into(),
            inputs: BTreeMap::from([("b".into(), b)]),
            responses: BTreeMap::from([("y".into(), observed)]),
            sigmas: BTreeMap::new(),
            constants: BTreeMap::new(),
        };
        let result = fit_model(
            builtin_model(DIFFUSION_B_ID).unwrap(),
            vec![data],
            &[],
            FitOptions::default(),
        )
        .unwrap();
        assert!(result.converged, "noise={noise}: {:?}", result.attempts);
        assert!(result.attempts[0].converged);
        let d = result
            .parameters
            .iter()
            .find(|p| p.parameter == "D")
            .unwrap();
        assert!((d.value / 1.2e-9 - 1.).abs() < 1e-10);
        assert!(result.statistics.chi_squared < 2e-20);
        assert!(d.standard_error.is_finite() && d.standard_error < 1e-21);
    }
}

#[test]
fn small_diffusion_parameter_matches_independent_least_squares_and_uncertainty() {
    // Post-FFT observations from the original synthetic acquisition. Keeping
    // these rounded values reproduces the Linux/macOS trust-region collapse.
    let observed = vec![
        1000.0,
        886.9224367171574,
        786.6318610665535,
        697.682326071031,
        618.7913918061408,
        548.8116360940264,
        486.7542559599717,
        431.71452342907975,
        382.898885975112,
        339.6035256449391,
        301.19421191220215,
        267.1373019658504,
        236.93175868212177,
        210.14207120076472,
    ];
    let b: Vec<f64> = (0..14).map(|i| i as f64 * 1e8).collect();
    let data = FitDataset {
        id: "synthetic".into(),
        inputs: BTreeMap::from([("b".into(), b.clone())]),
        responses: BTreeMap::from([("y".into(), observed.clone())]),
        sigmas: BTreeMap::new(),
        constants: BTreeMap::new(),
    };
    let result = fit_model(
        builtin_model(DIFFUSION_B_ID).unwrap(),
        vec![data],
        &[],
        FitOptions::default(),
    )
    .unwrap();
    assert!(result.attempts[0].converged, "{:?}", result.attempts);

    // Eliminate I0 analytically, then independently minimize the scalar SSE
    // with golden-section search, without the fitter's numerical Jacobian.
    let objective = |d: f64| {
        let decay: Vec<_> = b.iter().map(|b| (-d * b).exp()).collect();
        let i0 = observed.iter().zip(&decay).map(|(y, e)| y * e).sum::<f64>()
            / decay.iter().map(|e| e * e).sum::<f64>();
        let sse = observed
            .iter()
            .zip(&decay)
            .map(|(y, e)| (y - i0 * e).powi(2))
            .sum::<f64>();
        (sse, i0)
    };
    let (mut lo, mut hi) = (0.5e-9, 2e-9);
    let ratio = (5_f64.sqrt() - 1.) / 2.;
    for _ in 0..100 {
        let left = hi - ratio * (hi - lo);
        let right = lo + ratio * (hi - lo);
        if objective(left).0 < objective(right).0 {
            hi = right;
        } else {
            lo = left;
        }
    }
    let expected_d = (lo + hi) / 2.;
    let (expected_sse, i0) = objective(expected_d);
    let d = result
        .parameters
        .iter()
        .find(|p| p.parameter == "D")
        .unwrap();
    assert!((d.value / expected_d - 1.).abs() < 1e-7);
    assert!((result.statistics.chi_squared / expected_sse - 1.).abs() < 1e-8);

    // Covariance = residual variance * (JᵀJ)⁻¹, using analytic derivatives
    // of I0*exp(-D*b), independently of the production covariance path.
    let (mut aa, mut ad, mut dd) = (0., 0., 0.);
    for b in b {
        let e = (-expected_d * b).exp();
        let derivative = -i0 * b * e;
        aa += e * e;
        ad += e * derivative;
        dd += derivative * derivative;
    }
    let expected_se = (expected_sse / 12. * aa / (aa * dd - ad * ad)).sqrt();
    assert!((d.standard_error / expected_se - 1.).abs() < 1e-5);
}
