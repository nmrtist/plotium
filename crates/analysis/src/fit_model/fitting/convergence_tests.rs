use super::*;
use crate::fit_model::{ParameterDefinition, VariableDefinition};

fn constant(initial: f64, observed: Vec<f64>) -> (FitModelDefinition, FitDataset) {
    let mut model =
        FitModelDefinition::explicit("12345678-1234-4234-8234-123456789abc", "Constant", "y = a");
    model.responses = vec![VariableDefinition::new("y")];
    model.parameters = vec![ParameterDefinition::free("a", initial)];
    let data = FitDataset {
        id: "original".into(),
        inputs: BTreeMap::new(),
        responses: BTreeMap::from([("y".into(), observed)]),
        sigmas: BTreeMap::new(),
        constants: BTreeMap::new(),
    };
    (model, data)
}

fn assert_selected_status(result: &FitResult) {
    let selected = result
        .attempts
        .iter()
        .filter(|a| a.cost.is_finite())
        .reduce(|best, next| if next.cost < best.cost { next } else { best })
        .unwrap();
    assert_eq!(
        result.converged, selected.converged,
        "{:?}",
        result.attempts
    );
    assert_eq!(result.iterations, selected.iterations);
    assert_eq!(result.statistics.chi_squared, selected.cost);
}

#[test]
fn exact_initial_solution_converges_without_rejected_steps() {
    let (model, data) = constant(2., vec![2.; 8]);
    let result = fit_model(model, vec![data], &[], FitOptions::default()).unwrap();
    assert!(result.converged);
    assert_eq!(result.statistics.chi_squared, 0.);
    assert_eq!(result.parameters[0].standard_error, 0.);
    assert!(result.iterations < 2);
    assert_selected_status(&result);
}

#[test]
fn nonzero_residual_stationary_solution_converges() {
    let (model, data) = constant(2., vec![1., 3., 1., 3.]);
    let result = fit_model(model, vec![data], &[], FitOptions::default()).unwrap();
    assert!(result.converged, "{:?}", result.attempts);
    assert_eq!(result.parameters[0].value, 2.);
    assert_eq!(result.statistics.chi_squared, 4.);
    assert!((result.parameters[0].standard_error - (1_f64 / 3.).sqrt()).abs() < 1e-9);
    assert_selected_status(&result);
}

#[test]
fn nearly_exact_fit_reaches_observation_precision() {
    let (model, data) = constant(1., vec![2. + 1e-14, 2. - 1e-14, 2., 2.]);
    let result = fit_model(model, vec![data], &[], FitOptions::default()).unwrap();
    assert!(result.converged, "{:?}", result.attempts);
    assert!((result.parameters[0].value - 2.).abs() < 1e-14);
    assert!(result.statistics.chi_squared < 1e-26);
    assert_selected_status(&result);
}

#[test]
fn iteration_limit_is_not_reported_as_convergence() {
    let (model, data) = constant(0., vec![10.; 8]);
    let result = fit_model(
        model,
        vec![data],
        &[],
        FitOptions {
            max_iterations: 1,
            multi_start: 4,
            ..FitOptions::default()
        },
    )
    .unwrap();
    assert!(!result.converged);
    assert!(
        result
            .attempts
            .iter()
            .all(|a| !a.converged && a.reason == "iteration limit reached")
    );
    assert_selected_status(&result);
    assert!(
        result
            .notices
            .iter()
            .any(|n| n.contains("did not converge"))
    );
}

#[test]
fn multistart_status_follows_lowest_cost_even_with_failed_attempts() {
    let (model, data) = constant(2., vec![1., 3., 1., 3.]);
    let result = fit_model(
        model,
        vec![data],
        &[],
        FitOptions {
            max_iterations: 1,
            multi_start: 4,
            ..FitOptions::default()
        },
    )
    .unwrap();
    assert!(result.attempts.iter().any(|a| a.converged));
    assert!(result.attempts.iter().any(|a| !a.converged));
    assert!(result.converged);
    assert_selected_status(&result);
}

#[test]
fn bound_optimum_converges() {
    let (mut model, data) = constant(1., vec![3.; 4]);
    model.parameters[0].upper_bound = Some(1.);
    let result = fit_model(model, vec![data], &[], FitOptions::default()).unwrap();
    assert!(result.converged, "{:?}", result.attempts);
    assert_eq!(result.parameters[0].value, 1.);
    assert_selected_status(&result);
}

#[test]
fn final_accepted_step_can_converge_at_either_bound() {
    for (observed, bound) in [(3., 1.), (-3., -1.)] {
        let (mut model, data) = constant(0., vec![observed; 4]);
        if bound > 0. {
            model.parameters[0].upper_bound = Some(bound);
        } else {
            model.parameters[0].lower_bound = Some(bound);
        }
        let result = fit_model(
            model,
            vec![data],
            &[],
            FitOptions {
                max_iterations: 1,
                ..FitOptions::default()
            },
        )
        .unwrap();
        assert_eq!(result.parameters[0].value, bound);
        assert!(result.converged, "{:?}", result.attempts);
        assert_eq!(result.iterations, 1);
        assert_eq!(
            result.attempts[0].reason,
            "projected gradient tolerance reached"
        );
        assert_selected_status(&result);
    }
}

#[test]
fn small_damped_improvement_does_not_imply_convergence() {
    let (mut model, data) = constant(0., vec![1.; 4]);
    // Damping dominates this small Jacobian, so the first improvement is below
    // tolerance while the solution remains far from the least-squares optimum.
    model.kind = FitModelKind::Explicit {
        source: "y = 1e-9*a".into(),
    };
    let result = fit_model(
        model,
        vec![data],
        &[],
        FitOptions {
            tolerance: 0.01,
            max_iterations: 1,
            ..FitOptions::default()
        },
    )
    .unwrap();
    assert!(!result.converged, "{:?}", result.attempts);
    assert!(result.statistics.chi_squared > 3.);
    assert_selected_status(&result);
}
