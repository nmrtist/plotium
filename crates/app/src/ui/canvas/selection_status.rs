use plotium_core::state::{AxisRange, Dataset};

pub(super) fn range_status(dataset: &Dataset, x: AxisRange) -> String {
    if let Some(context) = dataset.as_afm().and_then(|afm| afm.force_curve_context()) {
        let unit = if context.x.unit.is_empty() {
            String::new()
        } else {
            format!(" {}", context.x.unit)
        };
        return format!(
            "Selected {}: {:.3} to {:.3}{unit}.",
            context.x.name, x.min, x.max
        );
    }
    let unit = if dataset.as_mass_spec().is_some() {
        "min"
    } else {
        "ppm"
    };
    format!("Selected {:.3}-{:.3} {unit}.", x.min, x.max)
}
