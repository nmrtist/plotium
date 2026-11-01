use super::*;
use plotium_figure::UnitInterval;

fn provenance() -> EstimateProvenance {
    EstimateProvenance {
        estimator: plotium_analysis::robust::NMR_BACKGROUND_ID.into(),
        version: 1,
    }
}

fn gaussian() -> Vec<f32> {
    let mut state = 123456789_u64;
    let mut uniform = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state as f64 / u64::MAX as f64).max(f64::MIN_POSITIVE)
    };
    (0..256 * 256)
        .map(|_| {
            ((-2.0 * uniform().ln()).sqrt() * (std::f64::consts::TAU * uniform()).cos()) as f32
        })
        .collect()
}

fn measured(values: &[f32]) -> NoiseEstimate {
    let EstimateResult::Noise(noise) = diagnose_noise(values, 256, 256, provenance()).unwrap()
    else {
        panic!("stationary synthetic background should be measurable")
    };
    noise
}

fn base(noise: &NoiseEstimate, peak: f64) -> f64 {
    noise_contour_base(
        noise,
        5.0,
        UnitInterval::new(0.0).unwrap(),
        FieldSummary {
            min: FiniteF64::new(-peak).unwrap(),
            max: FiniteF64::new(peak).unwrap(),
        },
    )
}

#[test]
fn correlated_background_is_suppressed_without_a_strong_peak_penalty() {
    let white = gaussian();
    let mut values = vec![0.0; white.len()];
    for r in 0..256 {
        for c in 0..256 {
            values[r * 256 + c] = (0..4)
                .flat_map(|dr| (0..4).map(move |dc| (dr, dc)))
                .map(|(dr, dc)| white[((r + dr) % 256) * 256 + (c + dc) % 256])
                .sum::<f32>()
                / 4.0;
        }
    }
    let old = 5.0 * plotium_analysis::robust::robust_difference_mad(&values, 256, 256);
    let noise = measured(&values);
    let threshold = base(&noise, 100.0);
    let old_count = values.iter().filter(|v| f64::from(v.abs()) > old).count();
    let new_count = values
        .iter()
        .filter(|v| f64::from(v.abs()) > threshold)
        .count();
    assert!(old_count > 100);
    assert!(
        new_count * 10 < old_count,
        "old={old_count}, new={new_count}"
    );
    assert!((noise.scale.get() - 1.0).abs() < 0.12);
    // Isolated high dynamic-range features must not set a global display floor.
    for &(index, amplitude) in &[(3000, 1e3), (15000, -1e5), (45000, 1e6)] {
        values[index] = amplitude;
    }
    let strong = base(&measured(&values), 1e6);
    assert!((strong / threshold - 1.0).abs() < 0.1);
    assert!(
        strong < 8.0,
        "8-sigma weak peaks remain above the first level"
    );
    eprintln!(
        "correlated contours: old base={old:.3} new={threshold:.3}; background pixels {old_count}->{new_count}; strong-peak base={strong:.3}"
    );
}

#[test]
fn offsets_and_sign_reversal_keep_shared_threshold_conservative() {
    let values = gaussian();
    let shifted = values.iter().map(|v| *v + 3.0).collect::<Vec<_>>();
    let inverse = shifted.iter().map(|v| -*v).collect::<Vec<_>>();
    let original = base(&measured(&values), 100.0);
    let shifted_base = base(&measured(&shifted), 100.0);
    assert!((shifted_base - original - 3.0).abs() < 0.3);
    assert!((shifted_base - base(&measured(&inverse), 100.0)).abs() < 1e-5);
    assert!(
        shifted
            .iter()
            .filter(|v| f64::from(v.abs()) > shifted_base)
            .count()
            < 5
    );
}

#[test]
fn rejected_spatial_background_still_resolves_contours() {
    let mut values = gaussian();
    for (i, value) in values.iter_mut().enumerate() {
        if i / 256 < 128 {
            *value *= 10.0;
        }
    }
    assert_eq!(
        plotium_analysis::nmr_background::diagnose(&values, 256, 256)
            .unwrap()
            .status,
        plotium_analysis::nmr_background::Status::Unidentifiable
    );
    let noise = measured(&values);
    let spec = crate::state::default_contour_spec(
        &crate::state::scalar_grid_capabilities(true, &[crate::automation::CAP_FIELD_NOISE_SCALE]),
        crate::state::NO_PEAK,
    );
    let source = VersionedFieldRef {
        field: FieldRef {
            resource: DatasetId::from_uuid(uuid::Uuid::from_u128(99)),
            field: FieldId::new(0),
        },
        version: FieldVersion(1),
    };
    let resolved = resolve_contour_levels(
        source,
        &spec,
        FieldSummary {
            min: FiniteF64::new(-100.0).unwrap(),
            max: FiniteF64::new(100.0).unwrap(),
        },
        |_| Some(EstimateResult::Noise(noise.clone())),
    );
    let ContourResolution::Ready { levels, .. } = resolved else {
        panic!("nonuniform spectrum must resolve")
    };
    assert!(!levels.positive.is_empty());
}

#[test]
fn magnitude_processing_removes_gaussian_capability_only_after_completion() {
    use crate::automation::{CAP_FIELD_BOUNDED, CAP_FIELD_NOISE_SCALE, CAP_FIELD_SIGNED};
    use crate::state::{ChartSpec, DataBinding, DataDomain, Dataset, PlotiumApp, StackSpec};
    use plotium_processing::{ProcessingStep, StepKind, StepSource};
    let mut dataset = super::tests::grid_dataset("modulus", &gaussian());
    let binding = DataBinding::single(&dataset);
    let field = binding.series[0].source.field;
    let Dataset::Nmr2D(nmr) = &mut dataset else {
        unreachable!()
    };
    let id = nmr.allocate_step_id();
    nmr.params.f2.steps.push(ProcessingStep::new(
        id,
        StepKind::Magnitude,
        StepSource::User,
    ));
    // Editing a pending recipe must not relabel the currently displayed field.
    assert!(
        dataset
            .field_descriptor(field)
            .unwrap()
            .capabilities
            .contains(CAP_FIELD_NOISE_SCALE)
    );
    let mut app = PlotiumApp::new();
    app.doc.datasets.push(dataset);
    assert!(app.schedule_2d_processing(0, false));
    super::tests::wait_for_app_compute(&mut app);
    let caps = app.doc.datasets[0]
        .field_descriptor(field)
        .unwrap()
        .capabilities;
    assert!(!caps.contains(CAP_FIELD_BOUNDED));
    let defaults = crate::state::default_contour_spec(&caps, crate::state::NO_PEAK);
    assert!(matches!(
        defaults.positive.base,
        plotium_figure::ContourBasePolicy::Absolute(_)
    ));
    assert!(!caps.contains(CAP_FIELD_NOISE_SCALE));
    assert!(caps.contains(CAP_FIELD_SIGNED));
    // A stored noise policy must not silently continue under a false model.
    crate::contour_probe::reset();
    let figure = app.build_binding_figure(
        &binding,
        &ChartSpec::default_for(DataDomain::Nmr2d),
        &StackSpec::default(),
        [120.0, 80.0],
    );
    assert!(figure.contours.is_empty());
    assert_eq!(crate::contour_probe::queued_estimates(), 0);
    assert!(app.session.status.contains("absolute"));
}
