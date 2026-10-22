use super::{AxisOverrides, CurveCoordinates, DerivedAxes};
use plotium_data::UnitSpec;
use plotium_figure::{AxisDisplay, Figure};

pub(super) fn apply_display(
    figure: &mut Figure,
    coordinates: Option<&CurveCoordinates>,
    axes: &DerivedAxes,
    overrides: &AxisOverrides,
) {
    let axes_to_project = [
        (
            Some(&mut figure.x),
            coordinates.and_then(|c| c.x.as_ref()),
            overrides.x_display_unit.as_deref(),
            &axes.x_label,
            overrides.x_label.as_ref(),
        ),
        (
            Some(&mut figure.y),
            coordinates.and_then(|c| c.y.as_ref()),
            overrides.y_display_unit.as_deref(),
            &axes.y_label,
            overrides.y_label.as_ref(),
        ),
        (
            figure.y2.as_mut(),
            coordinates.and_then(|c| c.y2.as_ref()),
            overrides.y2_display_unit.as_deref(),
            &axes.y2_label,
            overrides.y2_label.as_ref(),
        ),
    ];
    for (axis, source, target, derived, authored) in axes_to_project {
        let Some(axis) = axis else { continue };
        let had_display = axis.display.is_some();
        axis.display = match (source, target) {
            (Some(from), Some(to)) if from.is_compatible_with(to) => Some(projection(from, to)),
            _ => None,
        };
        if authored.is_none() && (target.is_some() || had_display) {
            axis.label = display_label(derived, source, target);
        }
    }
}
fn projection(from: &UnitSpec, to: &UnitSpec) -> AxisDisplay {
    AxisDisplay {
        scale: from.scale / to.scale,
        offset: (from.offset - to.offset) / to.scale,
        unit: to.display_unit.clone(),
    }
}
pub(super) fn display_label(label: &str, from: Option<&UnitSpec>, to: Option<&UnitSpec>) -> String {
    match (from, to) {
        (Some(from), Some(to)) => label
            .strip_suffix(&format!(" ({})", from.display_unit))
            .map_or_else(
                || label.to_owned(),
                |base| format!("{base} ({})", to.display_unit),
            ),
        _ => label.to_owned(),
    }
}

pub(crate) fn validate_display(
    overrides: &AxisOverrides,
    coordinates: Option<&CurveCoordinates>,
    figure: &Figure,
) -> Result<(), String> {
    for (source, target, axis, range) in [
        (
            coordinates.and_then(|c| c.x.as_ref()),
            overrides.x_display_unit.as_deref(),
            Some(&figure.x),
            overrides.x_range,
        ),
        (
            coordinates.and_then(|c| c.y.as_ref()),
            overrides.y_display_unit.as_deref(),
            Some(&figure.y),
            overrides.y_range,
        ),
        (
            coordinates.and_then(|c| c.y2.as_ref()),
            overrides.y2_display_unit.as_deref(),
            figure.y2.as_ref(),
            overrides.y2_range,
        ),
    ] {
        // Right-axis overrides stay dormant while no curve uses that axis.
        let Some(axis) = axis else { continue };
        if let Some(target) = target {
            target.validate().map_err(|e| e.to_string())?;
            let source = source.ok_or("Display units require known plot coordinate units.")?;
            if !source.is_compatible_with(target) {
                return Err("Display unit is incompatible with the plot coordinates.".into());
            }
            let display = projection(source, target);
            let (min, max) = range.map_or((axis.min, axis.max), |r| (r.min, r.max));
            let span = display.to_display(max) - display.to_display(min);
            if !display.scale.is_finite()
                || display.scale <= 0.
                || !display.offset.is_finite()
                || !display.to_display(min).is_finite()
                || !display.to_display(max).is_finite()
                || !span.is_finite()
                || span <= 0.
            {
                return Err("Axis display conversion overflows or collapses its range.".into());
            }
        }
    }
    for size in [
        overrides.typography.tick_pt,
        overrides.typography.label_pt,
        overrides.typography.legend_pt,
    ]
    .into_iter()
    .flatten()
    {
        if !(1.0..=72.0).contains(&size.get()) {
            return Err("Plot font sizes must be between 1 and 72 pt.".into());
        }
    }
    Ok(())
}
