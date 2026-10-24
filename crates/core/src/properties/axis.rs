//! Plot-object axis labels and visibility overrides.

use super::provider::PropertyProvider;
use super::target::{require_plot_object_target, resolved_schema};
use super::{
    AggregateValue, Applicability, Availability, ComponentKind, DefaultPolicy, EditOp, EnumVariant,
    FloatBounds, FloatDisplay, PropertyAccess, PropertyAddress, PropertyDefinition, PropertyError,
    PropertyId, PropertyTransaction, PropertyValue, ResolvedProperty, ScopeKind, Tier, ValueCopies,
    ValueSchema,
};
use crate::state::{FieldCapabilities, ObjectId, PlotObject, PlotiumApp};
use plotium_figure::{AxisFrame, AxisScale};

pub const X_LABEL: PropertyId = PropertyId("object.axes.x_label");
pub const Y_LABEL: PropertyId = PropertyId("object.axes.y_label");
pub const EQUAL_SCALE: PropertyId = PropertyId("object.axes.equal_scale");
pub const X_SHOW_TICK_LABELS: PropertyId = PropertyId("object.axes.x_show_tick_labels");
pub const X_SHOW_LABEL: PropertyId = PropertyId("object.axes.x_show_label");
pub const Y_SHOW_TICK_LABELS: PropertyId = PropertyId("object.axes.y_show_tick_labels");
pub const Y_SHOW_LABEL: PropertyId = PropertyId("object.axes.y_show_label");
pub const X_THOUSANDS_SEPARATOR: PropertyId = PropertyId("object.axes.x_thousands_separator");
pub const Y_THOUSANDS_SEPARATOR: PropertyId = PropertyId("object.axes.y_thousands_separator");
pub const FRAME: PropertyId = PropertyId("object.axes.frame");
pub const X_SCALE: PropertyId = PropertyId("object.axes.x_scale");
pub const Y_SCALE: PropertyId = PropertyId("object.axes.y_scale");
pub const X_MAJOR_STEP: PropertyId = PropertyId("object.axes.x_major_step");
pub const Y_MAJOR_STEP: PropertyId = PropertyId("object.axes.y_major_step");

pub const OPEN: &str = "open";
pub const BOX: &str = "box";
const FRAMES: &[EnumVariant] = &[
    EnumVariant::new(OPEN, "Left and bottom"),
    EnumVariant::new(BOX, "All four sides"),
];
pub const LINEAR: &str = "linear";
pub const LOG: &str = "log";
const SCALES: &[EnumVariant] = &[
    EnumVariant::new(LINEAR, "Linear"),
    EnumVariant::new(LOG, "Logarithmic"),
];
const NO_AXES: &str = "Choose a chart with visible axes to edit axis settings.";
const STACKED_EQUAL_SCALE: &str =
    "Plots in a shared-x stack keep the stack's width and cannot use equal x/y scale.";

const OBJECT: Applicability = Applicability::component(ComponentKind::None);

const MAJOR_STEP_BOUNDS: FloatBounds = FloatBounds::inclusive(0., 1e15);
/// Major tick step in the axis's display unit; 0 lets the axis choose.
pub(crate) const MAJOR_STEP_SCHEMA: ValueSchema = ValueSchema::Float {
    bounds: MAJOR_STEP_BOUNDS,
    display: FloatDisplay::Linear(""),
    drag_step: None,
};

const fn axis_definition(
    id: PropertyId,
    value_schema: ValueSchema,
    canonical_label: &'static str,
    canonical_aliases: &'static [&'static str],
    tier: Tier,
) -> PropertyDefinition {
    PropertyDefinition {
        id,
        scope_kind: ScopeKind::Object,
        value_schema,
        access: PropertyAccess::ReadWrite,
        applicability: OBJECT,
        default_policy: DefaultPolicy::Derived,
        tier,
        copies: ValueCopies::PerTarget,
        canonical_label,
        canonical_aliases,
    }
}

