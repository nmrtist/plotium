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
fn local_phase_preview_replaces_committed_phase_and_repivots_with_engine() {
    let input = source(&fixture());
    let mut baseline = [VolumePhase::default(); 3];
    baseline[2] = VolumePhase {
        p0_degrees: 37.0,
        p1_degrees: -81.0,
        pivot_fraction: 0.27,
    };
    let committed = NmrVolume::process(
        &input,
        baseline,
        Default::default(),
        &mut ExecutionContext::default(),
    )
    .unwrap();
    let mut draft = baseline;
    draft[2] = VolumePhase {
        p0_degrees: -19.0,
        p1_degrees: 131.0,
        pivot_fraction: 0.73,
    };
    let applied = NmrVolume::process(
        &input,
        draft,
        Default::default(),
        &mut ExecutionContext::default(),
    )
    .unwrap();
    for fixed in 0..3 {
        for index in 0..committed.shape()[fixed] {
            let started = std::time::Instant::now();
            let preview = committed
                .phase_preview_plane(
                    fixed,
                    index,
                    baseline,
                    draft,
                    &mut ExecutionContext::default(),
                )
                .unwrap();
            eprintln!(
                "local preview debug numerical profile: {:?}, fixed={fixed}, elapsed={:?}",
                committed.shape(),
                started.elapsed()
            );
            let expected = applied.plane_source(fixed, index).unwrap();
            let mut repivoted = draft;
            repivoted[2].repivot(0.11).unwrap();
            let invariant = committed
                .phase_preview_plane(
                    fixed,
                    index,
                    baseline,
                    repivoted,
                    &mut ExecutionContext::default(),
                )
                .unwrap();
            // Compare every stored component, including imaginary lanes, against
            // a fresh source-to-volume execution by the actual nmr engine.
            for ((actual, expected), invariant) in preview
                .dataset()
                .as_processed()
                .unwrap()
                .data()
                .samples()
                .iter()
                .zip(expected.dataset().as_processed().unwrap().data().samples())
                .zip(invariant.dataset().as_processed().unwrap().data().samples())
            {
                assert!((actual - expected).abs() < 1e-8);
                assert!((actual - invariant).abs() < 1e-8);
            }
        }
    }
}

fn cartesian_volume(shape: [usize; 3], reverse: bool) -> plotium_io::nmr_view::NmrSource {
    component_volume(
        shape,
        reverse,
        [const { nmr::processed::ComponentBasis::Cartesian }; 3],
    )
}

fn component_volume(
    shape: [usize; 3],
    reverse: bool,
    bases: [nmr::processed::ComponentBasis; 3],
) -> plotium_io::nmr_view::NmrSource {
    use nmr::axis::{AxisCoordinates, AxisDomain, AxisRole, AxisUnit};
    use nmr::processed::{ComponentBasis, ProcessedAxis, ProcessedDescriptor};
    let parent_volume = NmrVolume::process_source(&source(&fixture())).unwrap();
    let parent = parent_volume.source();
    let axes = shape
        .into_iter()
        .enumerate()
        .map(|(index, points)| {
            ProcessedAxis::new(
                AxisRole::Signal,
                AxisDomain::Frequency,
                Some(AxisUnit::Ppm),
                points,
                AxisCoordinates::Uniform {
                    start: 12.0,
                    step: if reverse { -0.031 } else { 0.031 },
                },
                bases[index].clone(),
            )
            .unwrap()
        })
        .collect();
    let samples = (0..shape.iter().product::<usize>()
        * bases
            .iter()
            .map(ComponentBasis::component_count)
            .product::<usize>())
        .map(|i| (i as f64 * 0.171).sin() + (i as f64 * 0.031).cos())
        .collect();
    let lineage = (0..3)
        .map(|axis| {
            nmr::external::ExternalAxisSource::Parent(nmr::provenance::InputAxisRef::new(
                nmr::provenance::InputSlot::new(0),
                axis,
            ))
        })
        .collect();
    let output = parent
        .dataset()
        .derive_external_processed(
            ProcessedDescriptor::new(axes).unwrap(),
            samples,
            lineage,
            nmr::external::ExternalAlgorithmDeclaration::new(
                "plotium.test.cartesian-volume",
                "1",
                "Synthetic hypercomplex fixture",
            )
            .unwrap(),
        )
        .unwrap();
    plotium_io::nmr_view::NmrSource::new(std::sync::Arc::new(output)).unwrap()
}

