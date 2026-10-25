//! Shared-x stacks keep one x axis: an x setting edited on any member is
//! written to every member of its stack in the same undoable commit, after the
//! same checks a direct edit of that member would pass.
use super::{AxisOverridesPlan, PropertyTransaction};
use crate::properties::{PropertyAddress, PropertyError, PropertyId, axis, plot_display};
use crate::state::{AxisOverrides, AxisRange, ObjectId, PlotObject, PlotiumApp};
use plotium_figure::AxisScale;

impl PropertyTransaction {
    pub(crate) fn record_shared_x_edit(&mut self, address: &PropertyAddress) {
        if matches!(
            address.definition,
            plot_display::X_RANGE
                | plot_display::X_BREAK
                | plot_display::X_BREAK_FRACTION
                | plot_display::X_UNIT
                | axis::X_SCALE
                | axis::X_THOUSANDS_SEPARATOR
        ) && !self.shared_x_edits.contains(address)
        {
            self.shared_x_edits.push(address.clone());
        }
    }

    /// Extend the staged axis edits to every stack peer, rejecting the whole
    /// commit when a peer cannot take a value, and report each peer edit.
    pub(crate) fn fan_out_shared_x(
        &mut self,
        app: &PlotiumApp,
        applied: &mut Vec<PropertyAddress>,
    ) -> Result<(), PropertyError> {
        let requests: Vec<_> = self
            .axis_overrides
            .iter()
            .map(|plan| {
                (
                    plan.canvas,
                    plan.object,
                    plan.before.clone(),
                    plan.after.clone(),
                )
            })
            .collect();
        for (canvas, object, before, after) in &requests {
            if !before.shared_x_differs(after) {
                continue;
            }
            let (canvas, object) = (*canvas, *object);
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
                    before,
                    after,
                    &mut self.axis_overrides[index].after,
                )
                .map_err(|error| peer_error(page.object(peer).map(|o| o.name.as_str()), error))?;
                if let Some(target) = app.object_target(canvas, peer) {
                    if let Some((_, _, _, requested)) = requests
                        .iter()
                        .find(|(c, o, _, _)| *c == canvas && *o == peer)
                    {
                        let proposed = &mut self.axis_overrides[index].after;
                        for address in self.shared_x_edits.iter().filter(|a| a.target == target) {
                            if !compatible(
                                address.definition,
                                requested,
                                proposed,
                                source_unit != x_unit(plot),
                            ) {
                                return Err(peer_error(
                                    page.object(peer).map(|o| o.name.as_str()),
                                    PropertyError::InvalidValue {
                                        property: address.definition,
                                        message:
                                            "Conflicting explicit settings for one shared X axis."
                                                .into(),
                                    },
                                ));
                            }
                            // Keep the exact authored value after allowing conversion roundoff.
                            preserve(address.definition, requested, proposed);
                        }
                    }
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

fn compatible(property: PropertyId, a: &AxisOverrides, b: &AxisOverrides, converted: bool) -> bool {
    // Only unit conversion may introduce roundoff; distinct native values conflict.
    let close =
        |a: f64, b: f64| a == b || (converted && (a - b).abs() <= 1e-12 * a.abs().max(b.abs()));
    match property {
        plot_display::X_RANGE => match (a.x_range, b.x_range) {
            (Some(a), Some(b)) => close(a.min, b.min) && close(a.max, b.max),
            (a, b) => a == b,
        },
        plot_display::X_BREAK => match (a.x_break, b.x_break) {
            (Some(a), Some(b)) => close(a.from, b.from) && close(a.to, b.to),
            (a, b) => a == b,
        },
        plot_display::X_BREAK_FRACTION => {
            a.x_break.map(|b| b.lower_fraction) == b.x_break.map(|b| b.lower_fraction)
        }
        plot_display::X_UNIT => a.x_display_unit == b.x_display_unit,
        axis::X_SCALE => a.x_scale == b.x_scale,
        axis::X_THOUSANDS_SEPARATOR => a.x_thousands_separator == b.x_thousands_separator,
        _ => true,
    }
}

fn preserve(property: PropertyId, from: &AxisOverrides, to: &mut AxisOverrides) {
    match property {
        plot_display::X_RANGE => to.x_range = from.x_range,
        plot_display::X_BREAK => {
            let fraction = to.x_break.and_then(|b| b.lower_fraction);
            to.x_break = from.x_break.map(|b| plotium_figure::AxisBreak {
                lower_fraction: fraction,
                ..b
            });
        }
        plot_display::X_BREAK_FRACTION => {
            if let Some(brk) = &mut to.x_break {
                brk.lower_fraction = from.x_break.and_then(|b| b.lower_fraction);
            }
        }
        plot_display::X_UNIT => to.x_display_unit.clone_from(&from.x_display_unit),
        axis::X_SCALE => to.x_scale = from.x_scale,
        axis::X_THOUSANDS_SEPARATOR => to.x_thousands_separator = from.x_thousands_separator,
        _ => {}
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
    if before.x_break != after.x_break {
        let endpoints_changed =
            before.x_break.map(|b| (b.from, b.to)) != after.x_break.map(|b| (b.from, b.to));
        let fraction_changed =
            before.x_break.map(|b| b.lower_fraction) != after.x_break.map(|b| b.lower_fraction);
        let converted = after
            .x_break
            .map(|brk| {
                let convert = |value| {
                    crate::state::shared_x_value(value, source_unit, &unit).map_err(|message| {
                        PropertyError::InvalidValue {
                            property: plot_display::X_BREAK,
                            message,
                        }
                    })
                };
                Ok::<_, PropertyError>(plotium_figure::AxisBreak {
                    from: convert(brk.from)?,
                    to: convert(brk.to)?,
                    ..brk
                })
            })
            .transpose()?;
        if endpoints_changed {
            let fraction = target.x_break.and_then(|b| b.lower_fraction);
            target.x_break = converted.map(|brk| plotium_figure::AxisBreak {
                lower_fraction: if fraction_changed {
                    brk.lower_fraction
                } else {
                    fraction
                },
                ..brk
            });
            changed.push(plot_display::X_BREAK);
        } else if let Some(brk) = &mut target.x_break {
            brk.lower_fraction = converted.and_then(|b| b.lower_fraction);
        }
        if fraction_changed {
            changed.push(plot_display::X_BREAK_FRACTION);
        }
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
    if before.x_major_step != after.x_major_step {
        let shown = |display: &Option<Box<plotium_data::UnitSpec>>, unit| {
            display.as_deref().cloned().or_else(|| Option::clone(unit))
        };
        target.x_major_step = crate::state::shared_x_step(
            after.x_major_step,
            &shown(&after.x_display_unit, source_unit),
            &shown(&target.x_display_unit, &unit),
        )
        .map_err(|message| PropertyError::InvalidValue {
            property: axis::X_MAJOR_STEP,
            message,
        })?;
        changed.push(axis::X_MAJOR_STEP);
    }
    Ok(changed)
}
