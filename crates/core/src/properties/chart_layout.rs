//! Category-chart layout properties: replicate spread and value labels on a
//! series, and the orientation of an explicit-coordinate plot.
use super::provider::PropertyProvider;
use super::target::{require_plot_object_target, resolved_schema, series_context};
use super::*;
use crate::state::PlotiumApp;
use plotium_figure::{SeriesEncoding, SeriesKind, UnitInterval};

pub const SPREAD: PropertyId = PropertyId("series.points.spread");
pub const VALUE_LABELS: PropertyId = PropertyId("series.value_labels");
pub const ORIENTATION: PropertyId = PropertyId("object.chart.orientation");
const SPREAD_BOUNDS: FloatBounds = FloatBounds::inclusive(0., 1.);
const ORIENTATIONS: &[EnumVariant] = &[
    EnumVariant::new("vertical", "Vertical"),
    EnumVariant::new("horizontal", "Horizontal"),
];

const fn series(id: PropertyId, schema: ValueSchema, label: &'static str) -> PropertyDefinition {
    PropertyDefinition {
        id,
        scope_kind: ScopeKind::Object,
        value_schema: schema,
        access: PropertyAccess::ReadWrite,
        applicability: Applicability::encoding(ComponentKind::Series, EncodingKind::Line),
        default_policy: DefaultPolicy::Derived,
        tier: Tier::Advanced,
        copies: ValueCopies::PerTarget,
        canonical_label: label,
        canonical_aliases: &[],
    }
}

const DEFINITIONS: &[PropertyDefinition] = &[
    series(
        SPREAD,
        ValueSchema::Float {
            bounds: SPREAD_BOUNDS,
            display: FloatDisplay::Linear(""),
            drag_step: Some(0.05),
        },
        "Replicate spread",
    ),
    series(VALUE_LABELS, ValueSchema::Bool, "Value labels"),
    PropertyDefinition {
        id: ORIENTATION,
        scope_kind: ScopeKind::Object,
        value_schema: ValueSchema::Enum {
            variants: ORIENTATIONS,
        },
        access: PropertyAccess::ReadWrite,
        applicability: Applicability::component(ComponentKind::None),
        default_policy: DefaultPolicy::Derived,
        tier: Tier::Advanced,
        copies: ValueCopies::PerTarget,
        canonical_label: "Chart orientation",
        canonical_aliases: &["horizontal bars"],
    },
];

pub(crate) struct ChartLayoutProvider;
pub(crate) static PROVIDER: ChartLayoutProvider = ChartLayoutProvider;

fn uses_right_axis(binding: &crate::state::DataBinding) -> bool {
    binding.series.iter().any(
        |s| matches!(&s.encoding, SeriesEncoding::Line(line) if line.appearance.y_axis.is_right()),
    )
}

fn invalid(id: PropertyId, message: &str) -> PropertyError {
    PropertyError::InvalidValue {
        property: id,
        message: message.to_owned(),
    }
}

