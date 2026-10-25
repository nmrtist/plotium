//! Right y axis title, visibility, scale, range, display unit and major tick
//! step. Every property applies only while some curve of the plot is on the
//! right axis.
use super::provider::PropertyProvider;
use super::target::require_plot_object_target;
use super::*;
use crate::state::{AxisOverrides, AxisRange, PlotObject, PlotiumApp, convert_curve_value};
use plotium_data::{UnitRegistry, UnitSpec};
use plotium_figure::AxisScale;

pub const Y2_LABEL: PropertyId = PropertyId("object.axes.y2_label");
pub const Y2_SHOW_TICK_LABELS: PropertyId = PropertyId("object.axes.y2_show_tick_labels");
pub const Y2_SHOW_LABEL: PropertyId = PropertyId("object.axes.y2_show_label");
pub const Y2_SCALE: PropertyId = PropertyId("object.axes.y2_scale");
pub const Y2_RANGE: PropertyId = PropertyId("object.axes.y2_range");
pub const Y2_UNIT: PropertyId = PropertyId("object.axes.y2_display_unit");
pub const Y2_MAJOR_STEP: PropertyId = PropertyId("object.axes.y2_major_step");

const SCALES: &[EnumVariant] = &[
    EnumVariant::new(super::axis::LINEAR, "Linear"),
    EnumVariant::new(super::axis::LOG, "Logarithmic"),
];

const fn def(
    id: PropertyId,
    schema: ValueSchema,
    label: &'static str,
    aliases: &'static [&'static str],
    tier: Tier,
) -> PropertyDefinition {
    PropertyDefinition {
        id,
        scope_kind: ScopeKind::Object,
        value_schema: schema,
        access: PropertyAccess::ReadWrite,
        applicability: Applicability::component(ComponentKind::None),
        default_policy: DefaultPolicy::Derived,
        tier,
        copies: ValueCopies::PerTarget,
        canonical_label: label,
        canonical_aliases: aliases,
    }
}

const DEFINITIONS: &[PropertyDefinition] = &[
    def(
        Y2_LABEL,
        ValueSchema::Text,
        "Right y-axis title",
        &["secondary y label", "y2 label"],
        Tier::Essential,
    ),
    def(
        Y2_SHOW_TICK_LABELS,
        ValueSchema::Bool,
        "Show right y-axis tick labels",
        &["y2 ticks", "right tick labels"],
        Tier::Essential,
    ),
    def(
        Y2_SHOW_LABEL,
        ValueSchema::Bool,
        "Show right y-axis title",
        &["y2 title visibility"],
        Tier::Advanced,
    ),
    def(
        Y2_SCALE,
        ValueSchema::Enum { variants: SCALES },
        "Right y-axis scale",
        &["log y2", "right log scale"],
        Tier::Advanced,
    ),
    def(
        Y2_RANGE,
        ValueSchema::AxisRange,
        "Right y range",
        &["y2 range", "secondary y range"],
        Tier::Advanced,
    ),
    def(
        Y2_UNIT,
        ValueSchema::UnitCode,
        "Right y display unit",
        &["y2 unit"],
        Tier::Advanced,
    ),
    def(
        Y2_MAJOR_STEP,
        super::axis::MAJOR_STEP_SCHEMA,
        "Right y major tick step",
        &["y2 tick spacing", "right tick interval"],
        Tier::Advanced,
    ),
];

pub(crate) struct RightAxisProvider;
pub(crate) static PROVIDER: RightAxisProvider = RightAxisProvider;

fn error(id: PropertyId, message: impl ToString) -> PropertyError {
    PropertyError::InvalidValue {
        property: id,
        message: message.to_string(),
    }
}

fn coordinate_unit(plot: &PlotObject) -> Option<UnitSpec> {
    plot.binding.coordinates.as_ref().and_then(|c| c.y2.clone())
}

fn shown_unit(plot: &PlotObject) -> Option<UnitSpec> {
    plot.axis_overrides
        .y2_display_unit
        .as_deref()
        .cloned()
        .or_else(|| coordinate_unit(plot))
}

fn scale_value(scale: AxisScale) -> PropertyValue {
    PropertyValue::Enum(match scale {
        AxisScale::Linear => super::axis::LINEAR,
        AxisScale::Log10 => super::axis::LOG,
    })
}

/// The plot of a right-axis property target, which must draw a right axis.
fn right_axis_plot<'a>(
    app: &'a PlotiumApp,
    address: &PropertyAddress,
) -> Result<
    (
        usize,
        crate::state::ObjectId,
        &'a PlotObject,
        &'static PropertyDefinition,
    ),
    PropertyError,
