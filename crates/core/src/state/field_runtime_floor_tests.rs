//! Explicit strong-peak protection remains honored independently of the default.
//! These tests check policy arithmetic against a recorded historical summary;
//! they do not validate the new estimator on the original experimental plane.

use super::tests::{finite, source};
use super::*;
use crate::state::{CONTOUR_BASE_NOISE_FLOOR, contour_base_policy};
use plotium_figure::{
    ContourLevelSpec, ContourSpec, ContourStyle, EstimatorSelection, PositiveFiniteF64,
    UnitInterval,
};

/// The ¹H–¹H NOESY the floor was calibrated on: a 2048 × 8192 real plane whose
/// peak is 3.304e8 against a robust noise estimate of 1.669e3, a dynamic range
/// of 197,900:1. Sweeping a single level over that grid, the crossing count
/// falls from 8.99e5 at 0.004 % of peak to 7.56e4 at 0.008 % and then halves
/// smoothly per octave: below ≈ 0.008 % of peak the contour is tracing the
/// field's artefact floor rather than its peaks.
const NOESY_PEAK: f64 = 3.3042e8;
const NOESY_SIGMA: f64 = 1.6690e3;
/// Where the measured crossing count stops falling steeply — the artefact floor.
const NOESY_ARTEFACT_KNEE_FRACTION: f64 = 8.0e-5;

fn scale_estimate(scale: f64) -> EstimateResult {
    EstimateResult::Noise(NoiseEstimate {
        baseline: 0.0,
        scale: EstimatedScale::new(scale).expect("test scales are finite and non-negative"),
        provenance: EstimateProvenance {
            estimator: plotium_analysis::robust::NMR_BACKGROUND_ID.to_owned(),
            version: plotium_analysis::robust::NMR_BACKGROUND_VERSION,
        },
    })
}

fn floored_spec(multiplier: f64, peak_fraction: f64, count: u16) -> ContourSpec {
    let level = ContourLevelSpec {
        base: ContourBasePolicy::NoiseFloor {
            multiplier: PositiveFiniteF64::new(multiplier).expect("test multiplier is positive"),
            peak_fraction: UnitInterval::new(peak_fraction).expect("test fraction is in [0, 1]"),
            estimator: EstimatorSelection::FollowLatest,
        },
        count,
        ratio: PositiveFiniteF64::new(1.35).expect("literal ratio is valid"),
    };
    ContourSpec {
        positive: level.clone(),
        negative: Some(level),
        style: ContourStyle::default(),
    }
}

fn resolved(spec: &ContourSpec, summary: FieldSummary, sigma: f64) -> ResolvedContourLevels {
    let ContourResolution::Ready { levels, .. } =
        resolve_contour_levels(source(11, 0, 1), spec, summary, |_| {
            Some(scale_estimate(sigma))
        })
    else {
        panic!("the estimate is supplied, so resolution is not pending");
    };
    levels
}

fn spanning(peak: f64) -> FieldSummary {
    FieldSummary {
        min: finite(-peak),
        max: finite(peak),
    }
}

/// The property the whole change turns on: a floor is a floor, not a
/// substitute. On a field whose estimated σ is ordinary next to its peak the
/// floor is never reached, and resolution produces byte-for-byte the ladder an
/// unfloored σ anchor produces. Nothing about an ordinary spectrum moves.
#[test]
fn a_field_whose_sigma_clears_the_floor_resolves_exactly_as_an_unfloored_anchor() {
    // Dynamic range 1,000:1 — a σ a hundred times the floor's fraction.
    let summary = spanning(1.0e6);
    let sigma = 1.0e3;
    let floored = resolved(&floored_spec(5.0, 1.0e-4, 14), summary, sigma);
    let unfloored = resolved(&floored_spec(5.0, 0.0, 14), summary, sigma);

    assert_eq!(floored, unfloored);
    assert_eq!(
        floored.positive.first().map(|level| level.get()),
        Some(5.0e3)
    );
    assert_eq!(
        resolved_noise_scale(
            EstimatedScale::new(sigma).expect("finite"),
            UnitInterval::new(1.0e-4).expect("valid"),
            summary,
        ),
        (sigma, NoiseScaleTerm::Estimated),
    );
}

