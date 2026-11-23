use super::*;
use crate::afm_hertz::fit_hertz;

const K: f64 = 20.0;
const RADIUS: f64 = 400.0;
const CONTACT: f64 = 100.0;

fn baseline(z: f64) -> f64 {
    1.25 + 0.002 * z
}

fn reference_curve() -> Vec<[f64; 2]> {
    let mut points: Vec<_> = (0..=60)
        .rev()
        .map(|i| {
            let z = CONTACT + f64::from(i);
            [z, baseline(z)]
        })
        .collect();
    points.extend((1..=30).map(|i| {
        // F = delta^(3/2) in nN corresponds to E* = 37.5 MPa at R = 400 nm.
        let delta = f64::from(i);
        let force = delta * delta.sqrt();
        let z = CONTACT - delta - force / K;
        [z, baseline(z) + force]
    }));
    points
}

#[test]
fn starts_at_a_known_noiseless_contact_and_yields_a_manual_hertz_window() {
    let mut points = reference_curve();
    let suggested = suggest_hertz(&points, K, RADIUS, 0.0).unwrap();
    assert_eq!(suggested.input.contact_z_nm, CONTACT);
    assert_eq!(suggested.input.baseline_z_nm, [143.0, 160.0]);
    assert_eq!(
        suggested.input.fit_z_nm,
        [points.last().unwrap()[0], points[61][0]]
    );
    assert!(suggested.baseline_rms_nn < 1e-12);
    assert!((suggested.onset_force_nn - 1.0).abs() < 1e-12);
    let fit = fit_hertz(&points, suggested.input).unwrap();
    assert_eq!(fit.rows.len(), 30);
    assert!((fit.reduced_modulus_mpa - 37.5).abs() < 1e-10);

    points.reverse();
    assert_eq!(suggest_hertz(&points, K, RADIUS, 0.0), Ok(suggested));
}

#[test]
fn tolerates_small_baseline_noise_and_caps_the_window_by_radius() {
    let mut points = reference_curve();
    for (i, point) in points.iter_mut().enumerate() {
        point[1] += [0.03, -0.015, -0.03, 0.015][i % 4];
    }
    let radius = 150.0;
    let suggested = suggest_hertz(&points, K, radius, 0.0).unwrap();
    assert!((suggested.input.contact_z_nm - CONTACT).abs() <= 1.1);
    assert!(suggested.baseline_rms_nn > 0.01);
    let fit = fit_hertz(&points, suggested.input).unwrap();
    assert!(fit.rows.len() < 30);
    assert!(fit.max_indentation_over_radius <= 0.1);
    assert!(fit.max_indentation_over_radius >= 0.08);
}

#[test]
fn does_not_propose_on_flat_force_noisy_force_or_insufficient_shallow_data() {
    let mut points = reference_curve();
    assert_eq!(
        suggest_hertz(&points, K, 1.0, 0.0),
        Err(SuggestionError::Window)
    );
    for [z, f] in &mut points {
        *f = baseline(*z);
    }
    assert_eq!(
        suggest_hertz(&points, K, RADIUS, 0.0),
        Err(SuggestionError::Onset)
    );
    for (i, [z, f]) in points.iter_mut().enumerate() {
        *f = baseline(*z) + if i % 2 == 0 { 1.0 } else { -1.0 };
    }
    assert_eq!(
        suggest_hertz(&points, K, RADIUS, 0.0),
        Err(SuggestionError::Onset)
    );
    assert_eq!(
        suggest_hertz(&points[..31], K, RADIUS, 0.0),
        Err(SuggestionError::Samples)
    );
    assert_eq!(
        suggest_hertz(&points, K, f64::NAN, 0.0),
        Err(SuggestionError::Calibration)
    );
}

#[test]
fn ignores_one_count_before_clear_loading() {
    let increment = 0.01;
    for sign in [-1.0, 1.0] {
        let mut points = reference_curve();
        for [z, force] in &mut points {
            *force = 2.0 + ((*force - baseline(*z)) / increment).round() * increment;
        }
        points[55][1] += sign * increment;
        let suggestion = suggest_hertz(&points, K, RADIUS, increment).unwrap();
        assert_eq!(suggestion.input.contact_z_nm, CONTACT);
        assert_eq!(suggestion.baseline_rms_nn, 0.0);
        assert!(suggestion.onset_force_nn > 5.0 * increment);
    }
}

#[test]
fn rejects_premature_spikes_and_adhesive_excursions() {
    for sign in [-1.0, 1.0] {
        let mut points = reference_curve();
        points[55][1] += sign;
        assert_eq!(
            suggest_hertz(&points, K, RADIUS, 0.01),
            Err(SuggestionError::Excursion)
        );
    }
}

#[test]
fn rejects_bad_ramps_and_unstable_baseline() {
    let mut points = reference_curve();
    points.swap(30, 31);
    assert_eq!(
        suggest_hertz(&points, K, RADIUS, 0.0),
        Err(SuggestionError::Ramp)
    );
    points.swap(30, 31);
    points.drain(30..34);
    assert_eq!(
        suggest_hertz(&points, K, RADIUS, 0.0),
        Err(SuggestionError::Ramp)
    );

    let mut points = reference_curve();
    points[4][1] += 1.0;
    assert_eq!(
        suggest_hertz(&points, K, RADIUS, 0.0),
        Err(SuggestionError::Baseline)
    );
    let mut points = reference_curve();
    for [z, f] in &mut points {
        *f += *z * K;
    }
    assert_eq!(
        suggest_hertz(&points, K, RADIUS, 0.0),
        Err(SuggestionError::Baseline)
    );
}