> {
    let d = definition(address.definition)
        .ok_or_else(|| PropertyError::UnknownProperty(address.definition.to_string()))?;
    let (canvas, object) = require_plot_object_target(app, &address.target, d)?;
    let plot = app.doc.canvases[canvas]
        .object(object)
        .and_then(|o| o.plot())
        .ok_or_else(|| PropertyError::UnknownTarget(address.target.describe()))?;
    if plot.figure().y2.is_none() {
        return Err(PropertyError::NotApplicable(
            crate::state::right_axis::NO_RIGHT_AXIS.into(),
        ));
    }
    Ok((canvas, object, plot, d))
}

impl PropertyProvider for RightAxisProvider {
    fn definitions(&self) -> &'static [PropertyDefinition] {
        DEFINITIONS
    }

    fn read(
        &self,
        app: &PlotiumApp,
        address: &PropertyAddress,
    ) -> Result<ResolvedProperty, PropertyError> {
        let (_, _, plot, d) = right_axis_plot(app, address)?;
        let axis = plot.figure().y2.as_ref().expect("checked right axis");
        let overrides = &plot.axis_overrides;
        let derived = plot.derived_axes();
        let coordinate = coordinate_unit(plot);
        let shown = shown_unit(plot);
        let code = |u: &Option<UnitSpec>| u.as_ref().map(|u| u.display_unit.clone());
        let convert = |v: f64| convert_curve_value(v, &coordinate, &shown, false);
        let mut availability = Availability::Editable;
        let mut schema = None;
        let (value, default_value, modified) = match d.id {
            Y2_LABEL => (
                PropertyValue::Text(axis.label.clone()),
                PropertyValue::Text(plot.automatic_right_axis_label()),
                overrides.y2_label.is_some(),
            ),
            Y2_SHOW_TICK_LABELS => (
                PropertyValue::Bool(axis.show_tick_labels),
                PropertyValue::Bool(derived.y2_show_tick_labels),
                overrides.y2_show_tick_labels.is_some(),
            ),
            Y2_SHOW_LABEL => (
                PropertyValue::Bool(axis.show_label),
                PropertyValue::Bool(derived.y2_show_label),
                overrides.y2_show_label.is_some(),
            ),
            Y2_SCALE => {
                if let Some(reason) = crate::state::right_log_scale_unavailable(plot.figure()) {
                    availability = Availability::Disabled(reason);
                }
                (
                    scale_value(axis.scale),
                    scale_value(derived.y2_scale),
                    overrides.y2_scale.is_some(),
                )
            }
            Y2_RANGE => {
                let value = match overrides.y2_range {
                    Some(r) => AxisRangeValue::Manual {
                        min: convert(r.min).map_err(|e| error(d.id, e))?,
                        max: convert(r.max).map_err(|e| error(d.id, e))?,
                        unit: code(&shown),
                    },
                    None => AxisRangeValue::Auto,
                };
                let view = plot.viewport.view_y2.unwrap_or(AxisRange::from_axis(axis));
                schema = Some(ResolvedSchema::AxisRange {
                    manual_allowed: true,
                    unit: code(&shown),
                    view: [
                        convert(view.min).map_err(|e| error(d.id, e))?,
                        convert(view.max).map_err(|e| error(d.id, e))?,
                    ],
                });
                (
                    PropertyValue::AxisRange(value),
                    PropertyValue::AxisRange(AxisRangeValue::Auto),
                    overrides.y2_range.is_some(),
                )
            }
            Y2_UNIT => {
                let mut codes: Vec<String> = coordinate
                    .as_ref()
                    .map(|u| {
                        UnitRegistry::plotium_v1()
                            .compatible_codes(u)
                            .into_iter()
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default();
                if let Some(u) = &coordinate
                    && !codes.contains(&u.display_unit)
                {
                    codes.push(u.display_unit.clone());
                }
                schema = Some(ResolvedSchema::UnitCode { codes });
                if coordinate.is_none() {
                    availability = Availability::Disabled(
                        "Choose a right axis with a known coordinate unit to change its display unit.",
                    );
                }
                (
                    PropertyValue::Text(code(&shown).unwrap_or_default()),
                    PropertyValue::Text(code(&coordinate).unwrap_or_default()),
                    overrides.y2_display_unit.is_some(),
                )
            }
            Y2_MAJOR_STEP => {
                if let Some(reason) = super::axis::major_step_unavailable(plot.figure(), axis) {
                    availability = Availability::Disabled(reason);
                }
                (
                    super::axis::major_step_value(axis.major_step),
                    super::axis::major_step_value(None),
                    overrides.y2_major_step.is_some(),
                )
            }
            _ => return Err(PropertyError::UnknownProperty(d.id.to_string())),
        };
        Ok(ResolvedProperty {
            address: address.clone(),
            value: AggregateValue::Uniform(value),
            default_value: Some(default_value),
            modified: Some(modified),
            availability,
            schema: schema.unwrap_or_else(|| {
                super::target::resolved_schema(d, &crate::state::FieldCapabilities::default())
            }),
        })
    }

    fn edit(
        &self,
        app: &PlotiumApp,
        tx: &mut PropertyTransaction,
        address: &PropertyAddress,
        op: &EditOp<'_>,
    ) -> Result<(), PropertyError> {
        let (canvas, object, plot, d) = right_axis_plot(app, address)?;
        let resolved = self.read(app, address)?;
        let reset = matches!(op, EditOp::Reset);
        let value = match op {
            EditOp::Set(v) => (*v).clone(),
            EditOp::Reset => resolved
                .default_value
                .ok_or_else(|| error(d.id, "No default"))?,
            EditOp::Step(_) => {
                return Err(error(d.id, "Right-axis settings have no step gesture."));
            }
        };
        if !reset && let Availability::Disabled(reason) = resolved.availability {
            return Err(PropertyError::NotApplicable(reason.into()));
        }
        let coordinate = coordinate_unit(plot);
        let shown = shown_unit(plot);
        let overrides = tx.axis_overrides(app, canvas, object)?;
        match (d.id, value) {
            (Y2_LABEL, PropertyValue::Text(text)) => {
                overrides.y2_label = (!reset && !text.trim().is_empty()).then_some(text);
            }
            (Y2_SHOW_TICK_LABELS, PropertyValue::Bool(show)) => {
                overrides.y2_show_tick_labels = (!reset).then_some(show);
            }
            (Y2_SHOW_LABEL, PropertyValue::Bool(show)) => {
                overrides.y2_show_label = (!reset).then_some(show);
            }
            (Y2_SCALE, PropertyValue::Enum(_)) if reset => overrides.y2_scale = None,
            (Y2_SCALE, PropertyValue::Enum(super::axis::LINEAR)) => {
                overrides.y2_scale = Some(AxisScale::Linear);
            }
            (Y2_SCALE, PropertyValue::Enum(super::axis::LOG)) => {
                if overrides.y2_range.is_some_and(|range| range.min <= 0.0) {
                    return Err(error(d.id, super::axis::NON_POSITIVE_LOG_RANGE));
                }
                overrides.y2_scale = Some(AxisScale::Log10);
            }
            (Y2_RANGE, PropertyValue::AxisRange(value)) => {
                overrides.y2_range = manual_range(d.id, value, &coordinate, &shown, overrides)?;
            }
            (Y2_MAJOR_STEP, PropertyValue::Float(step)) => {
                super::axis::checked_major_step(d.id, step)?;
                overrides.y2_major_step = (!reset && step > 0.0).then_some(step);
            }
            (Y2_UNIT, PropertyValue::Text(code)) => {
                let before = overrides.y2_display_unit.clone();
                overrides.y2_display_unit = if reset {
                    None
                } else {
                    let source = coordinate
                        .as_ref()
                        .ok_or_else(|| error(d.id, "Right-axis coordinate units are unknown."))?;
                    let target = super::plot_display::resolve_unit(&code, &coordinate, &shown)
                        .map_err(|e| error(d.id, e))?;
                    if !source.is_compatible_with(&target) {
                        return Err(error(
                            d.id,
                            "Display unit is incompatible with the right-axis coordinate unit.",
                        ));
                    }
                    Some(Box::new(target))
                };
                // A major step is written in the display unit it replaces.
                if overrides.y2_display_unit != before {
                    overrides.y2_major_step = None;
                }
            }
            _ => return Err(error(d.id, "Value does not match the property schema.")),
        }
        crate::state::axis_display::validate_display(
            overrides,
            plot.binding.coordinates.as_ref(),
            plot.figure(),
        )
        .map_err(|e| error(d.id, e))
    }
}

/// A manual right-axis range in coordinate units, or `None` for Auto.
fn manual_range(
    id: PropertyId,
    value: AxisRangeValue,
    coordinate: &Option<UnitSpec>,
    shown: &Option<UnitSpec>,
    overrides: &AxisOverrides,
) -> Result<Option<AxisRange>, PropertyError> {
    let AxisRangeValue::Manual { min, max, unit } = value else {
        return Ok(None);
    };
    if !min.is_finite() || !max.is_finite() || min >= max {
        return Err(error(id, "Range endpoints must be finite with min < max."));
    }
    let from = unit
        .map(|u| super::plot_display::resolve_unit(&u, coordinate, shown))
        .transpose()
        .map_err(|e| error(id, e))?;
    let range = AxisRange {
        min: convert_curve_value(min, &from, coordinate, false).map_err(|e| error(id, e))?,
        max: convert_curve_value(max, &from, coordinate, false).map_err(|e| error(id, e))?,
    };
    if !range.is_valid() || !range.span().is_finite() {
        return Err(error(id, "Converted range is degenerate or inverted."));
    }
    if overrides.y2_scale == Some(AxisScale::Log10) && range.min <= 0.0 {
        return Err(error(id, super::axis::NON_POSITIVE_LOG_RANGE));
    }
    Ok(Some(range))
}