pub(crate) const DEFINITIONS: &[PropertyDefinition] = &[
    axis_definition(
        EQUAL_SCALE,
        ValueSchema::Bool,
        "Equal x/y scale",
        &[
            "1:1 scale",
            "equal axis scale",
            "equal F1/F2 scale",
            "aspect lock",
            "Nyquist",
        ],
        Tier::Essential,
    ),
    axis_definition(
        X_LABEL,
        ValueSchema::Text,
        "X-axis title",
        &["x label"],
        Tier::Essential,
    ),
    axis_definition(
        Y_LABEL,
        ValueSchema::Text,
        "Y-axis title",
        &["y label"],
        Tier::Essential,
    ),
    axis_definition(
        X_SHOW_TICK_LABELS,
        ValueSchema::Bool,
        "Show x-axis tick labels",
        &["x ticks", "x tick labels"],
        Tier::Essential,
    ),
    axis_definition(
        X_SHOW_LABEL,
        ValueSchema::Bool,
        "Show x-axis title",
        &["x title visibility"],
        Tier::Advanced,
    ),
    axis_definition(
        Y_SHOW_TICK_LABELS,
        ValueSchema::Bool,
        "Show y-axis tick labels",
        &["y ticks", "y tick labels"],
        Tier::Essential,
    ),
    axis_definition(
        Y_SHOW_LABEL,
        ValueSchema::Bool,
        "Show y-axis title",
        &["y title visibility"],
        Tier::Advanced,
    ),
    axis_definition(
        X_THOUSANDS_SEPARATOR,
        ValueSchema::Bool,
        "X-axis thousands separator",
        &[
            "x digit grouping",
            "x comma separator",
            "x tick label format",
        ],
        Tier::Advanced,
    ),
    axis_definition(
        Y_THOUSANDS_SEPARATOR,
        ValueSchema::Bool,
        "Y-axis thousands separator",
        &[
            "y digit grouping",
            "y comma separator",
            "y tick label format",
        ],
        Tier::Advanced,
    ),
    axis_definition(
        FRAME,
        ValueSchema::Enum { variants: FRAMES },
        "Axis frame",
        &[
            "box frame",
            "top and right axes",
            "four-sided frame",
            "spines",
        ],
        Tier::Essential,
    ),
    axis_definition(
        X_SCALE,
        ValueSchema::Enum { variants: SCALES },
        "X-axis scale",
        &["log x", "logarithmic x axis", "x log scale"],
        Tier::Advanced,
    ),
    axis_definition(
        Y_SCALE,
        ValueSchema::Enum { variants: SCALES },
        "Y-axis scale",
        &["log y", "logarithmic y axis", "y log scale", "semilog"],
        Tier::Advanced,
    ),
    axis_definition(
        X_MAJOR_STEP,
        MAJOR_STEP_SCHEMA,
        "X major tick step",
        &["x tick spacing", "x tick interval", "x major tick interval"],
        Tier::Advanced,
    ),
    axis_definition(
        Y_MAJOR_STEP,
        MAJOR_STEP_SCHEMA,
        "Y major tick step",
        &["y tick spacing", "y tick interval", "y major tick interval"],
        Tier::Advanced,
    ),
];

pub(crate) struct AxisProvider;

pub(crate) static PROVIDER: AxisProvider = AxisProvider;

