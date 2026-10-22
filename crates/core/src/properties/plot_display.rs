//! Plot-local display units, range intent, and inherited typography.
use super::provider::PropertyProvider;
use super::target::require_plot_object_target;
use super::*;
use crate::state::{AxisRange, PlotObject, PlotiumApp, convert_curve_value};
use plotium_data::{UnitRegistry, UnitSpec};
use plotium_figure::PositiveFiniteF32;

pub const X_UNIT: PropertyId = PropertyId("object.axes.x_display_unit");
pub const Y_UNIT: PropertyId = PropertyId("object.axes.y_display_unit");
pub const X_RANGE: PropertyId = PropertyId("object.axes.x_range");
pub const Y_RANGE: PropertyId = PropertyId("object.axes.y_range");
pub const TICK_PT: PropertyId = PropertyId("object.figure.typography.tick_pt");
pub const LABEL_PT: PropertyId = PropertyId("object.figure.typography.label_pt");
pub const LEGEND_PT: PropertyId = PropertyId("object.figure.typography.legend_pt");
const fn def(id: PropertyId, schema: ValueSchema, label: &'static str) -> PropertyDefinition {
    PropertyDefinition {
        id,
        scope_kind: ScopeKind::Object,
        value_schema: schema,
        access: PropertyAccess::ReadWrite,
        applicability: Applicability::component(ComponentKind::None),
        default_policy: DefaultPolicy::Derived,
        tier: Tier::Advanced,
        copies: ValueCopies::PerTarget,
        canonical_label: label,
        canonical_aliases: &[],
    }
}
const fn font(id: PropertyId, label: &'static str) -> PropertyDefinition {
    def(
        id,
        ValueSchema::Float {
            bounds: FloatBounds::inclusive(1., 72.),
            display: FloatDisplay::Linear("pt"),
            drag_step: Some(0.25),
        },
        label,
    )
}
const DEFINITIONS: &[PropertyDefinition] = &[
    def(X_UNIT, ValueSchema::UnitCode, "X display unit"),
    def(Y_UNIT, ValueSchema::UnitCode, "Y display unit"),
    def(X_RANGE, ValueSchema::AxisRange, "X range"),
    def(Y_RANGE, ValueSchema::AxisRange, "Y range"),
    font(TICK_PT, "Plot tick-label size"),
    font(LABEL_PT, "Plot axis-title size"),
    font(LEGEND_PT, "Plot legend size"),
];
pub(crate) struct PlotDisplayProvider;
pub(crate) static PROVIDER: PlotDisplayProvider = PlotDisplayProvider;
fn is_x(id: PropertyId) -> bool {
    matches!(id, X_UNIT | X_RANGE)
}
fn coordinate_unit(plot: &PlotObject, x: bool) -> Option<UnitSpec> {
    plot.binding
        .coordinates
        .as_ref()
        .and_then(|c| if x { c.x.clone() } else { c.y.clone() })
}
fn shown_unit(plot: &PlotObject, x: bool) -> Option<UnitSpec> {
    (if x {
        &plot.axis_overrides.x_display_unit
    } else {
        &plot.axis_overrides.y_display_unit
    })
    .as_deref()
    .cloned()
    .or_else(|| coordinate_unit(plot, x))
}
fn error(id: PropertyId, message: impl ToString) -> PropertyError {
    PropertyError::InvalidValue {
        property: id,
        message: message.to_string(),
    }
}
// An existing explicit coordinate/display UnitSpec is authoritative even for
// domain units outside the built-in conversion registry.
pub(super) fn resolve_unit(
    code: &str,
    coordinate: &Option<UnitSpec>,
    shown: &Option<UnitSpec>,
) -> Result<UnitSpec, String> {
    coordinate
        .iter()
        .chain(shown.iter())
        .find(|u| u.display_unit == code)
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| {
            UnitRegistry::plotium_v1()
                .resolve(code)
                .map_err(|e| e.to_string())
        })
}
impl PropertyProvider for PlotDisplayProvider {
    fn definitions(&self) -> &'static [PropertyDefinition] {
        DEFINITIONS
    }
    fn read(
        &self,
        app: &PlotiumApp,
        address: &PropertyAddress,
    ) -> Result<ResolvedProperty, PropertyError> {
        let d = definition(address.definition)
            .ok_or_else(|| PropertyError::UnknownProperty(address.definition.to_string()))?;
        let (canvas, object) = require_plot_object_target(app, &address.target, d)?;
        let plot = app.doc.canvases[canvas]
            .object(object)
            .and_then(|o| o.plot())
            .ok_or_else(|| PropertyError::UnknownTarget(address.target.describe()))?;
        let x = is_x(d.id);
        let coordinate = coordinate_unit(plot, x);
        let shown = shown_unit(plot, x);
        let unit_code = |u: &Option<UnitSpec>| u.as_ref().map(|u| u.display_unit.clone());
        let range = if x {
            plot.axis_overrides.x_range
        } else {
            plot.axis_overrides.y_range
        };
        let axis = if x {
            &plot.figure().x
        } else {
            &plot.figure().y
        };
        let numeric = axis.categories.is_none()
            && plot.figure().axis_frame != plotium_figure::AxisFrame::Hidden;
        let ty = plot.axis_overrides.typography;
        let defaults = app.doc.style_library.figure_typography;
        let (value, default_value, modified, schema, availability) = match d.id {
            X_UNIT | Y_UNIT => {
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
                (
                    PropertyValue::Text(unit_code(&shown).unwrap_or_default()),
                    PropertyValue::Text(unit_code(&coordinate).unwrap_or_default()),
                    (if x {
                        &plot.axis_overrides.x_display_unit
                    } else {
                        &plot.axis_overrides.y_display_unit
                    })
                    .is_some(),
                    ResolvedSchema::UnitCode { codes },
                    if numeric && coordinate.is_some() {
                        Availability::Editable
                    } else {
                        Availability::Disabled(
                            "Choose a numeric axis with a known coordinate unit to change its display unit.",
                        )
                    },
                )
            }
            X_RANGE | Y_RANGE => {
                let value = match range {
                    Some(r) => AxisRangeValue::Manual {
                        min: convert_curve_value(r.min, &coordinate, &shown, false)
                            .map_err(|e| error(d.id, e))?,
                        max: convert_curve_value(r.max, &coordinate, &shown, false)
                            .map_err(|e| error(d.id, e))?,
                        unit: unit_code(&shown),
                    },
                    None => AxisRangeValue::Auto,
                };
                (
                    PropertyValue::AxisRange(value),
                    PropertyValue::AxisRange(AxisRangeValue::Auto),
                    range.is_some(),
                    ResolvedSchema::AxisRange {
                        manual_allowed: numeric,
                        unit: unit_code(&shown),
                        view: {
                            let v = if x {
                                plot.viewport.view_x
                            } else {
                                plot.viewport.view_y
                            };
                            [
                                convert_curve_value(v.min, &coordinate, &shown, false)
                                    .map_err(|e| error(d.id, e))?,
                                convert_curve_value(v.max, &coordinate, &shown, false)
                                    .map_err(|e| error(d.id, e))?,
                            ]
                        },
                    },
                    if numeric || range.is_some() {
                        Availability::Editable
                    } else {
                        Availability::Disabled(
                            "Choose a chart with numeric visible axes to set its range.",
                        )
                    },
                )
            }
            TICK_PT | LABEL_PT | LEGEND_PT => {
                let (v, default) = match d.id {
                    TICK_PT => (ty.tick_pt, defaults.tick_pt),
                    LABEL_PT => (ty.label_pt, defaults.label_pt),
                    _ => (ty.legend_pt, defaults.legend_pt),
                };
                (
                    PropertyValue::Float(f64::from(v.map_or(default, |v| v.get()))),
                    PropertyValue::Float(f64::from(default)),
                    v.is_some(),
                    ResolvedSchema::Float {
                        bounds: FloatBounds::inclusive(1., 72.),
                        display: FloatDisplay::Linear("pt"),
                    },
                    Availability::Editable,
                )
            }
            _ => return Err(PropertyError::UnknownProperty(d.id.to_string())),
        };
        Ok(ResolvedProperty {
            address: address.clone(),
            value: AggregateValue::Uniform(value),
            default_value: Some(default_value),
            modified: Some(modified),
            schema,
            availability,
        })
    }
    fn edit(
        &self,
        app: &PlotiumApp,
        tx: &mut PropertyTransaction,
        address: &PropertyAddress,
        op: &EditOp<'_>,
    ) -> Result<(), PropertyError> {
        let d = definition(address.definition)
            .ok_or_else(|| PropertyError::UnknownProperty(address.definition.to_string()))?;
        let (canvas, object) = require_plot_object_target(app, &address.target, d)?;
        let plot = app.doc.canvases[canvas]
            .object(object)
            .and_then(|o| o.plot())
            .ok_or_else(|| PropertyError::UnknownTarget(address.target.describe()))?;
        let resolved = self.read(app, address)?;
        let reset = matches!(op, EditOp::Reset);
        let value = match op {
            EditOp::Set(v) => (*v).clone(),
            EditOp::Reset => resolved
                .default_value
                .ok_or_else(|| error(d.id, "No default"))?,
            EditOp::Step(_) => return Err(error(d.id, "No step gesture")),
        };
        // A retained inactive range can always be cleared, never manually set.
        let clear_range = matches!(&value, PropertyValue::AxisRange(AxisRangeValue::Auto));
        if !reset
            && !clear_range
            && let Availability::Disabled(reason) = resolved.availability
        {
            return Err(PropertyError::NotApplicable(reason.into()));
        }
        let x = is_x(d.id);
        let coordinate = coordinate_unit(plot, x);
        let overrides = tx.axis_overrides(app, canvas, object)?;
        match (d.id, value) {
            (X_UNIT | Y_UNIT, PropertyValue::Text(code)) => {
                let selected = if reset {
                    None
                } else {
                    let source = coordinate
                        .as_ref()
                        .ok_or_else(|| error(d.id, "Coordinate units are unknown."))?;
                    let target = resolve_unit(&code, &coordinate, &shown_unit(plot, x))
                        .map_err(|e| error(d.id, e))?;
                    if !source.is_compatible_with(&target) {
                        return Err(error(
                            d.id,
                            "Display unit is incompatible with the plot coordinate unit.",
                        ));
                    }
                    let scale = source.scale / target.scale;
                    let offset = (source.offset - target.offset) / target.scale;
                    if !scale.is_finite() || scale <= 0. || !offset.is_finite() {
                        return Err(error(
                            d.id,
                            "Display conversion is not finite and increasing.",
                        ));
                    }
                    Some(target)
                };
                if x {
                    overrides.x_display_unit = selected.map(Box::new);
                } else {
                    overrides.y_display_unit = selected.map(Box::new);
                }
            }
            (X_RANGE | Y_RANGE, PropertyValue::AxisRange(value)) => {
                let range = match value {
                    AxisRangeValue::Auto => None,
                    AxisRangeValue::Manual { min, max, unit } => {
                        if !min.is_finite() || !max.is_finite() || min >= max {
                            return Err(error(
                                d.id,
                                "Range endpoints must be finite with min < max.",
                            ));
                        }
                        let from = unit
                            .map(|u| resolve_unit(&u, &coordinate, &shown_unit(plot, x)))
                            .transpose()
                            .map_err(|e| error(d.id, e))?;
                        let range = AxisRange {
                            min: convert_curve_value(min, &from, &coordinate, false)
                                .map_err(|e| error(d.id, e))?,
                            max: convert_curve_value(max, &from, &coordinate, false)
                                .map_err(|e| error(d.id, e))?,
                        };
                        if !range.is_valid() || !range.span().is_finite() {
                            return Err(error(d.id, "Converted range is degenerate or inverted."));
                        }
                        let axis = if x {
                            &plot.figure().x
                        } else {
                            &plot.figure().y
                        };
                        if axis.categories.is_some()
                            || plot.figure().axis_frame == plotium_figure::AxisFrame::Hidden
                        {
                            return Err(PropertyError::NotApplicable(
                                "Choose numeric visible axes to set a manual range.".into(),
                            ));
                        }
                        let scale = if x {
                            overrides.x_scale
                        } else {
                            overrides.y_scale
                        };
                        if scale == Some(plotium_figure::AxisScale::Log10) && range.min <= 0.0 {
                            return Err(error(d.id, super::axis::NON_POSITIVE_LOG_RANGE));
                        }
                        Some(range)
                    }
                };
                if x {
                    overrides.x_range = range;
                } else {
                    overrides.y_range = range;
                }
            }
            (TICK_PT | LABEL_PT | LEGEND_PT, PropertyValue::Float(v)) => {
                FloatBounds::inclusive(1., 72.).check(d.id, "font size", v)?;
                let v = if reset {
                    None
                } else {
                    Some(
                        PositiveFiniteF32::new(v as f32)
                            .ok_or_else(|| error(d.id, "Font size must be finite and positive."))?,
                    )
                };
                match d.id {
                    TICK_PT => overrides.typography.tick_pt = v,
                    LABEL_PT => overrides.typography.label_pt = v,
                    _ => overrides.typography.legend_pt = v,
                }
            }
            _ => return Err(error(d.id, "Value does not match the property schema.")),
        }
        crate::state::axis_display::validate_display(
            overrides,
            plot.binding.coordinates.as_ref(),
            plot.figure(),
        )
        .map_err(|e| error(d.id, e))?;
        Ok(())
    }
}
