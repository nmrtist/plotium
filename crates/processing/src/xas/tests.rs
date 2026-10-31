use super::spline::{BackgroundBasis, interpolate_not_a_knot};
use super::*;

fn close(left: f64, right: f64, tolerance: f64) {
    assert!(
        (left - right).abs() <= tolerance,
        "{left} vs {right} (tolerance {tolerance})"
    );
}

#[test]
fn builds_mu_for_each_geometry() {
    let i0 = [100.0, 100.0];
    let it = [50.0, 25.0];
    let transmission = transmission_mu(&i0, &it).unwrap();
    close(transmission[0], 2.0_f64.ln(), 1e-12);
    close(transmission[1], 4.0_f64.ln(), 1e-12);
    assert_eq!(fluorescence_mu(&it, &i0).unwrap(), vec![0.5, 0.25]);
    close(
        reference_mu(&it, &[25.0, 25.0]).unwrap()[0],
        2.0_f64.ln(),
        1e-12,
    );
    assert_eq!(
        transmission_mu(&i0, &[1.0]),
        Err(XasError::LengthMismatch(2, 1))
    );
}

#[test]
fn prepares_sorted_strictly_increasing_axis() {
    let energy = [7002.0, 7000.0, 7001.0, 7001.0, f64::NAN, 7003.0];
    let mu = [3.0, 1.0, 2.0, 2.5, 9.0, f64::INFINITY];
    let (energy, mu) = prepare_spectrum(&energy, &mu).unwrap();
    assert_eq!(mu, vec![1.0, 2.0, 3.0]);
    assert_eq!(energy, vec![7000.0, 7001.0, 7002.0]);

    let (energy, _) = prepare_spectrum(&[7001.0, 7001.0, 7001.0, 7001.001], &[0.0; 4]).unwrap();
    assert!(energy.windows(2).all(|pair| pair[1] > pair[0]));
}

#[test]
fn converts_energy_to_signed_k() {
    let k = energy_to_k(&[7100.0, 7112.0, 7212.0], 7112.0);
    close(k[0], -(12.0 * ETOK).sqrt(), 1e-12);
    assert_eq!(k[1], 0.0);
    close(k[2], 5.123_167, 1e-5);
    assert_eq!(k_weighted(&[2.0], &[0.5], 3).unwrap(), vec![4.0]);
}

#[test]
fn search_indices_match_their_definitions() {
    let values = [1.0, 2.0, 4.0];
    assert_eq!(index_of(&values, 0.5), 0);
    assert_eq!(index_of(&values, 2.0), 1);
    assert_eq!(index_of(&values, 9.0), 2);
    assert_eq!(index_nearest(&values, 2.9), 1);
    assert_eq!(index_nearest(&values, 3.1), 2);
    assert_eq!(index_nearest(&values, 9.0), 2);
}

#[test]
fn splines_reproduce_a_cubic_including_outside_the_sites() {
    let cubic = |x: f64| 1.0 - 2.0 * x + 0.5 * x * x + 0.25 * x * x * x;
    let sites = [0.0, 0.4, 1.3, 2.0, 3.5, 4.0, 5.2];
    let values: Vec<f64> = sites.iter().map(|x| cubic(*x)).collect();
    let points = [-0.3, 0.0, 0.7, 2.0, 5.2, 5.5];
    let basis = BackgroundBasis::new(&sites, &points);
    let coefficients = basis.interpolate(&sites, &values).unwrap();
    for (point, fitted) in points.iter().zip(basis.eval(&coefficients)) {
        close(fitted, cubic(*point), 1e-10);
    }
    for (point, value) in points
        .iter()
        .zip(interpolate_not_a_knot(&sites, &values, &points))
    {
        close(value, cubic(*point), 1e-10);
    }
}

#[test]
fn window_is_flat_between_tapers() {
    let x: Vec<f64> = (0..400).map(|index| 0.05 * index as f64).collect();
    let window = ft_window(&x, 3.0, 12.0, 1.0, 1.0, XafsWindow::Hanning);
    assert_eq!(window[0], 0.0);
    assert_eq!(window[150], 1.0);
    assert_eq!(window[399], 0.0);
}

#[test]
fn transform_of_a_single_shell_peaks_near_its_distance() {
    let k: Vec<f64> = (0..300).map(|index| 0.05 * index as f64).collect();
    let chi: Vec<f64> = k.iter().map(|k| (2.0 * k * 2.2).sin()).collect();
    let chi_r = xftf(
        &k,
        &chi,
        &XftfParams {
            kmin: 2.0,
            kmax: 13.0,
            kweight: 0,
            window: XafsWindow::Hanning,
            ..XftfParams::default()
        },
    )
    .unwrap();
    let peak = (0..chi_r.r.len())
        .max_by(|a, b| chi_r.magnitude[*a].total_cmp(&chi_r.magnitude[*b]))
        .unwrap();
    close(chi_r.r[peak], 2.2, 0.03);
}

