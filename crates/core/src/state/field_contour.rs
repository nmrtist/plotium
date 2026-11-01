use super::PeakMagnitude;
use plotium_figure::{ContourBasePolicy, EstimatorSelection, PositiveFiniteF64, UnitInterval};

/// Stable ids of the contour base policies, shared by the default factory and
/// the property catalog so a base chosen either way is the same value.
pub const CONTOUR_BASE_ABSOLUTE: &str = "absolute";
pub const CONTOUR_BASE_NOISE_FLOOR: &str = "noise_floor";
pub const CONTOUR_BASE_BACKGROUND_SCALE: &str = "background_scale";
pub const CONTOUR_BASE_FRACTION_OF_RANGE: &str = "fraction_of_range";

/// The conventional lowest level of a peak-anchored ladder, and the fraction the
/// bounded policy starts from.
const CONTOUR_BASE_FRACTION: f64 = 0.04;
/// The conventional distance from the noise or background floor.
const CONTOUR_BASE_MULTIPLIER: f64 = 5.0;
/// Strong-peak scaling is an explicit option, not a measured background.
/// Automatic thresholds use amplitude MAD and the absolute median instead.
const CONTOUR_NOISE_FLOOR_PEAK_FRACTION: f64 = 0.0;

pub fn contour_base_kind(policy: &ContourBasePolicy) -> &'static str {
    match policy {
        ContourBasePolicy::Absolute(_) => CONTOUR_BASE_ABSOLUTE,
        ContourBasePolicy::NoiseFloor { .. } => CONTOUR_BASE_NOISE_FLOOR,
        ContourBasePolicy::BackgroundScale { .. } => CONTOUR_BASE_BACKGROUND_SCALE,
        ContourBasePolicy::FractionOfRange(_) => CONTOUR_BASE_FRACTION_OF_RANGE,
    }
}

/// The canonical parameters of one base policy.
///
/// Whether a policy *may* be chosen is a capability question answered by the
/// caller; this only says what it looks like when it is. Returns `None` for an
/// unknown id rather than substituting a policy the caller did not ask for.
pub fn contour_base_policy(kind: &str, peak: PeakMagnitude<'_>) -> Option<ContourBasePolicy> {
    let policy = match kind {
        CONTOUR_BASE_ABSOLUTE => ContourBasePolicy::Absolute(absolute_base(peak)),
        CONTOUR_BASE_NOISE_FLOOR => ContourBasePolicy::NoiseFloor {
            multiplier: PositiveFiniteF64::new(CONTOUR_BASE_MULTIPLIER)
                .expect("literal multiplier is valid"),
            peak_fraction: UnitInterval::new(CONTOUR_NOISE_FLOOR_PEAK_FRACTION)
                .expect("literal fraction is valid"),
            estimator: EstimatorSelection::Frozen {
                estimator: plotium_analysis::robust::NMR_BACKGROUND_ID.to_owned(),
                version: plotium_analysis::robust::NMR_BACKGROUND_VERSION,
            },
        },
        CONTOUR_BASE_BACKGROUND_SCALE => ContourBasePolicy::BackgroundScale {
            multiplier: PositiveFiniteF64::new(CONTOUR_BASE_MULTIPLIER)
                .expect("literal multiplier is valid"),
            estimator: EstimatorSelection::Frozen {
                estimator: plotium_analysis::robust::DEPLANED_LOCATION_SCALE_ID.to_owned(),
                version: plotium_analysis::robust::DEPLANED_LOCATION_SCALE_VERSION,
            },
        },
        CONTOUR_BASE_FRACTION_OF_RANGE => ContourBasePolicy::FractionOfRange(
            UnitInterval::new(CONTOUR_BASE_FRACTION).expect("literal fraction is valid"),
        ),
        _ => return None,
    };
    Some(policy)
}

/// An absolute base anchored to the field's own peak.
///
/// A fixed literal cannot serve here: a base of one intensity unit draws nothing
/// at all on any field whose peak is below one, and does so silently, with no
/// control in the panel that explains the blank plot. The peak is the only
/// scale-free anchor available when no capability offers a better one; the
/// literal remains solely as the last resort when even that is unknown.
fn absolute_base(peak: PeakMagnitude<'_>) -> PositiveFiniteF64 {
    peak()
        .map(|peak| peak * CONTOUR_BASE_FRACTION)
        .and_then(PositiveFiniteF64::new)
        .unwrap_or_else(|| PositiveFiniteF64::new(1.0).expect("literal base is valid"))
}