#[test]
fn unsupported_bases_never_claim_local_phase_capability() {
    use nmr::processed::ComponentBasis;
    for conjugated in [false, true] {
        let bases = [
            ComponentBasis::SharedComplex {
                axis: nmr::acquisition::AxisIndex::new(2),
                conjugated,
            },
            ComponentBasis::Cartesian,
            ComponentBasis::Cartesian,
        ];
        let input = component_volume([3, 5, 7], true, bases);
        let volume = NmrVolume::process_source(&input).unwrap();
        assert!(!volume.supports_local_phase_preview());
        assert!(
            volume
                .phase_preview_plane(
                    0,
                    1,
                    [VolumePhase::default(); 3],
                    [VolumePhase::default(); 3],
                    &mut ExecutionContext::default()
                )
                .unwrap_err()
                .contains("independent Cartesian")
        );
    }
    let input = component_volume([3, 5, 7], false, [const { ComponentBasis::Scalar }; 3]);
    let volume = NmrVolume::process_source(&input).unwrap();
    assert!((0..3).all(|axis| !volume.supports_phase(axis)));
    let mut draft = [VolumePhase::default(); 3];
    draft[0].p0_degrees = 31.0;
    assert!(
        volume
            .phase_preview_plane(
                0,
                1,
                [VolumePhase::default(); 3],
                draft,
                &mut ExecutionContext::default()
            )
            .is_err()
    );
}

#[test]
fn hypercomplex_local_preview_matches_engine_on_all_axes_and_reversed_grids() {
    for reverse in [false, true] {
        let input = cartesian_volume([7, 5, 9], reverse);
        let baseline = std::array::from_fn(|axis| VolumePhase {
            p0_degrees: 13.0 + axis as f64 * 11.0,
            p1_degrees: -57.0,
            pivot_fraction: 0.19,
        });
        let committed = NmrVolume::process(
            &input,
            baseline,
            Default::default(),
            &mut ExecutionContext::default(),
        )
        .unwrap();
        for changed in 0..3 {
            let mut draft = baseline;
            draft[changed] = VolumePhase {
                p0_degrees: -41.0,
                p1_degrees: 93.0,
                pivot_fraction: 0.83,
            };
            let applied = NmrVolume::process(
                &input,
                draft,
                Default::default(),
                &mut ExecutionContext::default(),
            )
            .unwrap();
            for fixed in 0..3 {
                for index in [
                    0,
                    committed.shape()[fixed] / 2,
                    committed.shape()[fixed] - 1,
                ] {
                    let preview = committed
                        .phase_preview_plane(
                            fixed,
                            index,
                            baseline,
                            draft,
                            &mut ExecutionContext::default(),
                        )
                        .unwrap();
                    let expected = applied.plane_source(fixed, index).unwrap();
                    let mut moved = draft;
                    moved[changed].repivot(0.07).unwrap();
                    let invariant = committed
                        .phase_preview_plane(
                            fixed,
                            index,
                            baseline,
                            moved,
                            &mut ExecutionContext::default(),
                        )
                        .unwrap();
                    for ((a, b), c) in preview
                        .dataset()
                        .as_processed()
                        .unwrap()
                        .data()
                        .samples()
                        .iter()
                        .zip(expected.dataset().as_processed().unwrap().data().samples())
                        .zip(invariant.dataset().as_processed().unwrap().data().samples())
                    {
                        assert!(
                            (a - b).abs() < 1e-10,
                            "reverse={reverse} changed={changed} fixed={fixed} index={index}"
                        );
                        assert!((a - c).abs() < 1e-10);
                    }
                }
            }
        }
    }
}

#[test]
#[ignore = "explicit local preview benchmark"]
fn benchmark_local_phase_preview() {
    let input = cartesian_volume([64, 128, 256], true);
    let committed = NmrVolume::process_source(&input).unwrap();
    let baseline = [VolumePhase::default(); 3];
    let draft = [VolumePhase {
        p0_degrees: 21.0,
        p1_degrees: -137.0,
        pivot_fraction: 0.37,
    }; 3];
    for fixed in 0..3 {
        let mut times = Vec::new();
        for _ in 0..10 {
            let start = std::time::Instant::now();
            std::hint::black_box(
                committed
                    .phase_preview_plane(
                        fixed,
                        0,
                        baseline,
                        draft,
                        &mut ExecutionContext::default(),
                    )
                    .unwrap(),
            );
            times.push(start.elapsed());
        }
        times.sort();
        eprintln!(
            "shape={:?}, 8 components, fixed={fixed}, median={:?}, max={:?}",
            committed.shape(),
            times[5],
            times[9]
        );
    }
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
