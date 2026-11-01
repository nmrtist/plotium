use super::*;
use crate::state::{
    ContourResolution, DatasetId, EstimateProvenance, EstimateResult, FieldId, FieldRef,
    FieldSummary, FieldVersion, FiniteF64, VersionedFieldRef, resolve_contour_levels,
};
use num_complex::Complex64;
use plotium_figure::ContourStyle;
use plotium_processing::AxisMeta;
use plotium_render::{Margins, Projector, Rect};

/// A tilted plane: every first difference is identical, so its robust MAD is
/// exactly zero — the degenerate estimate an ideal noiseless grid produces.
fn planar_values() -> Vec<f32> {
    (0..4u8)
        .flat_map(|row| (0..4u8).map(move |col| 1.0 + f32::from(row) + f32::from(col)))
        .collect()
}

/// Resolve the same positive half through both contour paths: the legacy
/// analysis-map helper, and the versioned field resolver fed the degenerate
/// estimate its worker would produce for these values.
fn both_paths(values: &[f32], level: &ContourLevelSpec) -> (Vec<f64>, Vec<f64>) {
    let legacy = contour_levels(values, 4, 4, level, false);
    let (minimum, maximum) = finite_range(values).expect("fixture values are finite");
    let spec = ContourSpec {
        positive: level.clone(),
        negative: None,
        style: ContourStyle::default(),
    };
    let source = VersionedFieldRef {
        field: FieldRef {
            resource: DatasetId::from_uuid(uuid::Uuid::from_u128(77)),
            field: FieldId::new(0),
        },
        version: FieldVersion(1),
    };
    let summary = FieldSummary {
        min: FiniteF64::new(minimum).expect("finite"),
        max: FiniteF64::new(maximum).expect("finite"),
    };
    let diagnostic = crate::state::diagnose_noise(
        values,
        4,
        4,
        EstimateProvenance {
            estimator: plotium_analysis::robust::NMR_BACKGROUND_ID.to_owned(),
            version: plotium_analysis::robust::NMR_BACKGROUND_VERSION,
        },
    )
    .unwrap();
    let resolution = resolve_contour_levels(source, &spec, summary, |_| Some(diagnostic.clone()));
    if matches!(resolution, ContourResolution::Unavailable) {
        assert!(matches!(diagnostic, EstimateResult::Unavailable(_)));
        return (legacy, Vec::new());
    }
    let ContourResolution::Ready {
        levels: resolved, ..
    } = resolution
    else {
        panic!("the estimate is supplied, so resolution is not pending");
    };
    (
        legacy,
        resolved.positive.iter().map(|level| level.get()).collect(),
    )
}

fn noise_sigma_level(count: u16) -> ContourLevelSpec {
    ContourLevelSpec {
        base: ContourBasePolicy::NoiseFloor {
            multiplier: plotium_figure::PositiveFiniteF64::new(5.0).unwrap(),
            peak_fraction: plotium_figure::UnitInterval::new(0.0).expect("a zero floor is valid"),
            estimator: plotium_figure::EstimatorSelection::FollowLatest,
        },
        count,
        ratio: plotium_figure::PositiveFiniteF64::new(1.5).unwrap(),
    }
}

// The whole point of extracting `contour_ladder`: there is one policy for
// what an unusable base means, so the legacy ILT/DOSY path and the versioned
// field resolver cannot drift apart before ILT/DOSY move onto
// `FieldId`/`FieldVersion`.
#[test]
fn above_peak_amplitude_thresholds_are_respected_on_both_paths() {
    let planar = planar_values();

    // A measured amplitude threshold above the peak.
    let (legacy, resolved) = both_paths(&planar, &noise_sigma_level(1));
    assert_eq!(legacy, resolved);
    assert!(legacy.is_empty());

    // The same threshold with multiple levels.
    let (legacy, resolved) = both_paths(&planar, &noise_sigma_level(5));
    assert_eq!(legacy, resolved);
    assert!(legacy.is_empty());

    // An explicit threshold beyond the peak has no crossing, and is obeyed
    // literally rather than rewritten — on both paths alike.
    let above_peak = ContourLevelSpec {
        base: ContourBasePolicy::Absolute(plotium_figure::PositiveFiniteF64::new(1_000.0).unwrap()),
        count: 4,
        ratio: plotium_figure::PositiveFiniteF64::new(1.5).unwrap(),
    };
    let (legacy, resolved) = both_paths(&planar, &above_peak);
    assert_eq!(legacy, resolved);
    assert!(legacy.is_empty());
}