/// The dynamic range at which the two terms swap is exactly the reciprocal of
/// the fraction, and the swap is continuous: at the crossing point both terms
/// give the same scale, so no field's ladder jumps as its σ drifts across it.
#[test]
fn the_terms_swap_at_the_reciprocal_of_the_fraction_without_a_step() {
    let summary = spanning(1.0e6);
    let fraction = UnitInterval::new(1.0e-4).expect("valid");
    let exactly_at_the_floor = 1.0e-4 * 1.0e6;

    let (scale, term) = resolved_noise_scale(
        EstimatedScale::new(exactly_at_the_floor).expect("finite"),
        fraction,
        summary,
    );
    assert_eq!((scale, term), (100.0, NoiseScaleTerm::Estimated));

    let (scale, term) = resolved_noise_scale(
        EstimatedScale::new(exactly_at_the_floor * 0.5).expect("finite"),
        fraction,
        summary,
    );
    assert_eq!((scale, term), (100.0, NoiseScaleTerm::PeakFloor));
}

/// An explicit peak fraction applied to the historical calibration summary.
#[test]
fn the_calibrating_noesy_is_held_above_its_measured_artefact_floor() {
    let summary = spanning(NOESY_PEAK);
    let spec = floored_spec(5.0, 1.0e-4, 14);
    let levels = resolved(&spec, summary, NOESY_SIGMA);
    let base = levels
        .positive
        .first()
        .map(|level| level.get())
        .expect("a ladder with signal has a lowest level");

    let (scale, term) = resolved_noise_scale(
        EstimatedScale::new(NOESY_SIGMA).expect("finite"),
        UnitInterval::new(1.0e-4).expect("valid"),
        summary,
    );
    assert_eq!(term, NoiseScaleTerm::PeakFloor);
    assert!((scale - 1.0e-4 * NOESY_PEAK).abs() < 1.0e-6);

    assert!(
        base > NOESY_ARTEFACT_KNEE_FRACTION * NOESY_PEAK,
        "the lowest level {base:e} must sit above the measured artefact knee \
         {:e}, or the ladder is drawing t1 noise",
        NOESY_ARTEFACT_KNEE_FRACTION * NOESY_PEAK
    );
    // The unfloored synthetic summary yields a base about twenty times lower.
    // This checks arithmetic, not the new background diagnostics on real data.
    let unfloored = resolved(&floored_spec(5.0, 0.0, 14), summary, NOESY_SIGMA);
    let unfloored_base = unfloored
        .positive
        .first()
        .map(|level| level.get())
        .expect("the unfloored ladder also has a lowest level");
    assert!(unfloored_base < NOESY_ARTEFACT_KNEE_FRACTION * NOESY_PEAK);
    assert!(base / unfloored_base > 15.0);
}

/// The floor is measured against the field's peak magnitude, not each half's
/// own. Sampling artefacts are driven by the strongest feature whatever its
/// sign; a per-half floor would also give the two halves different ladders,
/// which the geometry budget's "drop a magnitude from both signs together" rule
/// depends on them not having.
#[test]
fn both_halves_share_one_floor_taken_from_the_fields_peak() {
    let summary = FieldSummary {
        min: finite(-1.0e8),
        max: finite(4.0e8),
    };
    let levels = resolved(&floored_spec(5.0, 1.0e-4, 14), summary, 1.0e3);
    let positive = levels.positive.first().map(|level| level.get());
    let negative = levels.negative.first().map(|level| -level.get());

    assert_eq!(positive, Some(5.0 * 1.0e-4 * 4.0e8));
    assert_eq!(negative, positive, "one field, one noise floor");
}

/// A degenerate estimate is not a very small scale; it is the absence of one.
/// Flooring it would replace the ladder that spans an ideal synthetic field
/// with fourteen rungs crowded against zero, so the degenerate policy in
/// `contour_ladder` keeps the case.
#[test]
fn a_degenerate_estimate_is_not_floored() {
    let summary = spanning(1.0e6);
    assert_eq!(
        resolved_noise_scale(
            EstimatedScale::Degenerate,
            UnitInterval::new(1.0e-4).expect("valid"),
            summary,
        ),
        (0.0, NoiseScaleTerm::Estimated),
    );
}

/// Automatic settings no longer assume a universal artifact-to-peak ratio.
#[test]
fn the_default_noise_anchor_uses_measured_background_only() {
    let policy = contour_base_policy(CONTOUR_BASE_NOISE_FLOOR, crate::state::NO_PEAK).unwrap();
    let ContourBasePolicy::NoiseFloor {
        multiplier,
        peak_fraction,
        ..
    } = policy
    else {
        panic!("noise floor policy")
    };
    assert_eq!(multiplier.get(), 5.0);
    assert_eq!(peak_fraction.get(), 0.0);
}
