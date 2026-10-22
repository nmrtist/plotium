//! Ticks for logarithmic axes: majors on powers of ten, minors on 2–9 × each
//! decade.

use crate::AxisTicks;

/// Powers of ten inside `[min, max]`, thinned to at most `target` labels.
/// `None` when fewer than two powers are visible, where decade labels would
/// leave the axis unreadable; callers then use linear ticks.
pub(crate) fn decade_ticks(min: f64, max: f64, target: usize) -> Option<AxisTicks> {
    let (lo, hi) = (min.min(max), min.max(max));
    if !(lo > 0.0 && hi.is_finite()) {
        return None;
    }
    // Tolerate rounding so a bound of exactly 10ⁿ keeps its own tick.
    let first = (lo.log10() - 1e-9).ceil() as i32;
    let last = (hi.log10() + 1e-9).floor() as i32;
    let count = usize::try_from(last - first + 1).ok()?;
    if count < 2 {
        return None;
    }
    let stride = count.div_ceil(target.max(1)).max(1);
    // Prefer exponents on multiples of the stride so panning keeps the same
    // labelled decades; fall back to the first decade when that leaves one.
    let aligned: Vec<i32> = (first..=last)
        .filter(|exponent| exponent.rem_euclid(stride as i32) == 0)
        .collect();
    let exponents = if aligned.len() >= 2 {
        aligned
    } else {
        (first..=last).step_by(stride).collect()
    };
    Some(AxisTicks {
        values: exponents.iter().map(|&k| 10f64.powi(k)).collect(),
        labels: exponents.iter().map(|k| format!("10^{{{k}}}")).collect(),
        scale_exponent: None,
        decades: true,
    })
}

/// Minor ticks at 2–9 × every decade that touches `[min, max]`. With
/// thinned majors only the decades themselves are marked, so `count` (the
/// minor-tick density setting) switches them on but does not subdivide.
pub(crate) fn decade_minor_values(min: f64, max: f64, majors: &AxisTicks) -> Vec<f64> {
    let (lo, hi) = (min.min(max), min.max(max));
    if !(lo > 0.0 && hi.is_finite()) {
        return Vec::new();
    }
    let thinned = majors
        .values
        .windows(2)
        .any(|pair| pair[1] / pair[0] > 10.0 * (1.0 + 1e-9));
    let first = lo.log10().floor() as i32;
    let last = hi.log10().ceil() as i32;
    let eps = 1e-9;
    (first..=last)
        .flat_map(|k| {
            let decade = 10f64.powi(k);
            let multiples: &[f64] = if thinned {
                &[1.0]
            } else {
                &[2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]
            };
            multiples.iter().map(move |m| m * decade)
        })
        .filter(|value| {
            *value >= lo * (1.0 - eps)
                && *value <= hi * (1.0 + eps)
                && !majors
                    .values
                    .iter()
                    .any(|major| (major - value).abs() <= major * eps)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn majors_sit_on_every_visible_decade() {
        let ticks = decade_ticks(0.5, 2000.0, 8).expect("four decades");
        assert_eq!(ticks.values, vec![1.0, 10.0, 100.0, 1000.0]);
        assert_eq!(ticks.labels, vec!["10^{0}", "10^{1}", "10^{2}", "10^{3}"]);
        assert_eq!(ticks.scale_exponent, None);
        assert!(ticks.decades);
    }

    #[test]
    fn many_decades_are_thinned_to_the_budget() {
        let ticks = decade_ticks(1e-6, 1e6, 5).expect("thirteen decades");
        assert!(ticks.values.len() <= 5, "{:?}", ticks.values);
        assert!(ticks.values.len() >= 2);
        assert_eq!(ticks.labels[0], "10^{-6}");
        assert!(ticks.labels.contains(&"10^{0}".to_owned()));
    }

    #[test]
    fn less_than_one_visible_power_falls_back() {
        assert!(decade_ticks(2.0, 9.0, 5).is_none());
        assert!(decade_ticks(2.0, 30.0, 5).is_none());
        assert!(decade_ticks(-1.0, 30.0, 5).is_none());
        assert!(decade_ticks(1.0, 10.0, 5).is_some());
    }

    #[test]
    fn minors_fill_each_decade_with_its_multiples() {
        let majors = decade_ticks(1.0, 100.0, 8).expect("two decades");
        let minors = decade_minor_values(1.0, 100.0, &majors);
        assert_eq!(minors.len(), 16);
        assert_eq!(minors[0], 2.0);
        assert_eq!(minors[8], 20.0);
        let partial = decade_minor_values(3.5, 100.0, &majors);
        assert_eq!(partial[0], 4.0);
    }

    #[test]
    fn thinned_majors_mark_the_skipped_decades_only() {
        let majors = decade_ticks(1e-6, 1e6, 5).expect("thinned");
        let minors = decade_minor_values(1e-6, 1e6, &majors);
        assert!(minors.iter().all(|value| {
            let exponent = value.log10();
            (exponent - exponent.round()).abs() < 1e-9
        }));
        assert_eq!(minors.len() + majors.values.len(), 13);
    }
}