impl PropertyProvider for AxisProvider {
    fn definitions(&self) -> &'static [PropertyDefinition] {
        DEFINITIONS
    }

    fn read(
        &self,
        app: &PlotiumApp,
        address: &PropertyAddress,
    ) -> Result<ResolvedProperty, PropertyError> {
        let definition = property_definition(address.definition)?;
        let (canvas, object) = require_plot_object_target(app, &address.target, definition)?;
        let plot = app.doc.canvases[canvas]
            .object(object)
            .and_then(|object| object.plot())
            .ok_or_else(|| PropertyError::UnknownTarget(address.target.describe()))?;
        let availability = if matches!(definition.id, X_LABEL | Y_LABEL | FRAME)
            && plot.figure().axis_frame == AxisFrame::Hidden
        {
            Availability::Disabled(NO_AXES)
        } else if let Some(reason) = scale_unavailable(app, definition.id, plot) {
            Availability::Disabled(reason)
        } else if let Some(reason) = major_step_axis(definition.id, plot.figure())
            .and_then(|axis| major_step_unavailable(plot.figure(), axis))
        {
            Availability::Disabled(reason)
        } else if definition.id == EQUAL_SCALE
            && let Some(reason) =
                equal_scale_blocked(app, canvas, object, plot, !plot.figure().lock_aspect)
        {
            Availability::Disabled(reason)
        } else {
            Availability::Editable
        };
        Ok(ResolvedProperty {
            address: address.clone(),
            value: AggregateValue::Uniform(value_of(definition.id, plot)?),
            default_value: Some(default_value(definition.id, plot)?),
            modified: Some(has_override(definition.id, plot)?),
            availability,
            schema: resolved_schema(definition, &FieldCapabilities::default()),
        })
    }

    fn edit(
        &self,
        app: &PlotiumApp,
        transaction: &mut PropertyTransaction,
        address: &PropertyAddress,
        operation: &EditOp<'_>,
    ) -> Result<(), PropertyError> {
        let definition = property_definition(address.definition)?;
        let (canvas, object) = require_plot_object_target(app, &address.target, definition)?;
        if definition.id == EQUAL_SCALE
            && let EditOp::Set(PropertyValue::Bool(lock)) = operation
        {
            let plot = app.doc.canvases[canvas]
                .object(object)
                .and_then(|object| object.plot())
                .ok_or_else(|| PropertyError::UnknownTarget(address.target.describe()))?;
            if let Some(reason) = equal_scale_blocked(app, canvas, object, plot, *lock) {
                return Err(PropertyError::NotApplicable(reason.to_owned()));
            }
        }
        if matches!(definition.id, X_LABEL | Y_LABEL | FRAME)
            && app.doc.canvases[canvas]
                .object(object)
                .and_then(|object| object.plot())
                .is_some_and(|plot| plot.figure().axis_frame == AxisFrame::Hidden)
        {
            return Err(PropertyError::NotApplicable(NO_AXES.to_owned()));
        }
        if matches!(definition.id, X_SCALE | Y_SCALE)
            && !matches!(operation, EditOp::Reset)
            && let Some(reason) = app.doc.canvases[canvas]
                .object(object)
                .and_then(|object| object.plot())
                .and_then(|plot| scale_unavailable(app, definition.id, plot))
        {
            return Err(PropertyError::NotApplicable(reason.to_owned()));
        }
        if !matches!(operation, EditOp::Reset)
            && let Some(reason) = app.doc.canvases[canvas]
                .object(object)
                .and_then(|object| object.plot())
                .and_then(|plot| {
                    major_step_axis(definition.id, plot.figure())
                        .and_then(|axis| major_step_unavailable(plot.figure(), axis))
                })
        {
            return Err(PropertyError::NotApplicable(reason.to_owned()));
        }
        let overrides = transaction.axis_overrides(app, canvas, object)?;
        match (definition.id, operation) {
            (EQUAL_SCALE, EditOp::Set(PropertyValue::Bool(value))) => {
                overrides.lock_aspect = Some(*value)
            }
            (X_LABEL, EditOp::Set(PropertyValue::Text(value))) => {
                overrides.x_label = (!value.trim().is_empty()).then(|| value.clone())
            }
            (Y_LABEL, EditOp::Set(PropertyValue::Text(value))) => {
                overrides.y_label = (!value.trim().is_empty()).then(|| value.clone())
            }
            (X_SHOW_TICK_LABELS, EditOp::Set(PropertyValue::Bool(value))) => {
                overrides.x_show_tick_labels = Some(*value)
            }
            (X_SHOW_LABEL, EditOp::Set(PropertyValue::Bool(value))) => {
                overrides.x_show_label = Some(*value)
            }
            (Y_SHOW_TICK_LABELS, EditOp::Set(PropertyValue::Bool(value))) => {
                overrides.y_show_tick_labels = Some(*value)
            }
            (Y_SHOW_LABEL, EditOp::Set(PropertyValue::Bool(value))) => {
                overrides.y_show_label = Some(*value)
            }
            (X_THOUSANDS_SEPARATOR, EditOp::Set(PropertyValue::Bool(value))) => {
                overrides.x_thousands_separator = Some(*value)
            }
            (Y_THOUSANDS_SEPARATOR, EditOp::Set(PropertyValue::Bool(value))) => {
                overrides.y_thousands_separator = Some(*value)
            }
            (FRAME, EditOp::Set(PropertyValue::Enum(OPEN))) => {
                overrides.axis_frame = Some(AxisFrame::Open)
            }
            (FRAME, EditOp::Set(PropertyValue::Enum(BOX))) => {
                overrides.axis_frame = Some(AxisFrame::Box)
            }
            (X_SCALE | Y_SCALE, EditOp::Set(PropertyValue::Enum(LINEAR))) => {
                *scale_slot(overrides, definition.id) = Some(AxisScale::Linear)
            }
            (X_SCALE | Y_SCALE, EditOp::Set(PropertyValue::Enum(LOG))) => {
                let manual = if definition.id == X_SCALE {
                    overrides.x_range
                } else {
                    overrides.y_range
                };
                if manual.is_some_and(|range| range.min <= 0.0) {
                    return Err(PropertyError::InvalidValue {
                        property: definition.id,
                        message: NON_POSITIVE_LOG_RANGE.to_owned(),
                    });
                }
                *scale_slot(overrides, definition.id) = Some(AxisScale::Log10)
            }
            (X_SCALE | Y_SCALE, EditOp::Reset) => *scale_slot(overrides, definition.id) = None,
            (X_MAJOR_STEP | Y_MAJOR_STEP, EditOp::Set(PropertyValue::Float(value))) => {
                checked_major_step(definition.id, *value)?;
                *major_step_slot(overrides, definition.id) = (*value > 0.0).then_some(*value);
            }
            (X_MAJOR_STEP | Y_MAJOR_STEP, EditOp::Reset) => {
                *major_step_slot(overrides, definition.id) = None
            }
            (EQUAL_SCALE, EditOp::Reset) => overrides.lock_aspect = None,
            (X_LABEL, EditOp::Reset) => overrides.x_label = None,
            (Y_LABEL, EditOp::Reset) => overrides.y_label = None,
            (X_SHOW_TICK_LABELS, EditOp::Reset) => overrides.x_show_tick_labels = None,
            (X_SHOW_LABEL, EditOp::Reset) => overrides.x_show_label = None,
            (Y_SHOW_TICK_LABELS, EditOp::Reset) => overrides.y_show_tick_labels = None,
            (Y_SHOW_LABEL, EditOp::Reset) => overrides.y_show_label = None,
            (X_THOUSANDS_SEPARATOR, EditOp::Reset) => overrides.x_thousands_separator = None,
            (Y_THOUSANDS_SEPARATOR, EditOp::Reset) => overrides.y_thousands_separator = None,
            (FRAME, EditOp::Reset) => overrides.axis_frame = None,
            (_, EditOp::Step(_)) => {
                return Err(PropertyError::InvalidValue {
                    property: definition.id,
                    message: "axis settings have no step gesture".to_owned(),
                });
            }
            (_, EditOp::Set(value)) => {
                return Err(PropertyError::InvalidValue {
                    property: definition.id,
                    message: format!(
                        "{} does not accept a value of kind {}",
                        definition.canonical_label,
                        value.kind()
                    ),
                });
            }
            (_, EditOp::Reset) => {
                return Err(PropertyError::UnknownProperty(
                    definition.id.as_str().to_owned(),
                ));
            }
        }
        Ok(())
    }
}

