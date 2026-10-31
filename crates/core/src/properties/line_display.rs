//! Curve appearance edits own only one series' native presentation fields.
use super::provider::PropertyProvider;
use super::target::{resolved_schema, series_context};
use super::*;
use crate::state::PlotiumApp;
use plotium_figure::{
    ColorSource, HatchPattern, LineStyle, Marker, PositiveFiniteF32, SeriesEncoding,
};

pub const COLOR: PropertyId = PropertyId("series.line.color");
pub const LABEL: PropertyId = PropertyId("series.label");
pub const STYLE: PropertyId = PropertyId("series.line.style");
pub const MARKER: PropertyId = PropertyId("series.line.marker");
pub const MARKER_SIZE: PropertyId = PropertyId("series.line.marker_size");
pub const MARKER_FILL: PropertyId = PropertyId("series.line.marker_fill");
pub const MARKER_EDGE_COLOR: PropertyId = PropertyId("series.line.marker_edge_color");
pub const MARKER_EDGE_WIDTH: PropertyId = PropertyId("series.line.marker_edge_width");
pub const MARKER_HOLLOW: PropertyId = PropertyId("series.line.marker_hollow");
pub const MARKER_EVERY: PropertyId = PropertyId("series.line.marker_every");
pub const BAR_WIDTH: PropertyId = PropertyId("series.bar.width");
pub const BAR_EDGE_COLOR: PropertyId = PropertyId("series.bar.edge_color");
pub const BAR_EDGE_WIDTH: PropertyId = PropertyId("series.bar.edge_width");
pub const BAR_HATCH: PropertyId = PropertyId("series.bar.hatch");
pub const Y_AXIS: PropertyId = PropertyId("series.y_axis");
pub const LEFT: &str = "left";
pub const RIGHT: &str = "right";
const Y_AXES: &[EnumVariant] = &[
    EnumVariant::new(LEFT, "Left"),
    EnumVariant::new(RIGHT, "Right"),
];
const MAX_MARKER_EVERY: i64 = 1000;
const MARKER_EDGE_BOUNDS: FloatBounds = FloatBounds::inclusive(0., 5.);
const BAR_WIDTH_BOUNDS: FloatBounds = FloatBounds::inclusive(0.05, 1.);
const BAR_EDGE_BOUNDS: FloatBounds = FloatBounds::inclusive(0., 5.);
const HATCH_NONE: &str = "none";
const HATCH_FORWARD: &str = "forward";
const HATCH_BACKWARD: &str = "backward";
const HATCH_CROSS: &str = "cross";
pub(super) const HATCH_DEFAULT: &str = HATCH_NONE;
pub(super) const HATCHES: &[EnumVariant] = &[
    EnumVariant::new(HATCH_NONE, "None"),
    EnumVariant::new(HATCH_FORWARD, "Forward diagonal"),
    EnumVariant::new(HATCH_BACKWARD, "Backward diagonal"),
    EnumVariant::new(HATCH_CROSS, "Crosshatch"),
];
pub(super) fn hatch_id(hatch: Option<HatchPattern>) -> &'static str {
    match hatch {
        None => HATCH_NONE,
        Some(HatchPattern::Forward) => HATCH_FORWARD,
        Some(HatchPattern::Backward) => HATCH_BACKWARD,
        Some(HatchPattern::Cross) => HATCH_CROSS,
    }
}
/// `None` for an unknown id, `Some(None)` for no hatch.
pub(super) fn hatch_from_id(id: &str) -> Option<Option<HatchPattern>> {
    match id {
        HATCH_NONE => Some(None),
        HATCH_FORWARD => Some(Some(HatchPattern::Forward)),
        HATCH_BACKWARD => Some(Some(HatchPattern::Backward)),
        HATCH_CROSS => Some(Some(HatchPattern::Cross)),
        _ => None,
    }
}
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
    EnumVariant::new("star", "Star"),
    EnumVariant::new("pentagon", "Pentagon"),
    EnumVariant::new("hexagon", "Hexagon"),
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
    advanced(
        MARKER_EVERY,
        ValueSchema::Int {
            min: 1,
            max: MAX_MARKER_EVERY,
        },
        "Marker every N points",
    ),
    advanced(Y_AXIS, ValueSchema::Enum { variants: Y_AXES }, "Y axis"),
    advanced(
        BAR_WIDTH,
        ValueSchema::Float {
            bounds: BAR_WIDTH_BOUNDS,
            display: FloatDisplay::Linear(""),
            drag_step: Some(0.05),
        },
        "Bar width",
    ),
    advanced(BAR_EDGE_COLOR, ValueSchema::Color, "Bar outline color"),
    advanced(
        BAR_EDGE_WIDTH,
        ValueSchema::Float {
            bounds: BAR_EDGE_BOUNDS,
            display: FloatDisplay::Linear("pt"),
            drag_step: Some(0.05),
        },
        "Bar outline width",
    ),
    advanced(
        BAR_HATCH,
        ValueSchema::Enum { variants: HATCHES },
        "Bar hatch",
    ),
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
        let table_stack = plot.stack.mode == crate::state::StackMode::Offset
            && app.stacks_table_columns(&plot.binding, &plot.chart);
        let kind = if sb.source.columns().is_some() {
            line.kind
        } else if table_stack {
            plotium_figure::SeriesKind::Line
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
            MARKER_EVERY => (
                PropertyValue::Int(line.appearance.marker_stride() as i64),
                PropertyValue::Int(1),
                Some(line.appearance.marker_every.is_some()),
            ),
            Y_AXIS => (
                PropertyValue::Enum(if line.appearance.y_axis.is_right() {
                    RIGHT
                } else {
                    LEFT
                }),
                PropertyValue::Enum(LEFT),
                None,
            ),
            BAR_WIDTH => (
                PropertyValue::Float(line.bar_width()),
                PropertyValue::Float(plotium_figure::DEFAULT_BAR_WIDTH),
                Some(line.bar_width.is_some()),
            ),
            BAR_EDGE_COLOR => {
                let fill = line.color.resolve();
                (
                    PropertyValue::Color(crate::state::bar_edge_color(line.bar_edge_color, fill)),
                    // Unset, the outline is a near-black ink of the curve's hue.
                    PropertyValue::Color(crate::state::bar_edge_color(None, fill)),
                    Some(line.bar_edge_color.is_some()),
                )
            }
            BAR_EDGE_WIDTH => {
                // Unset, the outline follows the document's supporting lines.
                let aux = app.doc.style_library.figure_typography.aux_line_pt;
                let derived = plotium_figure::LineEncoding {
                    bar_edge_width: None,
                    ..line.clone()
                };
                (
                    PropertyValue::Float(line.bar_edge_width(aux)),
                    PropertyValue::Float(derived.bar_edge_width(aux)),
                    Some(line.bar_edge_width.is_some()),
                )
            }
            BAR_HATCH => (
                PropertyValue::Enum(hatch_id(line.bar_hatch)),
                PropertyValue::Enum(HATCH_DEFAULT),
                Some(line.bar_hatch.is_some()),
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
                | MARKER_EVERY
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
            } else if marker_property
                && matches!(
                    kind,
                    plotium_figure::SeriesKind::Stick
                        | plotium_figure::SeriesKind::Bar
                        | plotium_figure::SeriesKind::Box
                        | plotium_figure::SeriesKind::Violin
                )
            {
                Availability::Disabled("Choose a line or scatter curve to edit markers.")
            } else if d.id == BAR_EDGE_COLOR && kind != plotium_figure::SeriesKind::Bar {
                Availability::Disabled("Choose a bar curve to edit its outline color.")
            } else if d.id == BAR_HATCH && kind != plotium_figure::SeriesKind::Bar {
                Availability::Disabled("Choose a bar curve to edit its hatch.")
            } else if matches!(d.id, BAR_WIDTH | BAR_EDGE_WIDTH)
                && !matches!(
                    kind,
                    plotium_figure::SeriesKind::Bar
                        | plotium_figure::SeriesKind::Box
                        | plotium_figure::SeriesKind::Violin
                )
            {
                Availability::Disabled("Choose a bar, box or violin curve to edit its width.")
            } else if d.id == Y_AXIS && !crate::state::right_axis_supports(kind) {
                Availability::Disabled(crate::state::right_axis::RIGHT_AXIS_KINDS)
            } else if d.id == Y_AXIS
                && !line.appearance.y_axis.is_right()
                && app.doc.canvases[c.canvas]
                    .object(c.object)
                    .and_then(|o| o.plot())
                    .and_then(|p| p.binding.coordinates.as_ref())
                    .is_some_and(|coordinates| coordinates.transposed)
            {
                Availability::Disabled(crate::state::right_axis::HORIZONTAL_ONE_VALUE_AXIS)
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
        // The first curve on a y axis fixes that axis's unit, as the first
        // curve fixes the left one at creation; later curves must convert.
        let moved = match (d.id, &value) {
            (Y_AXIS, PropertyValue::Enum(side)) => {
                let side = if *side == RIGHT {
                    plotium_figure::YAxisSide::Right
                } else {
                    plotium_figure::YAxisSide::Left
                };
                curve_y_unit(app, c.canvas, c.object, c.series)
                    .map_err(|message| PropertyError::InvalidValue {
                        property: d.id,
                        message,
                    })?
                    .map(|unit| (side, unit))
            }
            _ => None,
        };
        let binding = tx.data_binding(app, c.canvas, c.object)?;
        let mut unit_replaced = None;
        if let Some((side, unit)) = moved {
            let shared = binding
                .series
                .iter()
                .any(|s| s.id != c.series && s.y_axis() == side);
            if let Some(coordinates) = binding.coordinates.as_mut() {
                let axis_unit = if side.is_right() {
                    &mut coordinates.y2
                } else {
                    &mut coordinates.y
                };
                if !shared {
                    if *axis_unit != unit {
                        unit_replaced = Some(side);
                    }
                    *axis_unit = unit;
                } else if let Err(message) =
                    crate::state::CurveUnitConversion::new(&unit, axis_unit, false)
                {
                    return Err(PropertyError::InvalidValue {
                        property: d.id,
                        message,
                    });
                }
            }
        }
        if let Some(side) = unit_replaced {
            // Ranges and display units were written in the replaced unit.
            tx.axis_overrides(app, c.canvas, c.object)?
                .forget_axis_units(side);
        }
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
        // A hand-chosen colour survives re-theming; resetting the curve colour
        // hands it back to the theme, while marker resets only drop their override.
        match (d.id, op) {
            (COLOR, EditOp::Reset) => line.color_locked = false,
            (COLOR | MARKER_FILL | MARKER_EDGE_COLOR, EditOp::Set(_)) => line.color_locked = true,
            _ => {}
        }
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
                        "star" => Marker::Star,
                        "pentagon" => Marker::Pentagon,
                        "hexagon" => Marker::Hexagon,
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
            (MARKER_EVERY, PropertyValue::Int(n)) => {
                if !(1..=MAX_MARKER_EVERY).contains(&n) {
                    return Err(PropertyError::InvalidValue {
                        property: d.id,
                        message: format!(
                            "marker spacing {n} is out of range: it must be between 1 and {MAX_MARKER_EVERY}"
                        ),
                    });
                }
                // One is "every point", stored as no override.
                line.appearance.marker_every = u32::try_from(n)
                    .ok()
                    .and_then(std::num::NonZeroU32::new)
                    .filter(|n| n.get() > 1);
            }
            (Y_AXIS, PropertyValue::Enum(v)) => {
                line.appearance.y_axis = match v {
                    LEFT => plotium_figure::YAxisSide::Left,
                    RIGHT => plotium_figure::YAxisSide::Right,
                    _ => return Err(invalid()),
                }
            }
            (BAR_EDGE_COLOR, PropertyValue::Color(color)) => {
                line.bar_edge_color = (!matches!(op, EditOp::Reset)).then_some(color);
            }
            (BAR_HATCH, PropertyValue::Enum(v)) => {
                line.bar_hatch = hatch_from_id(v).ok_or_else(invalid)?;
            }
            (BAR_EDGE_WIDTH, PropertyValue::Float(v)) => {
                BAR_EDGE_BOUNDS.check(d.id, "bar outline width", v)?;
                // Zero is a stored "no outline", distinct from the default.
                line.bar_edge_width = if matches!(op, EditOp::Reset) {
                    None
                } else {
                    Some(plotium_figure::FiniteF64::new(v).ok_or_else(invalid)?)
                };
            }
            (BAR_WIDTH, PropertyValue::Float(v)) => {
                BAR_WIDTH_BOUNDS.check(d.id, "bar width", v)?;
                line.bar_width = if matches!(op, EditOp::Reset) {
                    None
                } else {
                    Some(plotium_figure::UnitInterval::new(v).ok_or_else(invalid)?)
                };
            }
            _ => return Err(invalid()),
        }
        Ok(())
    }
}
/// The y unit a curve brings to the axis it moves to: `Some(unit)` for an
/// explicit-coordinate curve (the unit itself may be unknown), `None` for a
/// plot without explicit coordinates.
fn curve_y_unit(
    app: &PlotiumApp,
    canvas: usize,
    object: crate::state::ObjectId,
    series: crate::state::SeriesId,
) -> Result<Option<Option<plotium_data::UnitSpec>>, String> {
    let Some(plot) = app.doc.canvases[canvas]
        .object(object)
        .and_then(|o| o.plot())
        .filter(|plot| plot.binding.coordinates.is_some())
    else {
        return Ok(None);
    };
    let Some(sb) = plot.binding.series.iter().find(|s| s.id == series) else {
        return Ok(None);
    };
    let (table, selections) = app.table_curve_sources(sb.source)?;
    let columns = selections.first().ok_or("The curve selects no columns.")?;
    Ok(Some(
        crate::state::table_curve_coordinates(table, *columns)?.y,
    ))
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
        Marker::Star => "star",
        Marker::Pentagon => "pentagon",
        Marker::Hexagon => "hexagon",
        Marker::Plus => "plus",
        Marker::Cross => "cross",
    }
}