#[test]
fn rejects_unusable_input() {
    let energy: Vec<f64> = (0..20).map(|index| 7000.0 + 10.0 * index as f64).collect();
    let mu = vec![1.0; 20];
    assert!(pre_edge(&energy[..5], &mu[..5], &PreEdgeParams::default()).is_err());
    assert!(autobk(&energy, &mu, 6000.0, 1.0, &AutobkParams::default()).is_err());
    let params = XftfParams {
        nfft: 4,
        ..XftfParams::default()
    };
    assert!(xftf(&[0.0, 1.0], &[0.0, 0.0], &params).is_err());
}

#[test]
fn window_tolerates_a_range_past_the_grid() {
    let x: Vec<f64> = (0..20).map(|index| 0.05 * index as f64).collect();
    let window = ft_window(&x, 30.0, 40.0, 1.0, 1.0, XafsWindow::Hanning);
    assert_eq!(window.len(), 20);
    assert!(window.iter().all(|value| value.is_finite()));
}

#[test]
fn rejects_a_transform_longer_than_nfft() {
    let params = XftfParams {
        kmax: 1e12,
        ..XftfParams::default()
    };
    assert!(matches!(
        xftf(&[0.0, 1.0], &[0.0, 0.0], &params),
        Err(XasError::InvalidParameter(_))
    ));
}

#[test]
fn interpolation_holds_the_end_values() {
    let values = normalize::interpolate(&[2.0, 3.0], &[1.0, 5.0], &[1.5, 2.5, 4.0]);
    assert_eq!(values, vec![1.0, 3.0, 5.0]);
}

#[test]
fn a_short_chi_transforms_with_a_small_nfft() {
    let k: Vec<f64> = (0..200).map(|index| 0.05 * index as f64).collect();
    let params = XftfParams {
        nfft: 256,
        ..XftfParams::default()
    };
    assert_eq!(xftf(&k, &vec![0.0; 200], &params).unwrap().re.len(), 41);
}

#[test]
fn extreme_parameters_fail_without_panicking() {
    let energy: Vec<f64> = (0..200).map(|index| 7000.0 + 5.0 * index as f64).collect();
    let mu: Vec<f64> = energy.iter().map(|e| (e - 7400.0).atan()).collect();
    for params in [
        AutobkParams {
            nfft: 1 << 48,
            ..AutobkParams::default()
        },
        AutobkParams {
            rbkg: f64::NAN,
            ..AutobkParams::default()
        },
    ] {
        assert!(autobk(&energy, &mu, 7400.0, 1.0, &params).is_err());
    }
    let params = AutobkParams {
        rbkg: 1e30,
        ..AutobkParams::default()
    };
    assert!(matches!(
        autobk(&energy, &mu, 7400.0, 1.0, &params),
        Err(XasError::TooFewPoints(_))
    ));

    let unsorted = [1.0, 2.0, 2.0, 3.0];
    assert!(pre_edge(&unsorted, &unsorted, &PreEdgeParams::default()).is_err());
    let params = XftfParams {
        nfft: 1 << 48,
        ..XftfParams::default()
    };
    assert!(xftf(&[0.0, 1.0], &[0.0, 0.0], &params).is_err());
}

#[test]
fn athena_preset_stops_the_transform_before_the_data_ends() {
    let current = XasProcessing {
        energy_shift: 1.5,
        ..XasProcessing::default()
    };
    let params = XasPreset::Athena.apply(&current, Some(14.0));
    assert_eq!(params.energy_shift, 1.5);
    assert_eq!((params.transform.kmin, params.transform.kmax), (3.0, 12.0));
    close(params.pre_edge.norm2.unwrap(), KTOE * 196.0 - 100.0, 1e-9);
    assert_eq!(params.transform.window, XafsWindow::Hanning);
    assert_eq!(XasPreset::Larch.apply(&params, None), current);
    assert!(params.validate().is_ok());
}

#[test]
fn invalid_ranges_are_rejected() {
    let mut params = XasProcessing::default();
    params.transform.kmin = 15.0;
    params.transform.kmax = 5.0;
    assert!(params.validate().is_err());
}

#[test]
fn edges_come_from_the_table() {
    assert_eq!(edge_energy("fe", "k"), Some(7112.0));
    assert_eq!(nearest_edge(12280.0).map(|edge| edge.0), Some("Hg"));
}
