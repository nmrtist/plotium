//! Axis scale transforms. Positions on an axis are linear in the scale's
//! transformed space, so every renderer maps values through one transform.

/// How data values are spaced along an axis.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AxisScale {
    #[default]
    Linear,
    /// Decades are evenly spaced. Only positive values have a position.
    Log10,
}

impl AxisScale {
    pub fn is_linear(&self) -> bool {
        *self == AxisScale::Linear
    }

    /// Value → transformed space. On a log scale zero and negative values sit
    /// at negative infinity, beyond the low end of any axis, so callers can
    /// skip them or clip them to that edge; NaN stays NaN.
    pub fn forward(self, value: f64) -> f64 {
        match self {
            AxisScale::Linear => value,
            AxisScale::Log10 if value > 0.0 => value.log10(),
            AxisScale::Log10 if value.is_nan() => f64::NAN,
            AxisScale::Log10 => f64::NEG_INFINITY,
        }
    }

    /// Transformed space → value; the inverse of [`AxisScale::forward`].
    pub fn inverse(self, transformed: f64) -> f64 {
        match self {
            AxisScale::Linear => transformed,
            AxisScale::Log10 => 10f64.powf(transformed),
        }
    }

    /// Whether `[min, max]` has finite positions on this scale.
    pub fn supports_range(self, min: f64, max: f64) -> bool {
        match self {
            AxisScale::Linear => min.is_finite() && max.is_finite(),
            AxisScale::Log10 => min.is_finite() && max.is_finite() && min > 0.0 && max > 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Axis;

    #[test]
    fn log_axis_places_decades_evenly_and_inverts() {
        let mut axis = Axis::new("x", 1.0, 1000.0);
        axis.scale = AxisScale::Log10;
        for (value, expected) in [
            (1.0, 0.0),
            (10.0, 1.0 / 3.0),
            (100.0, 2.0 / 3.0),
            (1000.0, 1.0),
        ] {
            let t = axis.normalize(value);
            assert!((t - expected).abs() < 1e-12, "{value} → {t}");
            assert!((axis.value_at(t) - value).abs() < 1e-9 * value);
        }
        axis.reversed = true;
        assert!((axis.normalize(1000.0)).abs() < 1e-12);
        assert!((axis.value_at(0.0) - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn non_positive_values_have_no_finite_log_position() {
        let mut axis = Axis::new("x", 1.0, 100.0);
        axis.scale = AxisScale::Log10;
        assert_eq!(axis.normalize(0.0), f64::NEG_INFINITY);
        assert_eq!(axis.normalize(-5.0), f64::NEG_INFINITY);
        assert!(axis.normalize(f64::NAN).is_nan());
        axis.reversed = true;
        assert_eq!(axis.normalize(0.0), f64::INFINITY);
    }

    #[test]
    fn linear_inverse_round_trips() {
        let axis = Axis::new("x", -2.0, 6.0).reversed(true);
        assert_eq!(axis.normalize(-2.0), 1.0);
        assert_eq!(axis.value_at(0.25), 4.0);
    }

    #[test]
    fn a_log_axis_with_a_non_positive_bound_draws_linearly() {
        let mut axis = Axis::new("x", -1.0, 9.0);
        axis.scale = AxisScale::Log10;
        assert_eq!(axis.effective_scale(), AxisScale::Linear);
        assert_eq!(axis.normalize(4.0), 0.5);
    }

    #[test]
    fn linear_scale_is_omitted_from_serialized_axes() {
        let mut axis = Axis::new("x", 1.0, 10.0);
        let json = serde_json::to_string(&axis).expect("serializes");
        assert!(!json.contains("scale"));
        axis.scale = AxisScale::Log10;
        let json = serde_json::to_string(&axis).expect("serializes");
        assert!(json.contains(r#""scale":"log10""#));
        let restored: Axis = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(restored.scale, AxisScale::Log10);
    }
}
