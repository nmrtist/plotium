//! Document-owned figure typography properties.

use super::provider::PropertyProvider;
use super::target::require_document_target;
use super::{
    AggregateValue, Applicability, Availability, ComponentKind, DefaultPolicy, EditOp, EnumVariant,
    FloatBounds, FloatDisplay, PropertyAccess, PropertyAddress, PropertyDefinition, PropertyError,
    PropertyId, PropertyTransaction, PropertyValue, ResolvedProperty, ScopeKind, Tier, ValueCopies,
    ValueSchema, definition, static_schema,
};
use crate::state::PlotiumApp;
use plotium_figure::{Color, TickDirection};

pub const TICK_PT: PropertyId = PropertyId("document.figure.typography.tick_pt");
pub const LABEL_PT: PropertyId = PropertyId("document.figure.typography.label_pt");
pub const TITLE_PT: PropertyId = PropertyId("document.figure.typography.title_pt");
pub const LEGEND_PT: PropertyId = PropertyId("document.figure.typography.legend_pt");
pub const LEGEND_COLOR: PropertyId = PropertyId("document.figure.typography.legend_color");
pub const AXIS_LINE_PT: PropertyId = PropertyId("document.figure.typography.axis_line_pt");
pub const TICK_LENGTH_PT: PropertyId = PropertyId("document.figure.typography.tick_length_pt");
pub const TICK_DIRECTION: PropertyId = PropertyId("document.figure.typography.tick_direction");

pub const OUT: &str = "out";
pub const IN: &str = "in";
const TICK_DIRECTIONS: &[EnumVariant] = &[
    EnumVariant::new(OUT, "Outward"),
    EnumVariant::new(IN, "Inward"),
];

const POINT_BOUNDS: FloatBounds = FloatBounds::inclusive(1.0, 72.0);
/// A quarter point per drag notch: point sizes are chosen to a half point, and
/// the admissible range spans seventy-one of them, so the range says nothing
/// about how finely the value is usually set.
const POINT_STEP: f64 = 0.25;
/// Axis strokes share the line-series width range.
const STROKE_BOUNDS: FloatBounds = FloatBounds::inclusive(0.05, 10.0);
const STROKE_STEP: f64 = 0.05;
/// Zero-length ticks are allowed: they hide the marks but keep their labels.
const TICK_LENGTH_BOUNDS: FloatBounds = FloatBounds::inclusive(0.0, 20.0);

const fn typography_definition(
    id: PropertyId,
    default: f64,
    label: &'static str,
    aliases: &'static [&'static str],
) -> PropertyDefinition {
    float_definition(
        id,
        POINT_BOUNDS,
        POINT_STEP,
        default,
        Tier::Essential,
        label,
        aliases,
    )
}

const fn float_definition(
    id: PropertyId,
    bounds: FloatBounds,
    step: f64,
    default: f64,
    tier: Tier,
    label: &'static str,
    aliases: &'static [&'static str],
) -> PropertyDefinition {
    PropertyDefinition {
        id,
        scope_kind: ScopeKind::Document,
        value_schema: ValueSchema::Float {
            bounds,
            display: FloatDisplay::Linear("pt"),
            drag_step: Some(step),
        },
        access: PropertyAccess::ReadWrite,
        applicability: Applicability::component(ComponentKind::None),
        default_policy: DefaultPolicy::Fixed(PropertyValue::Float(default)),
        tier,
        copies: ValueCopies::PerTarget,
        canonical_label: label,
        canonical_aliases: aliases,
    }
}

fn bounds_of(definition: &PropertyDefinition) -> FloatBounds {
    match definition.value_schema {
        ValueSchema::Float { bounds, .. } => bounds,
        _ => POINT_BOUNDS,
    }
}

