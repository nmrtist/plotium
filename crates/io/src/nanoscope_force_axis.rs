use super::*;

pub(super) fn standalone_force_axis(
    section: &Section,
    globals: &HashMap<String, String>,
    samples: usize,
) -> Result<ForceAxis, IoError> {
    // NanoScope lists retract then extend counts, but stores extend first.
    let approach = if let Some([retract, extend]) = section
        .value(&["Samps/line", "Samples/line"])
        .or_else(|| global_value(globals, &["Samps/line"]))
        .and_then(|value| {
            let mut counts = value.split_whitespace();
            Some([
                counts.next()?.parse::<usize>().ok()?,
                counts.next()?.parse().ok()?,
            ])
        }) {
        if extend.checked_add(retract) != Some(samples) {
            return Err(invalid(
                "force branch sample counts do not match the data block",
            ));
        }
        extend
    } else {
        samples / 2
    };
    let z_positions = section
        .value(&["X Data Type"])
        .filter(|axis| axis.eq_ignore_ascii_case("Ramp"))
        .and_then(|_| section.value(&["Ramp Size"]))
        .and_then(|value| {
            let (magnitude, unit) = calibration::display_quantity(value)?;
            let nm = if unit.eq_ignore_ascii_case("V") {
                let reference = bracketed(value)?;
                let (sensitivity, length_unit) =
                    sensitivity_scale(globals.get(&reference.to_ascii_lowercase())?)?;
                magnitude * sensitivity * calibration::length_factor_nm(&length_unit)?
            } else {
                magnitude * calibration::length_factor_nm(unit)?
            };
            let nm = nm.abs();
            (nm.is_finite() && nm > 0.0).then(|| {
                // Both branches are stored closest first. This is relative Z,
                // not tip-sample separation or indentation.
                [approach, samples - approach]
                    .into_iter()
                    .flat_map(|count| {
                        (0..count)
                            .map(move |i| nm * (i as f64 / count.saturating_sub(1).max(1) as f64))
                    })
                    .collect()
            })
        });
    Ok(((0..samples).collect(), approach, z_positions))
}
