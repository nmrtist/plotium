//! Display-only amplification around each trace's own zero-signal baseline.
use crate::{AxisScale, Figure, Series, SeriesKind};

pub const MIN_DISPLAY_GAIN: f64 = 1e-6;
pub const MAX_DISPLAY_GAIN: f64 = 1e6;

pub(crate) fn unit_gain() -> f64 {
    1.0
}

impl Figure {
    /// Recover runtime source coordinates only after complete geometry equality.
    /// Project snapshots carry the materialized geometry, not this cache.
    pub fn inherit_display_gain_sources(&mut self, reference: &Self) {
        for (series, original) in self.series.iter_mut().zip(&reference.series) {
            if series.points == original.points
                && series.display_baseline == original.display_baseline
            {
                series.unamplified_y.clone_from(&original.unamplified_y);
            }
        }
    }

    pub fn supports_display_gain(&self) -> bool {
        !self.series.is_empty()
            && self.heatmap.is_none()
            && self.contours.is_empty()
            && self.polygons.is_empty()
            && self.y2.is_none()
            && self.x.categories.is_none()
            && self.y.categories.is_none()
            && self.y.scale == AxisScale::Linear
            && self.axis_frame != crate::AxisFrame::Hidden
            && self.series.iter().all(|s| {
                matches!(
                    s.kind,
                    SeriesKind::Line | SeriesKind::Points | SeriesKind::Stick
                )
            })
    }

    pub fn check_display_gain(&self, gain: f64) -> Result<(), &'static str> {
        let valid = |value: f64| {
            value.is_finite() && (MIN_DISPLAY_GAIN..=MAX_DISPLAY_GAIN).contains(&value)
        };
        if !valid(gain) || !valid(self.display_gain) {
            return Err("Display gain is outside the supported range.");
        }
        let ratio = gain / self.display_gain;
        if !ratio.is_finite()
            || self.series.iter().any(|series| {
                !series.display_baseline.is_finite()
                    || series
                        .unamplified_y
                        .as_ref()
                        .is_some_and(|ys| ys.len() != series.points.len())
                    || series.points.iter().enumerate().any(|(index, point)| {
                        point[1].is_finite()
                            && !amplify(
                                original_y(series, index, self.display_gain),
                                series.display_baseline,
                                gain,
                            )
                            .is_finite()
                    })
            })
            || self.error_bars.iter().any(|error| {
                !error.display_baseline.is_finite()
                    || !(error.display_baseline
                        + (error.center[1] - error.display_baseline) * ratio)
                        .is_finite()
                    || (!error.horizontal
                        && (!(error.negative * ratio).is_finite()
                            || !(error.positive * ratio).is_finite()))
            })
            || self.annotations.iter().any(|annotation| {
                !annotation.display_baseline.is_finite()
                    || !amplify(annotation.at[1], annotation.display_baseline, ratio).is_finite()
            })
        {
            return Err("This display gain would exceed the numeric range.");
        }
        Ok(())
    }

    /// The applied factor travels with materialized figures so restoring a
    /// snapshot or applying the same viewport twice never amplifies it twice.
    pub fn set_display_gain(&mut self, gain: f64) -> Result<(), &'static str> {
        if gain == self.display_gain {
            return Ok(());
        }
        self.check_display_gain(gain)?;
        let ratio = gain / self.display_gain;
        for series in &mut self.series {
            // Always derive from the same original coordinates. Repeated small
            // wheel steps must not accumulate cancellation around large offsets.
            if series.unamplified_y.is_none() {
                series.unamplified_y = Some(std::sync::Arc::new(
                    (0..series.points.len())
                        .map(|i| original_y(series, i, self.display_gain))
                        .collect(),
                ));
            }
            let source = series
                .unamplified_y
                .as_ref()
                .expect("source coordinates were just captured");
            for (point, &original) in series.points.iter_mut().zip(source.iter()) {
                point[1] = amplify(original, series.display_baseline, gain);
            }
            if gain == 1.0 {
                series.unamplified_y = None;
            }
        }
        for error in &mut self.error_bars {
            error.center[1] =
                error.display_baseline + (error.center[1] - error.display_baseline) * ratio;
            if !error.horizontal {
                error.negative *= ratio;
                error.positive *= ratio;
            }
        }
        for annotation in &mut self.annotations {
            annotation.at[1] = amplify(annotation.at[1], annotation.display_baseline, ratio);
        }
        self.display_gain = gain;
        Ok(())
    }
}

fn original_y(series: &Series, index: usize, gain: f64) -> f64 {
    series.unamplified_y.as_ref().map_or_else(
        || {
            if gain == 1.0 {
                series.points[index][1]
            } else {
                series.display_baseline + (series.points[index][1] - series.display_baseline) / gain
            }
        },
        |source| source[index],
    )
}

fn amplify(original: f64, baseline: f64, gain: f64) -> f64 {
    if gain == 1.0 {
        original
    } else {
        baseline + (original - baseline) * gain
    }
}

#[cfg(test)]
#[path = "display_gain_tests.rs"]
mod tests;
