//! Curve appearance edits own only one series' native presentation fields.
use super::provider::PropertyProvider;
use super::target::{resolved_schema, series_context};
use super::*;
use crate::state::PlotiumApp;
use plotium_figure::{ColorSource, LineStyle, Marker, PositiveFiniteF32, SeriesEncoding};

pub const COLOR: PropertyId = PropertyId("series.line.color");
pub const LABEL: PropertyId = PropertyId("series.label");
pub const STYLE: PropertyId = PropertyId("series.line.style");
pub const MARKER: PropertyId = PropertyId("series.line.marker");
pub const MARKER_SIZE: PropertyId = PropertyId("series.line.marker_size");
pub const MARKER_FILL: PropertyId = PropertyId("series.line.marker_fill");
pub const MARKER_EDGE_COLOR: PropertyId = PropertyId("series.line.marker_edge_color");
pub const MARKER_EDGE_WIDTH: PropertyId = PropertyId("series.line.marker_edge_width");
pub const MARKER_HOLLOW: PropertyId = PropertyId("series.line.marker_hollow");
const MARKER_EDGE_BOUNDS: FloatBounds = FloatBounds::inclusive(0., 5.);
const STYLES: &[EnumVariant] = &[
    EnumVariant::new("solid", "Solid"),
    EnumVariant::new("dashed", "Dashed"),
    EnumVariant::new("dotted", "Dotted"),
    EnumVariant::new("dash_dot", "Dash-dot"),
];
const MARKERS: &[EnumVariant] = &[
    EnumVariant::new("none", "None"),
    EnumVariant::new("circle", "Circle"),
    EnumVariant::new("square", "Square"),
    EnumVariant::new("diamond", "Diamond"),
    EnumVariant::new("triangle_up", "Triangle up"),
    EnumVariant::new("triangle_down", "Triangle down"),
    EnumVariant::new("plus", "Plus"),
    EnumVariant::new("cross", "Cross"),
];
const fn def(id: PropertyId, schema: ValueSchema, label: &'static str) -> PropertyDefinition {
    PropertyDefinition {
        id,
        scope_kind: ScopeKind::Object,
        value_schema: schema,
        access: PropertyAccess::ReadWrite,
        applicability: Applicability::encoding(ComponentKind::Series, EncodingKind::Line),
        default_policy: DefaultPolicy::Derived,
        tier: Tier::Essential,
        copies: ValueCopies::PerTarget,
        canonical_label: label,
        canonical_aliases: &[],
    }
}
const fn advanced(id: PropertyId, schema: ValueSchema, label: &'static str) -> PropertyDefinition {
    let mut definition = def(id, schema, label);
    definition.tier = Tier::Advanced;
    definition
}
const DEFINITIONS: &[PropertyDefinition] = &[
    def(COLOR, ValueSchema::Color, "Curve color"),
    def(LABEL, ValueSchema::Text, "Legend label"),
    def(STYLE, ValueSchema::Enum { variants: STYLES }, "Line style"),
    def(MARKER, ValueSchema::Enum { variants: MARKERS }, "Marker"),
    def(
        MARKER_SIZE,
        ValueSchema::Float {
            bounds: FloatBounds::inclusive(0.1, 20.),
            display: FloatDisplay::Linear("pt"),
            drag_step: Some(0.1),
        },
        "Marker diameter",
    ),
    advanced(MARKER_FILL, ValueSchema::Color, "Marker fill"),
    advanced(MARKER_EDGE_COLOR, ValueSchema::Color, "Marker edge color"),
    advanced(
        MARKER_EDGE_WIDTH,
        ValueSchema::Float {
            bounds: MARKER_EDGE_BOUNDS,
            display: FloatDisplay::Linear("pt"),
            drag_step: Some(0.05),
        },
        "Marker edge width",
    ),
    advanced(MARKER_HOLLOW, ValueSchema::Bool, "Hollow marker"),
];
pub(crate) struct DisplayProvider;
pub(crate) static PROVIDER: DisplayProvider = DisplayProvider;
impl PropertyProvider for DisplayProvider {
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
        let c = series_context(app, &address.target, d)?;
        let SeriesEncoding::Line(line) = c.encoding else {
            unreachable!("encoding gate")
        };
        let plot = app.doc.canvases[c.canvas]
            .object(c.object)
            .and_then(|o| o.plot())
            .ok_or_else(|| PropertyError::UnknownTarget(address.target.describe()))?;
        let sb = plot
            .binding
            .series
            .iter()
            .find(|s| s.id == c.series)
            .ok_or_else(|| PropertyError::UnknownTarget(address.target.describe()))?;
        // Explicit column curves own their kind; native field renderers own it
        // otherwise. The default LineEncoding does not turn scatter/sticks into
        // lines, and applicability must describe that same native geometry.
        let kind = if sb.source.columns().is_some() {
            line.kind
        } else {
            c.dataset
                .field_descriptor(c.field)
                .ok_or_else(|| PropertyError::UnknownTarget(address.target.describe()))?
                .curve_kind()
        };
        let mut derived = sb.clone();
        derived.label = None;
        let (value, default_value, modified) = match d.id {
            COLOR => {
                let default = plotium_figure::LineEncoding::default().color.resolve();
                (
                    PropertyValue::Color(line.color.resolve()),
                    PropertyValue::Color(default),
                    None,
                )
            }
            LABEL => (
                PropertyValue::Text(app.series_label(sb)),
                PropertyValue::Text(app.series_label(&derived)),
                Some(sb.label.is_some()),
            ),
            STYLE => (
                PropertyValue::Enum(style_id(line.appearance.line_style)),
                PropertyValue::Enum("solid"),
                None,
            ),
            MARKER => (
                PropertyValue::Enum(marker_id(line.appearance.marker(kind))),
                PropertyValue::Enum(if kind == plotium_figure::SeriesKind::Points {
                    "circle"
                } else {
                    "none"
                }),
                Some(line.appearance.marker.is_some()),
            ),
            MARKER_SIZE => (
                PropertyValue::Float(f64::from(
                    line.appearance.marker_radius(line.width.get()) * 2.,
                )),
                PropertyValue::Float(f64::from(line.width.get() * 2.)),
                Some(line.appearance.marker_size.is_some()),
            ),
            MARKER_FILL => (
                PropertyValue::Color(line.appearance.marker_fill.unwrap_or(line.color.resolve())),
                PropertyValue::Color(line.color.resolve()),
                Some(line.appearance.marker_fill.is_some()),
            ),
            MARKER_EDGE_COLOR => (
                PropertyValue::Color(
                    line.appearance
                        .marker_edge_color
                        .unwrap_or(line.color.resolve()),
                ),
                PropertyValue::Color(line.color.resolve()),
                Some(line.appearance.marker_edge_color.is_some()),
            ),
            MARKER_EDGE_WIDTH => (
                PropertyValue::Float(line.appearance.marker_edge_width().map_or(0., f64::from)),
                PropertyValue::Float(
                    plotium_figure::CurveAppearance {
                        marker_edge_width: None,
                        ..line.appearance
                    }
                    .marker_edge_width()
                    .map_or(0., f64::from),
                ),
                Some(line.appearance.marker_edge_width.is_some()),
            ),
            MARKER_HOLLOW => (
                PropertyValue::Bool(line.appearance.marker_hollow),
                PropertyValue::Bool(false),
                None,
            ),
            _ => return Err(PropertyError::UnknownProperty(d.id.to_string())),
        };
        let marker_property = matches!(
            d.id,
            MARKER
                | MARKER_SIZE
                | MARKER_FILL
                | MARKER_EDGE_COLOR
                | MARKER_EDGE_WIDTH
                | MARKER_HOLLOW
        );
        Ok(ResolvedProperty {
            address: address.clone(),
            value: AggregateValue::Uniform(value),
            default_value: Some(default_value),
            modified,
            availability: if matches!(d.id, COLOR | MARKER_FILL | MARKER_EDGE_COLOR)
                && plot.figure().series_colors_are_semantic
            {
                Availability::Disabled(
                    "Choose a curve without semantic colors to set a single series color.",
                )
            } else if marker_property && kind == plotium_figure::SeriesKind::Stick {
                Availability::Disabled("Choose a line or scatter curve to edit markers.")
            } else if d.id == STYLE && kind != plotium_figure::SeriesKind::Line {
                Availability::Disabled("Choose a line curve to edit its stroke style.")
            } else {
                Availability::Editable
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
        let c = series_context(app, &address.target, d)?;
        let resolved = self.read(app, address)?;
        if let Availability::Disabled(reason) = resolved.availability {
            return Err(PropertyError::NotApplicable(reason.into()));
        }
        let invalid = || PropertyError::InvalidValue {
            property: d.id,
            message: "Value does not match this curve property.".into(),
        };
        let value = match op {
            EditOp::Set(v) => (*v).clone(),
            EditOp::Reset => resolved.default_value.ok_or_else(invalid)?,
            EditOp::Step(_) => return Err(invalid()),
        };
        let binding = tx.data_binding(app, c.canvas, c.object)?;
        let sb = binding
            .series
            .iter_mut()
            .find(|s| s.id == c.series)
            .ok_or_else(|| PropertyError::UnknownTarget(address.target.describe()))?;
        if d.id == LABEL {
            let PropertyValue::Text(label) = value else {
                return Err(invalid());
            };
            sb.label = if matches!(op, EditOp::Reset) {
                None
            } else {
                Some(label)
            };
            return Ok(());
        }
        let SeriesEncoding::Line(line) = &mut sb.encoding else {
            unreachable!("encoding gate")
        };
        match (d.id, value) {
            (COLOR, PropertyValue::Color(color)) => line.color = ColorSource::Explicit(color),
            (STYLE, PropertyValue::Enum(v)) => {
                line.appearance.line_style = match v {
                    "solid" => LineStyle::Solid,
                    "dashed" => LineStyle::Dashed,
                    "dotted" => LineStyle::Dotted,
                    "dash_dot" => LineStyle::DashDot,
                    _ => return Err(invalid()),
                }
            }
            (MARKER, PropertyValue::Enum(v)) => {
                line.appearance.marker = if matches!(op, EditOp::Reset) {
                    None
                } else {
                    Some(match v {
                        "none" => Marker::None,
                        "circle" => Marker::Circle,
                        "square" => Marker::Square,
                        "diamond" => Marker::Diamond,
                        "triangle_up" => Marker::TriangleUp,
                        "triangle_down" => Marker::TriangleDown,
                        "plus" => Marker::Plus,
                        "cross" => Marker::Cross,
                        _ => return Err(invalid()),
                    })
                }
            }
            (MARKER_SIZE, PropertyValue::Float(v)) => {
                FloatBounds::inclusive(0.1, 20.).check(d.id, "marker diameter", v)?;
                line.appearance.marker_size = if matches!(op, EditOp::Reset) {
                    None
                } else {
                    Some(PositiveFiniteF32::new(v as f32).ok_or_else(invalid)?)
                };
            }
            (MARKER_FILL, PropertyValue::Color(color)) => {
                line.appearance.marker_fill = (!matches!(op, EditOp::Reset)).then_some(color);
            }
            (MARKER_EDGE_COLOR, PropertyValue::Color(color)) => {
                line.appearance.marker_edge_color = (!matches!(op, EditOp::Reset)).then_some(color);
            }
            (MARKER_EDGE_WIDTH, PropertyValue::Float(v)) => {
                MARKER_EDGE_BOUNDS.check(d.id, "marker edge width", v)?;
                // Zero is the "no outline" value, stored as no override.
                line.appearance.marker_edge_width = if matches!(op, EditOp::Reset) {
                    None
                } else {
                    PositiveFiniteF32::new(v as f32)
                };
            }
            (MARKER_HOLLOW, PropertyValue::Bool(hollow)) => line.appearance.marker_hollow = hollow,
            _ => return Err(invalid()),
        }
        Ok(())
    }
}
pub fn style_id(style: LineStyle) -> &'static str {
    match style {
        LineStyle::Solid => "solid",
        LineStyle::Dashed => "dashed",
        LineStyle::Dotted => "dotted",
        LineStyle::DashDot => "dash_dot",
    }
}
pub fn marker_id(marker: Marker) -> &'static str {
    match marker {
        Marker::None => "none",
        Marker::Circle => "circle",
        Marker::Square => "square",
        Marker::Diamond => "diamond",
        Marker::TriangleUp => "triangle_up",
        Marker::TriangleDown => "triangle_down",
        Marker::Plus => "plus",
        Marker::Cross => "cross",
    }
}
