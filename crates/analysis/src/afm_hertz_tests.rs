use super::*;

const INPUT: HertzInput = HertzInput {
    spring_constant_n_per_m: 2.0,
    radius_nm: 400.0,
    baseline_z_nm: [106.0, 125.0],
    contact_z_nm: 100.0,
    fit_z_nm: [12.5, 95.0],
};

fn baseline(z: f64) -> f64 {
    0.03125 * z - 2.0
}

fn reference_curve() -> Vec<[f64; 2]> {
    let mut points = [125.0, 119.0, 112.0, 106.0, 101.0, 100.0, 99.0]
        .into_iter()
        .map(|z| [z, baseline(z)])
        .collect::<Vec<_>>();
    // Independent closed-form reference: E*=37.5 MPa and R=400 nm give
    // 1 nN / nm^(3/2), so F is [8, 27, 64, 125] nN at these depths.
    points.extend(
        [92.0, 77.5, 52.0, 12.5]
            .into_iter()
            .zip([8.0, 27.0, 64.0, 125.0])
            .map(|(z, force)| [z, baseline(z) + force]),
    );
    points
}

#[test]
fn recovers_reduced_modulus_and_indentation_from_raw_z_and_force() {
    let result = fit_hertz(&reference_curve(), INPUT).unwrap();
    assert!((result.reduced_modulus_mpa - 37.5).abs() < 1.0e-10);
    assert_eq!(result.baseline_points, 4);
    assert!(result.baseline_rms_nn < 1.0e-12);
    assert_eq!(result.baseline_slope_nn_per_nm, 0.03125);
    assert_eq!(result.baseline_force_at_contact_nn, 1.125);
    assert!(result.residual_rms_nn < 1.0e-10);
    assert_eq!(result.r_squared, Some(1.0));
    assert!((result.max_indentation_over_radius - 0.0625).abs() < 1.0e-12);
    for (row, depth) in result.rows.iter().zip([4.0, 9.0, 16.0, 25.0]) {
        assert!((row.indentation_nm - depth).abs() < 1.0e-12);
        assert!(row.residual_nn().abs() < 1.0e-10);
    }
    assert_eq!(result.rows.len(), 4);

    let mut reversed = reference_curve();
    reversed.reverse();
    let reversed = fit_hertz(&reversed, INPUT).unwrap();
    assert_eq!(reversed.rows.first(), result.rows.last());
    assert_eq!(reversed.rows.last(), result.rows.first());
}

#[test]
fn contact_perturbation_keeps_the_windows_fixed_and_can_be_unavailable() {
    let points = reference_curve();
    let result = fit_hertz(&points, INPUT).unwrap();
    let sensitivity = result.contact_sensitivity.unwrap();
    assert_eq!(sensitivity.shift_nm, 1.0);
    assert!(sensitivity.lower_contact_modulus_mpa.unwrap() > result.reduced_modulus_mpa);
    assert!(sensitivity.higher_contact_modulus_mpa.unwrap() < result.reduced_modulus_mpa);

    let narrow = HertzInput {
        baseline_z_nm: [100.5, 125.0],
        ..INPUT
    };
    let result = fit_hertz(&points, narrow).unwrap();
    let sensitivity = result.contact_sensitivity.unwrap();
    assert!(sensitivity.lower_contact_modulus_mpa.is_some());
    assert!(sensitivity.higher_contact_modulus_mpa.is_none());
}

#[test]
fn rejects_invalid_windows_and_calibration() {
    let points = reference_curve();
    for (input, expected) in [
        (
            HertzInput {
                radius_nm: 0.0,
                ..INPUT
            },
            HertzError::Radius,
        ),
        (
            HertzInput {
                spring_constant_n_per_m: f64::NAN,
                ..INPUT
            },
            HertzError::SpringConstant,
        ),
        (
            HertzInput {
                contact_z_nm: 106.0,
                ..INPUT
            },
            HertzError::IntervalOrder,
        ),
        (
            HertzInput {
                fit_z_nm: [0.0, 95.0],
                ..INPUT
            },
            HertzError::OutsideCurve,
        ),
        (
            HertzInput {
                fit_z_nm: [77.0, 95.0],
                ..INPUT
            },
            HertzError::FitSamples,
        ),
        (
            HertzInput {
                baseline_z_nm: [118.0, 125.0],
                ..INPUT
            },
            HertzError::BaselineSamples,
        ),
        (
            HertzInput {
                fit_z_nm: [95.0, 12.5],
                ..INPUT
            },
            HertzError::Intervals,
        ),
    ] {
        assert_eq!(fit_hertz(&points, input), Err(expected));
    }
}

#[test]
fn rejects_a_baseline_with_identical_nonexact_z_values() {
    let points = [
        [0.1, 0.0],
        [0.1, 0.0],
        [0.1, 0.0],
        [-1.0, 0.2],
        [-2.0, 0.5],
        [-3.0, 1.0],
    ];
    let input = HertzInput {
        baseline_z_nm: [0.05, 0.1],
        contact_z_nm: 0.0,
        fit_z_nm: [-3.0, -1.0],
        ..INPUT
    };
    assert_eq!(fit_hertz(&points, input), Err(HertzError::BaselineSamples));
}

#[test]
fn rejects_one_noncompressive_or_nonpositive_sample_without_dropping_rows() {
    let mut points = reference_curve();
    points.push([70.0, baseline(70.0) - 1.0]);
    assert_eq!(
        fit_hertz(&points, INPUT),
        Err(HertzError::NonCompressiveForce)
    );
    *points.last_mut().unwrap() = [70.0, baseline(70.0) + 60.0];
    assert_eq!(
        fit_hertz(&points, INPUT),
        Err(HertzError::NonpositiveIndentation)
    );
    *points.last_mut().unwrap() = [70.0, f64::NAN];
    assert_eq!(fit_hertz(&points, INPUT), Err(HertzError::Samples));
}