#[test]
fn one_level_specs_draw_exactly_one_level_on_both_paths() {
    let planar = planar_values();
    // A usable base stays exactly where the user put it.
    let usable = ContourLevelSpec {
        base: ContourBasePolicy::Absolute(plotium_figure::PositiveFiniteF64::new(2.0).unwrap()),
        count: 1,
        ratio: plotium_figure::PositiveFiniteF64::new(1.5).unwrap(),
    };
    let (legacy, resolved) = both_paths(&planar, &usable);
    assert_eq!(legacy, [2.0]);
    assert_eq!(resolved, [2.0]);

    // The measured threshold exceeds this small plane's peak on both paths.
    let (legacy, resolved) = both_paths(&planar, &noise_sigma_level(1));
    assert!(legacy.is_empty());
    assert!(resolved.is_empty());
}

fn spectrum_2d() -> Spectrum2D {
    let f2_ppm = vec![0.0, 1.0, 2.0, 3.0];
    let f1_ppm = vec![0.0, 1.0, 2.0, 3.0];
    let (f2_size, f1_size) = (f2_ppm.len(), f1_ppm.len());
    Spectrum2D {
        magnitude_plane: None,
        f2_domain: plotium_io::Domain::Frequency,
        f1_domain: plotium_io::Domain::Frequency,
        data: vec![Complex64::new(1.0, 0.0); f1_size * f2_size],
        f2_ppm,
        f1_ppm,
        f2_size,
        f1_size,
        direct: AxisMeta {
            nucleus: "1H".to_owned(),
            observe_freq_mhz: Some(400.0),
            unit: Some(nmr::axis::AxisUnit::Ppm),
        },
        indirect: AxisMeta {
            nucleus: "13C".to_owned(),
            observe_freq_mhz: Some(100.0),
            unit: Some(nmr::axis::AxisUnit::Ppm),
        },
        source: "test".to_owned(),
    }
}

#[test]
fn equal_scale_requires_matching_frequency_axes_and_ranges_within_twofold() {
    let mut spectrum = spectrum_2d();
    spectrum.indirect.nucleus = "1H".to_owned();
    assert!(equal_scale_for_nmr_2d(&spectrum));

    spectrum.f2_ppm = vec![0.0, 2.0, 4.0, 6.0];
    assert!(
        equal_scale_for_nmr_2d(&spectrum),
        "a range exactly twice as wide remains eligible"
    );

    spectrum.f2_ppm = vec![0.0, 2.1, 4.2, 6.3];
    assert!(!equal_scale_for_nmr_2d(&spectrum));

    spectrum.f2_ppm = vec![0.0, 1.0, 2.0, 3.0];
    spectrum.indirect.nucleus = "13C".to_owned();
    assert!(!equal_scale_for_nmr_2d(&spectrum));

    spectrum.indirect.nucleus = "1H".to_owned();
    spectrum.f1_domain = plotium_io::Domain::Time;
    assert!(!equal_scale_for_nmr_2d(&spectrum));
}

// The 2D NMR convention places low chemical shift at the top of the plot. The
// F1 axis is built `reversed`, which — with the projector's own y-flip — maps
// low F1 ppm to `plot.top`. Guards against "un-reversing" it (screen and export
// share the projector, so a wrong flip is invisible in preview).
#[test]
fn contour_places_low_f1_ppm_near_the_top() {
    let fig = build_figure_2d(&spectrum_2d(), Preset2D::Hsqc);
    assert_eq!(fig.axis_frame, AxisFrame::Box);
    let proj = Projector::new(&fig, Rect::new(0.0, 0.0, 400.0, 300.0), &Margins::default());
    let (_, py_low_ppm) = proj.project([1.5, 0.0]);
    let (_, py_high_ppm) = proj.project([1.5, 3.0]);
    // Screen y grows downward, so the smaller py is higher on the page.
    assert!(
        py_low_ppm < py_high_ppm,
        "low F1 ppm ({py_low_ppm}) should sit above high F1 ppm ({py_high_ppm})"
    );
}

// Positive control for the marching-squares probe. Every "no synchronous
// contour build" assertion elsewhere reads zero from the same counter, so
// that counter must be shown to move at least once: this test fails the
// moment an increment is dropped from a call site.
//
// It also pins the remaining synchronous path: ILT/DOSY analysis maps still
// contour on the caller's thread and have not been moved onto the versioned
// field cache.
#[test]
fn ilt_figure_runs_marching_squares_on_the_calling_thread() {
    crate::contour_probe::reset();
    let result = IltResult {
        diagnostics: Vec::new(),
        ppm: vec![0.0, 1.0, 2.0, 3.0],
        d_grid: vec![1.0e-10, 2.0e-10, 4.0e-10, 8.0e-10],
        amp: vec![vec![0.0, 0.5, 1.0, 0.5]; 4],
    };

    let figure = build_ilt_figure(&result, "1H", "probe");

    assert!(
        !figure.contours.is_empty(),
        "the fixture must actually reach contour extraction"
    );
    assert!(
        crate::contour_probe::marching_squares_on_this_thread() > 0,
        "the marching-squares probe must observe a build on the calling thread"
    );
}