fn property_definition(id: PropertyId) -> Result<&'static PropertyDefinition, PropertyError> {
    super::definition(id).ok_or_else(|| PropertyError::UnknownProperty(id.as_str().to_owned()))
}

fn value_of(id: PropertyId, plot: &PlotObject) -> Result<PropertyValue, PropertyError> {
    match id {
        EQUAL_SCALE => Ok(PropertyValue::Bool(plot.figure().lock_aspect)),
        X_LABEL => Ok(PropertyValue::Text(plot.figure().x.label.clone())),
        Y_LABEL => Ok(PropertyValue::Text(plot.figure().y.label.clone())),
        X_SHOW_TICK_LABELS => Ok(PropertyValue::Bool(plot.figure().x.show_tick_labels)),
        X_SHOW_LABEL => Ok(PropertyValue::Bool(plot.figure().x.show_label)),
        Y_SHOW_TICK_LABELS => Ok(PropertyValue::Bool(plot.figure().y.show_tick_labels)),
        Y_SHOW_LABEL => Ok(PropertyValue::Bool(plot.figure().y.show_label)),
        X_THOUSANDS_SEPARATOR => Ok(PropertyValue::Bool(plot.figure().x.thousands_separator)),
        Y_THOUSANDS_SEPARATOR => Ok(PropertyValue::Bool(plot.figure().y.thousands_separator)),
        FRAME => Ok(frame_value(plot.figure().axis_frame)),
        X_SCALE => Ok(scale_value(plot.figure().x.scale)),
        Y_SCALE => Ok(scale_value(plot.figure().y.scale)),
        X_MAJOR_STEP => Ok(major_step_value(plot.figure().x.major_step)),
        Y_MAJOR_STEP => Ok(major_step_value(plot.figure().y.major_step)),
        _ => Err(PropertyError::UnknownProperty(id.as_str().to_owned())),
    }
}

