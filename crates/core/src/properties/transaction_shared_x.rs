//! Shared-x stacks keep one x axis: an x setting edited on any member is
//! written to every member of its stack in the same undoable commit, after the
//! same checks a direct edit of that member would pass.
use super::{AxisOverridesPlan, PropertyTransaction};
use crate::properties::{PropertyAddress, PropertyError, PropertyId, axis, plot_display};
use crate::state::{AxisOverrides, AxisRange, ObjectId, PlotObject, PlotiumApp};
use plotium_figure::AxisScale;

impl PropertyTransaction {
    /// Extend the staged axis edits to every stack peer, rejecting the whole
    /// commit when a peer cannot take a value, and report each peer edit.
    pub(crate) fn fan_out_shared_x(
        &mut self,
        app: &PlotiumApp,
        applied: &mut Vec<PropertyAddress>,
    ) -> Result<(), PropertyError> {
        let sources: Vec<_> = self
            .axis_overrides
            .iter()
            .filter(|plan| plan.before.shared_x_differs(&plan.after))
            .map(|plan| {
                (
                    plan.canvas,
                    plan.object,
                    plan.before.clone(),
                    plan.after.clone(),
                )
            })
            .collect();
        for (canvas, object, before, after) in sources {
            let Some(page) = app.doc.canvases.get(canvas) else {
                continue;
            };
            let Some(source) = page.object(object).and_then(|item| item.plot()) else {
                continue;
            };
            let source_unit = x_unit(source);
            for peer in page.shared_x_axis_peers(object) {
                if peer == object {
                    continue;
                }
                let Some(plot) = page.object(peer).and_then(|item| item.plot()) else {
                    continue;
                };
                let index = self.plan_index(canvas, peer, plot);
                let changed = copy_into(
                    app,
                    plot,
                    &source_unit,
                    &before,
                    &after,
                    &mut self.axis_overrides[index].after,
                )
                .map_err(|error| peer_error(page.object(peer).map(|o| o.name.as_str()), error))?;
                if let Some(target) = app.object_target(canvas, peer) {
                    for property in changed {
                        let address = PropertyAddress::new(target.clone(), property);
                        if !applied.contains(&address) {
                            applied.push(address);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn plan_index(&mut self, canvas: usize, object: ObjectId, plot: &PlotObject) -> usize {
        if let Some(index) = self
            .axis_overrides
            .iter()
            .position(|plan| plan.canvas == canvas && plan.object == object)
        {
            return index;
        }
        self.axis_overrides.push(AxisOverridesPlan {
            canvas,
            object,
            before: plot.axis_overrides.clone(),
            after: plot.axis_overrides.clone(),
        });
        self.axis_overrides.len() - 1
    }
}

fn x_unit(plot: &PlotObject) -> Option<plotium_data::UnitSpec> {
    plot.binding
        .coordinates
        .as_ref()
        .and_then(|c| c.horizontal().cloned())
}

fn peer_error(name: Option<&str>, error: PropertyError) -> PropertyError {
    let name = name.unwrap_or("another plot");
    match error {
        PropertyError::NotApplicable(message) => PropertyError::NotApplicable(format!(
            "Shared X Stack member \"{name}\" cannot follow this change: {message}"
        )),
        PropertyError::InvalidValue { property, message } => PropertyError::InvalidValue {
            property,
            message: format!(
                "Shared X Stack member \"{name}\" cannot follow this change: {message}"
            ),
        },
        other => other,
    }
}

/// Copy the shared x settings that changed on another member into `target`,
/// returning the properties written.
fn copy_into(
    app: &PlotiumApp,
    plot: &PlotObject,
    source_unit: &Option<plotium_data::UnitSpec>,
    before: &AxisOverrides,
    after: &AxisOverrides,
    target: &mut AxisOverrides,
) -> Result<Vec<PropertyId>, PropertyError> {
    let unit = x_unit(plot);
    let mut changed = Vec::new();
    if before.x_range != after.x_range {
        target.x_range = after
            .x_range
            .map(|range| {
                let convert = |value| {
                    crate::state::shared_x_value(value, source_unit, &unit).map_err(|message| {
                        PropertyError::InvalidValue {
                            property: plot_display::X_RANGE,
                            message,
                        }
                    })
                };
                Ok::<_, PropertyError>(AxisRange::new(convert(range.min)?, convert(range.max)?))
            })
            .transpose()?;
        changed.push(plot_display::X_RANGE);
    }
    if before.x_scale != after.x_scale {
        if after.x_scale == Some(AxisScale::Log10)
            && let Some(reason) = axis::scale_unavailable(app, axis::X_SCALE, plot)
        {
            return Err(PropertyError::NotApplicable(reason.to_owned()));
        }
        target.x_scale = after.x_scale;
        changed.push(axis::X_SCALE);
    }
    if target.x_scale == Some(AxisScale::Log10)
        && target.x_range.is_some_and(|range| range.min <= 0.0)
    {
        return Err(PropertyError::InvalidValue {
            property: axis::X_SCALE,
            message: axis::NON_POSITIVE_LOG_RANGE.to_owned(),
        });
    }
    if before.x_display_unit != after.x_display_unit {
        if let Some(display) = &after.x_display_unit {
            let shown = Some(display.as_ref().clone());
            if unit.is_none()
                || crate::state::convert_curve_value(1.0, &unit, &shown, false).is_err()
            {
                return Err(PropertyError::InvalidValue {
                    property: plot_display::X_UNIT,
                    message: "Display unit is incompatible with the plot coordinate unit.".into(),
                });
            }
        }
        target.x_display_unit.clone_from(&after.x_display_unit);
        changed.push(plot_display::X_UNIT);
    }
    if before.x_thousands_separator != after.x_thousands_separator {
        target.x_thousands_separator = after.x_thousands_separator;
        changed.push(axis::X_THOUSANDS_SEPARATOR);
    }
    Ok(changed)
}