#[test]
fn axis_label_formats_isotope_mass_as_superscript() {
    assert_eq!(axis_label("13C"), "¹³C chemical shift (ppm)");
    assert_eq!(axis_label("1H"), "¹H chemical shift (ppm)");
    assert_eq!(axis_label("F"), "F chemical shift (ppm)");
}

#[test]
fn background_scale_removes_a_planar_afm_tilt_before_measuring_mad() {
    let values = (0..4)
        .flat_map(|row| (0..4).map(move |col| 10.0 + 3.0 * col as f32 + 2.0 * row as f32))
        .collect::<Vec<_>>();
    let (location, scale) = deplaned_location_scale(&values, 4, 4);

    assert!((location - 17.5).abs() < 1e-10);
    assert!(scale < 1e-10, "a plane is not background roughness");
}

#[test]
fn finite_range_preserves_signed_and_baselined_fields() {
    assert_eq!(finite_range(&[-40.0, -5.0]), Some((-40.0, -5.0)));
    assert_eq!(finite_range(&[500.0, 600.0]), Some((500.0, 600.0)));
    assert_eq!(finite_range(&[f32::NAN, f32::INFINITY]), None);
}

#[test]
fn fraction_of_range_starts_from_the_field_minimum() {
    let level = ContourLevelSpec {
        base: ContourBasePolicy::FractionOfRange(plotium_figure::UnitInterval::new(0.04).unwrap()),
        count: 1,
        ratio: plotium_figure::PositiveFiniteF64::new(1.35).unwrap(),
    };
    assert_eq!(
        contour_levels(&[500.0, 600.0], 1, 2, &level, false),
        [504.0]
    );
}

#[test]
fn all_negative_fields_get_negative_levels() {
    let level = ContourLevelSpec {
        base: ContourBasePolicy::Absolute(plotium_figure::PositiveFiniteF64::new(10.0).unwrap()),
        count: 1,
        ratio: plotium_figure::PositiveFiniteF64::new(1.35).unwrap(),
    };
    assert_eq!(contour_levels(&[-40.0, -5.0], 1, 2, &level, true), [-10.0]);
    assert!(contour_levels(&[-40.0, -5.0], 1, 2, &level, false).is_empty());

    // The shared ladder speaks only in positive magnitudes, so each path
    // must still apply its own half's sign. The versioned resolver agrees.
    let spec = ContourSpec {
        positive: level.clone(),
        negative: Some(level),
        style: ContourStyle::default(),
    };
    let source = VersionedFieldRef {
        field: FieldRef {
            resource: DatasetId::from_uuid(uuid::Uuid::from_u128(78)),
            field: FieldId::new(0),
        },
        version: FieldVersion(1),
    };
    let summary = FieldSummary {
        min: FiniteF64::new(-40.0).expect("finite"),
        max: FiniteF64::new(-5.0).expect("finite"),
    };
    let ContourResolution::Ready {
        levels: resolved,
        unreachable,
    } = resolve_contour_levels(source, &spec, summary, |_| None)
    else {
        panic!("an absolute contour needs no estimate");
    };
    assert!(
        unreachable.is_empty(),
        "an all-negative field has no positive signal at all; that is the \
         field's shape, not a mistyped threshold"
    );
    assert_eq!(
        resolved
            .negative
            .iter()
            .map(|level| level.get())
            .collect::<Vec<_>>(),
        [-10.0]
    );
    assert!(resolved.positive.is_empty());
}

#[test]
fn small_plane_amplitude_threshold_above_peak_is_respected() {
    let level = ContourLevelSpec {
        base: ContourBasePolicy::NoiseFloor {
            multiplier: plotium_figure::PositiveFiniteF64::new(5.0).unwrap(),
            peak_fraction: plotium_figure::UnitInterval::new(0.0).expect("a zero floor is valid"),
            estimator: plotium_figure::EstimatorSelection::FollowLatest,
        },
        count: 1,
        ratio: plotium_figure::PositiveFiniteF64::new(1.35).unwrap(),
    };
    assert!(contour_levels(&[0.0, 2.0], 1, 2, &level, false).is_empty());
}