impl PropertyProvider for ChartLayoutProvider {
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
        if d.id == ORIENTATION {
            let (canvas, object) = require_plot_object_target(app, &address.target, d)?;
            let plot = app.doc.canvases[canvas]
                .object(object)
                .and_then(|o| o.plot())
                .ok_or_else(|| PropertyError::UnknownTarget(address.target.describe()))?;
            let coordinates = plot.binding.coordinates.as_ref();
            let horizontal = coordinates.is_some_and(|c| c.transposed);
            return Ok(ResolvedProperty {
                address: address.clone(),
                value: AggregateValue::Uniform(PropertyValue::Enum(if horizontal {
                    "horizontal"
                } else {
                    "vertical"
                })),
                default_value: Some(PropertyValue::Enum("vertical")),
                modified: Some(horizontal),
                availability: if coordinates.is_none() {
                    Availability::Disabled(
                        "Only plots built from explicit columns can turn sideways.",
                    )
                } else if !horizontal && uses_right_axis(&plot.binding) {
                    Availability::Disabled(crate::state::right_axis::HORIZONTAL_ONE_VALUE_AXIS)
                } else {
                    Availability::Editable
                },
                schema: ResolvedSchema::Enum {
                    variants: ORIENTATIONS.iter().collect(),
                },
            });
        }
        let c = series_context(app, &address.target, d)?;
        let SeriesEncoding::Line(line) = c.encoding else {
            unreachable!("encoding gate")
        };
        let kind = line.kind;
        let (value, default, modified, available) = match d.id {
            SPREAD => (
                PropertyValue::Float(line.spread.map_or(0.0, UnitInterval::get)),
                PropertyValue::Float(0.0),
                Some(line.spread.is_some()),
                matches!(kind, SeriesKind::Points | SeriesKind::Line),
            ),
            VALUE_LABELS => (
                PropertyValue::Bool(line.value_labels),
                PropertyValue::Bool(false),
                Some(line.value_labels),
                matches!(
                    kind,
                    SeriesKind::Bar | SeriesKind::Points | SeriesKind::Line
                ),
            ),
            _ => return Err(PropertyError::UnknownProperty(d.id.to_string())),
        };
        Ok(ResolvedProperty {
            address: address.clone(),
            value: AggregateValue::Uniform(value),
            default_value: Some(default),
            modified,
            availability: if available {
                Availability::Editable
            } else if d.id == SPREAD {
                Availability::Disabled("Choose a point or line curve to spread its replicates.")
            } else {
                Availability::Disabled("Choose a bar, point or line curve to label its values.")
            },
            schema: resolved_schema(d, &c.capabilities),
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
        let resolved = self.read(app, address)?;
        if let Availability::Disabled(reason) = resolved.availability {
            return Err(PropertyError::NotApplicable(reason.into()));
        }
        let value = match op {
            EditOp::Set(v) => (*v).clone(),
            EditOp::Reset => resolved
                .default_value
                .ok_or_else(|| invalid(d.id, "No default"))?,
            EditOp::Step(_) => return Err(invalid(d.id, "No step gesture")),
        };
        if d.id == ORIENTATION {
            let (canvas, object) = require_plot_object_target(app, &address.target, d)?;
            let PropertyValue::Enum(choice) = value else {
                return Err(invalid(d.id, "Expected vertical or horizontal."));
            };
            let transposed = match choice {
                "vertical" => false,
                "horizontal" => true,
                _ => return Err(invalid(d.id, "Expected vertical or horizontal.")),
            };
            let coordinates = tx
                .data_binding(app, canvas, object)?
                .coordinates
                .as_mut()
                .ok_or_else(|| invalid(d.id, "The plot has no explicit coordinates."))?;
            if coordinates.transposed != transposed {
                coordinates.transposed = transposed;
                // Author titles, ranges, units and annotations turn with the data.
                tx.axis_overrides(app, canvas, object)?.swap_axes();
            }
            return Ok(());
        }
        let c = series_context(app, &address.target, d)?;
        let binding = tx.data_binding(app, c.canvas, c.object)?;
        let sb = binding
            .series
            .iter_mut()
            .find(|s| s.id == c.series)
            .ok_or_else(|| PropertyError::UnknownTarget(address.target.describe()))?;
        let SeriesEncoding::Line(line) = &mut sb.encoding else {
            unreachable!("encoding gate")
        };
        match (d.id, value) {
            (SPREAD, PropertyValue::Float(v)) => {
                SPREAD_BOUNDS.check(d.id, "replicate spread", v)?;
                line.spread = if matches!(op, EditOp::Reset) || v == 0.0 {
                    None
                } else {
                    Some(UnitInterval::new(v).ok_or_else(|| invalid(d.id, "Spread is 0–1."))?)
                };
            }
            (VALUE_LABELS, PropertyValue::Bool(on)) => line.value_labels = on,
            _ => return Err(invalid(d.id, "Value does not match this property.")),
        }
        Ok(())
    }
}