pub(crate) const DEFINITIONS: &[PropertyDefinition] = &[
    typography_definition(
        TICK_PT,
        7.0,
        "Figure tick-label size",
        &["figure typography", "tick size", "font size", "points"],
    ),
    typography_definition(
        LABEL_PT,
        8.0,
        "Figure axis-title size",
        &["axis title size", "axis label size", "figure typography"],
    ),
    typography_definition(
        TITLE_PT,
        8.0,
        "Figure title size",
        &["title size", "figure heading", "figure typography"],
    ),
    typography_definition(
        LEGEND_PT,
        7.0,
        "Figure legend size",
        &["legend font size", "key size", "figure typography"],
    ),
    PropertyDefinition {
        id: LEGEND_COLOR,
        scope_kind: ScopeKind::Document,
        value_schema: ValueSchema::Color,
        access: PropertyAccess::ReadWrite,
        applicability: Applicability::component(ComponentKind::None),
        default_policy: DefaultPolicy::Fixed(PropertyValue::Color(Color::AXIS)),
        tier: Tier::Advanced,
        copies: ValueCopies::PerTarget,
        canonical_label: "Figure legend text color",
        canonical_aliases: &["legend colour", "key text color", "figure typography"],
    },
    float_definition(
        AXIS_LINE_PT,
        STROKE_BOUNDS,
        STROKE_STEP,
        1.0,
        Tier::Advanced,
        "Figure axis line width",
        &["axis stroke", "spine width", "tick width", "frame width"],
    ),
    float_definition(
        TICK_LENGTH_PT,
        TICK_LENGTH_BOUNDS,
        POINT_STEP,
        3.0,
        Tier::Advanced,
        "Figure tick length",
        &["tick mark length", "tick size"],
    ),
    PropertyDefinition {
        id: TICK_DIRECTION,
        scope_kind: ScopeKind::Document,
        value_schema: ValueSchema::Enum {
            variants: TICK_DIRECTIONS,
        },
        access: PropertyAccess::ReadWrite,
        applicability: Applicability::component(ComponentKind::None),
        default_policy: DefaultPolicy::Fixed(PropertyValue::Enum(OUT)),
        tier: Tier::Advanced,
        copies: ValueCopies::PerTarget,
        canonical_label: "Figure tick direction",
        canonical_aliases: &["inward ticks", "outward ticks", "tick orientation"],
    },
];

pub(crate) struct TypographyProvider;

pub(crate) static PROVIDER: TypographyProvider = TypographyProvider;

