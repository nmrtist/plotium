use super::*;

#[test]
fn phase_pivot_uses_sample_positions_and_preserves_the_complex_rotation() {
    for step in [0.125, -0.125] {
        let scale = PhasePivotScale::new(8, 4.0, step).unwrap();
        assert_eq!(scale.ppm(7.0 / 8.0).unwrap(), 4.0 + 7.0 * step);
        assert_eq!(scale.ppm(1.0).unwrap(), 4.0 + 8.0 * step);
        let before = VolumePhase {
            p0_degrees: 23.0,
            p1_degrees: -137.0,
            pivot_fraction: 0.31,
        };
        for q in [0.0, 0.17, 7.0 / 8.0, 1.0] {
            let mut after = before;
            let recovered = scale.fraction(scale.ppm(q).unwrap()).unwrap();
            assert!((recovered - q).abs() < 1e-14);
            after.repivot(recovered).unwrap();
            for i in 0..8 {
                let angle = |p: VolumePhase| {
                    p.p0_degrees + p.p1_degrees * (i as f64 / 8.0 - p.pivot_fraction)
                };
                let rotate = |p| {
                    nmr::Complex64::new(0.37, -1.4)
                        * nmr::Complex64::from_polar(1.0, angle(p).to_radians())
                };
                assert!((angle(before) - angle(after)).abs() < 1e-12);
                assert!((rotate(before) - rotate(after)).norm() < 1e-12);
            }
        }
    }
}

#[test]
fn phase_pivot_rejects_unusable_grids_and_out_of_range_values() {
    assert!(PhasePivotScale::new(1, 4.0, 0.1).is_err());
    assert!(PhasePivotScale::new(8, 4.0, 0.0).is_err());
    assert!(PhasePivotScale::new(8, f64::INFINITY, 0.1).is_err());
    let scale = PhasePivotScale::new(8, 4.0, -0.125).unwrap();
    for ppm in [f64::NAN, f64::INFINITY, 2.99, 4.01] {
        assert!(scale.fraction(ppm).is_err());
    }
    let mut phase = VolumePhase::default();
    for fraction in [f64::NAN, -0.1, 1.1] {
        assert!(scale.ppm(fraction).is_err());
        assert!(phase.repivot(fraction).is_err());
        assert_eq!(phase, VolumePhase::default());
    }
}