/// Why equal x/y scale cannot be set on `plot`. Turning it on is also refused
/// inside a shared-x stack, whose plots must keep the stack's common width;
/// an already locked plot there may still turn it off.
fn equal_scale_blocked(
    app: &PlotiumApp,
    canvas: usize,
    object: ObjectId,
    plot: &PlotObject,
    turning_on: bool,
) -> Option<&'static str> {
    crate::state::equal_scale_unavailable(plot.figure()).or_else(|| {
        (turning_on
            && !app.doc.canvases[canvas]
                .shared_x_axis_peers(object)
                .is_empty())
        .then_some(STACKED_EQUAL_SCALE)
    })
}

fn default_value(id: PropertyId, plot: &PlotObject) -> Result<PropertyValue, PropertyError> {
    match id {
        X_LABEL => Ok(PropertyValue::Text(plot.automatic_axis_label(true))),
        Y_LABEL => Ok(PropertyValue::Text(plot.automatic_axis_label(false))),
        X_SHOW_TICK_LABELS => Ok(PropertyValue::Bool(plot.derived_axes().x_show_tick_labels)),
        X_SHOW_LABEL => Ok(PropertyValue::Bool(plot.derived_axes().x_show_label)),
        Y_SHOW_TICK_LABELS => Ok(PropertyValue::Bool(plot.derived_axes().y_show_tick_labels)),
        Y_SHOW_LABEL => Ok(PropertyValue::Bool(plot.derived_axes().y_show_label)),
        X_THOUSANDS_SEPARATOR => Ok(PropertyValue::Bool(
            plot.derived_axes().x_thousands_separator,
        )),
        Y_THOUSANDS_SEPARATOR => Ok(PropertyValue::Bool(
            plot.derived_axes().y_thousands_separator,
        )),
        FRAME => Ok(frame_value(plot.derived_axes().frame)),
        EQUAL_SCALE => Ok(PropertyValue::Bool(plot.derived_axes().lock_aspect)),
        X_SCALE => Ok(scale_value(plot.derived_axes().x_scale)),
        Y_SCALE => Ok(scale_value(plot.derived_axes().y_scale)),
        X_MAJOR_STEP | Y_MAJOR_STEP => Ok(major_step_value(None)),
        _ => Err(PropertyError::UnknownProperty(id.as_str().to_owned())),
    }
}

fn has_override(id: PropertyId, plot: &PlotObject) -> Result<bool, PropertyError> {
    match id {
        X_LABEL => Ok(plot.axis_overrides.x_label.is_some()),
        Y_LABEL => Ok(plot.axis_overrides.y_label.is_some()),
        X_SHOW_TICK_LABELS => Ok(plot.axis_overrides.x_show_tick_labels.is_some()),
        X_SHOW_LABEL => Ok(plot.axis_overrides.x_show_label.is_some()),
        Y_SHOW_TICK_LABELS => Ok(plot.axis_overrides.y_show_tick_labels.is_some()),
        Y_SHOW_LABEL => Ok(plot.axis_overrides.y_show_label.is_some()),
        X_THOUSANDS_SEPARATOR => Ok(plot.axis_overrides.x_thousands_separator.is_some()),
        Y_THOUSANDS_SEPARATOR => Ok(plot.axis_overrides.y_thousands_separator.is_some()),
        FRAME => Ok(plot.axis_overrides.axis_frame.is_some()),
        // Imported 2D spectra seed this override; it counts as an edit only
        // where it departs from the chart's own choice.
        EQUAL_SCALE => Ok(plot
            .axis_overrides
            .lock_aspect
            .is_some_and(|lock| lock != plot.derived_axes().lock_aspect)),
        X_SCALE => Ok(plot.axis_overrides.x_scale.is_some()),
        Y_SCALE => Ok(plot.axis_overrides.y_scale.is_some()),
        X_MAJOR_STEP => Ok(plot.axis_overrides.x_major_step.is_some()),
        Y_MAJOR_STEP => Ok(plot.axis_overrides.y_major_step.is_some()),
        _ => Err(PropertyError::UnknownProperty(id.as_str().to_owned())),
    }
}