impl PropertyProvider for TypographyProvider {
    fn definitions(&self) -> &'static [PropertyDefinition] {
        DEFINITIONS
    }

    fn read(
        &self,
        app: &PlotiumApp,
        address: &PropertyAddress,
    ) -> Result<ResolvedProperty, PropertyError> {
        let definition = definition(address.definition).ok_or_else(|| {
            PropertyError::UnknownProperty(address.definition.as_str().to_owned())
        })?;
        require_document_target(&address.target, definition)?;
        let typography = app.doc.style_library.figure_typography;
        let value = match definition.id {
            TICK_PT => PropertyValue::Float(f64::from(typography.tick_pt)),
            LABEL_PT => PropertyValue::Float(f64::from(typography.label_pt)),
            TITLE_PT => PropertyValue::Float(f64::from(typography.title_pt)),
            LEGEND_PT => PropertyValue::Float(f64::from(typography.legend_pt)),
            LEGEND_COLOR => PropertyValue::Color(typography.legend_color),
            AXIS_LINE_PT => PropertyValue::Float(f64::from(typography.axis_line_pt)),
            TICK_LENGTH_PT => PropertyValue::Float(f64::from(typography.tick_length_pt)),
            TICK_DIRECTION => PropertyValue::Enum(match typography.tick_direction {
                TickDirection::Out => OUT,
                TickDirection::In => IN,
            }),
            _ => return Err(PropertyError::UnknownProperty(definition.id.to_string())),
        };
        Ok(ResolvedProperty {
            address: address.clone(),
            modified: None,
            value: AggregateValue::Uniform(value),
            default_value: match &definition.default_policy {
                DefaultPolicy::Fixed(value) => Some(value.clone()),
                DefaultPolicy::EncodingFactory
                | DefaultPolicy::ProcessingFactory
                | DefaultPolicy::Derived
                | DefaultPolicy::None => None,
            },
            availability: Availability::Editable,
            schema: static_schema(definition),
        })
    }

    fn edit(
        &self,
        app: &PlotiumApp,
        transaction: &mut PropertyTransaction,
        address: &PropertyAddress,
        operation: &EditOp<'_>,
    ) -> Result<(), PropertyError> {
        let definition = definition(address.definition).ok_or_else(|| {
            PropertyError::UnknownProperty(address.definition.as_str().to_owned())
        })?;
        require_document_target(&address.target, definition)?;
        if definition.id == LEGEND_COLOR {
            let value = match operation {
                EditOp::Set(PropertyValue::Color(value)) => *value,
                EditOp::Reset => Color::AXIS,
                EditOp::Set(value) => {
                    return Err(PropertyError::InvalidValue {
                        property: definition.id,
                        message: format!("expected a color, got {}", value.kind()),
                    });
                }
                EditOp::Step(_) => {
                    return Err(PropertyError::InvalidValue {
                        property: definition.id,
                        message: "this setting has no step gesture".to_owned(),
                    });
                }
            };
            transaction.figure_typography(app).legend_color = value;
            return Ok(());
        }
        if definition.id == TICK_DIRECTION {
            let value = match operation {
                EditOp::Set(PropertyValue::Enum(OUT)) | EditOp::Reset => TickDirection::Out,
                EditOp::Set(PropertyValue::Enum(IN)) => TickDirection::In,
                EditOp::Set(value) => {
                    return Err(PropertyError::InvalidValue {
                        property: definition.id,
                        message: format!("expected `out` or `in`, got {}", value.kind()),
                    });
                }
                EditOp::Step(_) => {
                    return Err(PropertyError::InvalidValue {
                        property: definition.id,
                        message: "this setting has no step gesture".to_owned(),
                    });
                }
            };
            transaction.figure_typography(app).tick_direction = value;
            return Ok(());
        }
        let value = match operation {
            EditOp::Set(PropertyValue::Float(value)) => {
                bounds_of(definition).check(definition.id, definition.canonical_label, *value)?
            }
            EditOp::Set(value) => {
                return Err(PropertyError::InvalidValue {
                    property: definition.id,
                    message: format!("expected a number, got {}", value.kind()),
                });
            }
            EditOp::Reset => match &definition.default_policy {
                DefaultPolicy::Fixed(PropertyValue::Float(value)) => *value,
                DefaultPolicy::Fixed(_)
                | DefaultPolicy::EncodingFactory
                | DefaultPolicy::ProcessingFactory
                | DefaultPolicy::Derived
                | DefaultPolicy::None => {
                    return Err(PropertyError::InvalidValue {
                        property: definition.id,
                        message: "the default policy has no numeric value".to_owned(),
                    });
                }
            },
            EditOp::Step(_) => {
                return Err(PropertyError::InvalidValue {
                    property: definition.id,
                    message: "this setting has no step gesture".to_owned(),
                });
            }
        };
        let typography = transaction.figure_typography(app);
        match definition.id {
            TICK_PT => typography.tick_pt = value as f32,
            LABEL_PT => typography.label_pt = value as f32,
            TITLE_PT => typography.title_pt = value as f32,
            LEGEND_PT => typography.legend_pt = value as f32,
            AXIS_LINE_PT => typography.axis_line_pt = value as f32,
            TICK_LENGTH_PT => typography.tick_length_pt = value as f32,
            _ => return Err(PropertyError::UnknownProperty(definition.id.to_string())),
        }
        Ok(())
    }
}
