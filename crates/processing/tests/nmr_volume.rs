use std::path::PathBuf;

use nmr::ExecutionContext;
use nmr::processing::ProcessingOptions;
use plotium_processing::nmr_volume::{Axis3d, NmrVolume, VolumePhase};
fn source(path: &std::path::Path) -> plotium_io::nmr_view::NmrSource {
    plotium_io::nmr_view::NmrSource::new(
        plotium_io::nmr_bridge::read(path, &mut ExecutionContext::default()).unwrap(),
    )
    .unwrap()
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../io/tests/fixtures/nmr/bruker-3d-qf")
}

#[test]
fn processes_unequal_axes_and_preserves_indexed_signed_peaks() {
    let volume = NmrVolume::process(
        &source(&fixture()),
        [VolumePhase::default(); 3],
        ProcessingOptions::default(),
        &mut ExecutionContext::default(),
    )
    .unwrap();
    assert_eq!(volume.shape(), [5, 3, 4]);
    assert!(volume.processed().provenance().history().is_some());
    for fixed in Axis3d::ALL {
        let plane = volume.slice(fixed, 1).unwrap();
        let expected: Vec<_> = Axis3d::ALL
            .into_iter()
            .filter(|axis| *axis != fixed)
            .collect();
        assert_eq!(plane.axes.as_slice(), expected);
        assert_eq!(plane.fixed_ppm, volume.ppm(fixed, 1).unwrap());
        assert!(volume.slice(fixed, volume.shape()[fixed as usize]).is_err());
        for (i, x) in plane.ppm[0].iter().enumerate() {
            assert_eq!(*x, volume.ppm(plane.axes[0], i).unwrap());
            for (j, y) in plane.ppm[1].iter().enumerate() {
                assert_eq!(*y, volume.ppm(plane.axes[1], j).unwrap());
                let mut indices = [0; 3];
                indices[fixed as usize] = 1;
                indices[plane.axes[0] as usize] = i;
                indices[plane.axes[1] as usize] = j;
                assert_eq!(
                    plane.signed_values[i * plane.ppm[1].len() + j],
                    volume.signed_value(indices).unwrap(),
                );
            }
        }
    }
    let mut strongest = ([0; 3], 0.0_f64);
    for a in 0..5 {
        for b in 0..3 {
            for c in 0..4 {
                let indices = [a, b, c];
                let intensity = volume.signed_value(indices).unwrap();
                if intensity.abs() > strongest.1.abs() {
                    strongest = (indices, intensity);
                }
            }
        }
    }
    assert!(strongest.1 < 0.0);
}

#[test]
fn rejects_two_dimensional_inputs_and_cancellation() {
    let two_dimensional = fixture().with_file_name("bruker-states");
    assert!(
        NmrVolume::process(
            &source(&two_dimensional),
            [VolumePhase::default(); 3],
            ProcessingOptions::default(),
            &mut ExecutionContext::default(),
        )
        .is_err()
    );
    let mut context = ExecutionContext::default();
    context.cancellation().cancel();
    assert!(
        NmrVolume::process(
            &source(&fixture()),
            [VolumePhase::default(); 3],
            ProcessingOptions::default(),
            &mut context,
        )
        .is_err()
    );
}

#[test]
fn retains_complex_lanes_and_phases_before_slice() {
    let input = source(&fixture());
    let original = NmrVolume::process_source(&input).unwrap();
    assert!(original.axes().iter().any(|a| a.component_count() > 1));
    let mut phases = [VolumePhase::default(); 3];
    phases[2].p0_degrees = 180.0;
    let inverted = NmrVolume::process(
        &input,
        phases,
        ProcessingOptions::default(),
        &mut ExecutionContext::default(),
    )
    .unwrap();
    for a in 0..5 {
        for b in 0..3 {
            for c in 0..4 {
                assert!(
                    (original.signed_value([a, b, c]).unwrap()
                        + inverted.signed_value([a, b, c]).unwrap())
                    .abs()
                        < 1e-8
                );
            }
        }
    }
    for axis in 0..3 {
        let plane = original.plane_source(axis, 1).unwrap();
        assert_eq!(plane.axes().len(), 2);
        let slice = original.slice(Axis3d::ALL[axis], 1).unwrap();
        let data = plane.dataset().as_processed().unwrap();
        for (i, value) in data.data().component_plane(&[0, 0]).unwrap().enumerate() {
            assert_eq!(*value, slice.signed_values[i]);
        }
    }
    assert!(original.nearest(0, f64::NAN).is_err());
    assert!(original.nearest(3, 0.0).is_err());
}

#[test]
fn signed_volume_matches_independent_direct_dft() {
    use std::f64::consts::TAU;
    let input = std::fs::read(fixture().join("ser")).unwrap();
    let integers: Vec<f64> = input
        .as_chunks::<4>()
        .0
        .iter()
        .map(|bytes| i32::from_le_bytes(*bytes) as f64)
        .collect();
    let volume = NmrVolume::process_source(&source(&fixture())).unwrap();
    let shape = volume.shape();
    for a in 0..shape[0] {
        for b in 0..shape[1] {
            for c in 0..shape[2] {
                let frequencies = [
                    a as isize - (shape[0] / 2) as isize,
                    b as isize - (shape[1] / 2) as isize,
                    c as isize - (shape[2] / 2) as isize,
                ];
                let mut expected = nmr::Complex64::new(0.0, 0.0);
                for i in 0..shape[0] {
                    for j in 0..shape[1] {
                        for k in 0..shape[2] {
                            let offset = ((i * shape[1] + j) * shape[2] + k) * 2;
                            let value = nmr::Complex64::new(integers[offset], integers[offset + 1]);
                            // QF indirect axes are scalar: each contributes its cosine
                            // transform independently to each direct Cartesian lane.
                            let angle = -TAU * frequencies[2] as f64 * k as f64 / shape[2] as f64;
                            let weight = (TAU * frequencies[0] as f64 * i as f64 / shape[0] as f64)
                                .cos()
                                * (TAU * frequencies[1] as f64 * j as f64 / shape[1] as f64).cos();
                            expected += value * nmr::Complex64::from_polar(weight, angle);
                        }
                    }
                }
                assert!(
                    (volume.signed_value([a, b, c]).unwrap() - expected.re).abs() < 1e-8,
                    "{a} {b} {c}: {} vs {}",
                    volume.signed_value([a, b, c]).unwrap(),
                    expected.re
                );
            }
        }
    }
}