/// A hidden frame only occurs on axis-less charts, where the property is
/// disabled; it reads as the open frame those charts would otherwise use.
fn frame_value(frame: AxisFrame) -> PropertyValue {
    PropertyValue::Enum(match frame {
        AxisFrame::Box => BOX,
        AxisFrame::Open | AxisFrame::Hidden => OPEN,
    })
}

/// Rejection shared by scale and range writes that would leave a log axis
/// without a positive window.
pub(crate) const NON_POSITIVE_LOG_RANGE: &str = "A logarithmic axis needs a positive range: set a manual minimum above zero or return the range to Auto.";

fn scale_value(scale: AxisScale) -> PropertyValue {
    PropertyValue::Enum(match scale {
        AxisScale::Linear => LINEAR,
        AxisScale::Log10 => LOG,
    })
}

fn scale_slot(
    overrides: &mut crate::state::AxisOverrides,
    id: PropertyId,
) -> &mut Option<AxisScale> {
    if id == X_SCALE {
        &mut overrides.x_scale
    } else {
        &mut overrides.y_scale
    }
}

/// Why a scale property is disabled for `plot`. NMR spectra stay linear
/// because their integral, phase and cursor tools measure in linear ppm.
pub(crate) fn scale_unavailable(
    app: &PlotiumApp,
    id: PropertyId,
    plot: &PlotObject,
) -> Option<&'static str> {
    if !matches!(id, X_SCALE | Y_SCALE) {
        return None;
    }
    if let Some(reason) = crate::state::log_scale_unavailable(plot.figure(), id == X_SCALE) {
        return Some(reason);
    }
    is_nmr_spectrum(app, plot).then_some("NMR spectra keep linear axes: their integral, phase and cursor tools measure in linear ppm.")
}

/// Whether `plot` shows an NMR signal, whose tools measure in linear ppm.
pub(crate) fn is_nmr_spectrum(app: &PlotiumApp, plot: &PlotObject) -> bool {
    app.display_binding(plot.display_owner, &plot.binding)
        .series
        .first()
        .and_then(|series| {
            let dataset = app.doc.dataset_by_id(series.source.resource)?;
            dataset.field_descriptor(series.source.field)
        })
        .is_some_and(|field| {
            field
                .capabilities
                .contains(crate::automation::CAP_FIELD_NMR_SIGNAL)
        })
}

fn major_step_slot(
    overrides: &mut crate::state::AxisOverrides,
    id: PropertyId,
) -> &mut Option<f64> {
    if id == X_MAJOR_STEP {
        &mut overrides.x_major_step
    } else {
        &mut overrides.y_major_step
    }
}

fn major_step_axis(
    id: PropertyId,
    figure: &plotium_figure::Figure,
) -> Option<&plotium_figure::Axis> {
    match id {
        X_MAJOR_STEP => Some(&figure.x),
        Y_MAJOR_STEP => Some(&figure.y),
        _ => None,
    }
}

pub(crate) fn major_step_value(step: Option<f64>) -> PropertyValue {
    PropertyValue::Float(step.unwrap_or(0.0))
}

pub(crate) fn checked_major_step(id: PropertyId, value: f64) -> Result<f64, PropertyError> {
    MAJOR_STEP_BOUNDS.check(id, "major tick step", value)
}

/// Why `axis` cannot take a set major tick step.
pub(crate) fn major_step_unavailable(
    figure: &plotium_figure::Figure,
    axis: &plotium_figure::Axis,
) -> Option<&'static str> {
    if figure.axis_frame == AxisFrame::Hidden {
        Some(NO_AXES)
    } else if axis.categories.is_some() {
        Some("A category axis places one tick per category.")
    } else if !axis.effective_scale().is_linear() {
        Some("A logarithmic axis places its major ticks on powers of ten.")
    } else if axis.axis_break.is_some() {
        Some("A broken axis chooses its own ticks; remove the break to set a tick step.")
    } else {
        None
    }
}
