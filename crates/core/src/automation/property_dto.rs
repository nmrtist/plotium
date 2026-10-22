//! Wire shapes of the property tools.
//!
//! The catalog model deliberately carries no `serde` derives: it is the
//! language-neutral semantic layer, and giving it a wire format would make the
//! serialized shape part of its public contract — every future field rename or
//! variant split would become a compatibility question for JSON callers. The
//! transport shapes live here instead, where changing one is a change to this
//! adapter alone.

use super::*;

#[derive(Serialize)]
pub(super) struct InspectValue {
    pub(super) property: &'static str,
    pub(super) canonical_label: &'static str,
    pub(super) tier: &'static str,
    pub(super) aggregate: AggregateValueDto,
    pub(super) readings: Vec<ReadingDto>,
}

#[derive(Serialize)]
pub(super) struct ReadingDto {
    target: TargetRef,
    value: AggregateValueDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_value: Option<PropertyValueDto>,
    availability: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    disabled_reason: Option<&'static str>,
    modified: bool,
    /// The user set this value by hand; writes skip it unless overridden.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    hand_set: bool,
    schema: ResolvedSchemaDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    guide_geometry: Option<serde_json::Value>,
}

#[derive(Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(super) enum AggregateValueDto {
    Uniform { value: PropertyValueDto },
    Mixed,
    Unavailable,
}

#[derive(Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub(super) enum PropertyValueDto {
    NormalizedPosition(Option<[f64; 2]>),
    AxisRange(range::AxisRangeDto),
    Bool(bool),
    Text(String),
    Int(i64),
    /// Domain value, even when the resolved UI schema displays degrees or a
    /// logarithm. Phase values therefore remain radians on the wire.
    Float(f64),
    Enum(&'static str),
    Color(String),
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum ResolvedSchemaDto {
    NormalizedPosition {
        input_schema: serde_json::Value,
    },
    AxisRange {
        unit: Option<String>,
        view: [f64; 2],
        manual_allowed: bool,
        input_schema: serde_json::Value,
    },
    UnitCode {
        codes: Vec<String>,
    },
    Bool,
    Text,
    Int {
        min: i64,
        max: i64,
        #[serde(skip_serializing_if = "Option::is_none")]
        drag_step: Option<f64>,
        #[serde(skip_serializing_if = "str::is_empty")]
        unit: &'static str,
    },
    SteppedInt {
        min: i64,
        max: i64,
        step: i64,
        drag_step: f64,
        #[serde(skip_serializing_if = "str::is_empty")]
        unit: &'static str,
    },
    Float {
        // Bounds validate the domain value carried by `PropertyValueDto::Float`.
        // `display` and `unit` describe only a UI projection of that value.
        min: f64,
        max: f64,
        exclusive_min: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        excluded: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        excluded_magnitude: Option<f64>,
        log: bool,
        unit: &'static str,
        display: &'static str,
    },
    Enum {
        variants: Vec<EnumVariantDto>,
    },
    Color {
        format: &'static str,
    },
}

#[derive(Serialize)]
pub(super) struct EnumVariantDto {
    id: &'static str,
    canonical_label: &'static str,
}

pub(super) fn reading(
    app: &PlotiumApp,
    resolved: &ResolvedProperty,
) -> Result<ReadingDto, AutomationError> {
    Ok(ReadingDto {
        target: resolved.address.target.clone(),
        value: aggregate_dto(&resolved.value),
        default_value: resolved.default_value.as_ref().map(value_dto),
        availability: match resolved.availability {
            Availability::Editable => "editable",
            Availability::Disabled(_) => "disabled",
            Availability::ReadOnly => "read_only",
        },
        disabled_reason: match resolved.availability {
            Availability::Disabled(reason) => Some(reason),
            Availability::Editable | Availability::ReadOnly => None,
        },
        modified: resolved.is_modified(),
        hand_set: app.is_hand_set(&resolved.address.target, resolved.address.definition),
        schema: schema_dto(&resolved.schema),
        guide_geometry: guides::geometry(app, &resolved.address)?,
    })
}

pub(super) fn aggregate_dto(value: &AggregateValue<PropertyValue>) -> AggregateValueDto {
    match value {
        AggregateValue::Uniform(value) => AggregateValueDto::Uniform {
            value: value_dto(value),
        },
        AggregateValue::Mixed => AggregateValueDto::Mixed,
        AggregateValue::Unavailable => AggregateValueDto::Unavailable,
    }
}

fn value_dto(value: &PropertyValue) -> PropertyValueDto {
    match value {
        PropertyValue::NormalizedPosition(value) => PropertyValueDto::NormalizedPosition(*value),
        PropertyValue::AxisRange(value) => PropertyValueDto::AxisRange(value.into()),
        PropertyValue::Bool(value) => PropertyValueDto::Bool(*value),
        PropertyValue::Text(value) => PropertyValueDto::Text(value.clone()),
        PropertyValue::Int(value) => PropertyValueDto::Int(*value),
        PropertyValue::Float(value) => PropertyValueDto::Float(*value),
        PropertyValue::Enum(value) => PropertyValueDto::Enum(value),
        PropertyValue::Color(value) => PropertyValueDto::Color(value.to_hex()),
    }
}

pub(super) fn schema_dto(schema: &ResolvedSchema) -> ResolvedSchemaDto {
    match schema {
        ResolvedSchema::NormalizedPosition => ResolvedSchemaDto::NormalizedPosition {
            input_schema: guides::input_schema(),
        },
        ResolvedSchema::AxisRange {
            unit,
            view,
            manual_allowed,
        } => ResolvedSchemaDto::AxisRange {
            unit: unit.clone(),
            view: *view,
            manual_allowed: *manual_allowed,
            input_schema: range::input_schema(),
        },
        ResolvedSchema::UnitCode { codes } => ResolvedSchemaDto::UnitCode {
            codes: codes.clone(),
        },
        ResolvedSchema::Bool => ResolvedSchemaDto::Bool,
        ResolvedSchema::Text => ResolvedSchemaDto::Text,
        ResolvedSchema::Int { min, max, unit } => ResolvedSchemaDto::Int {
            min: *min,
            max: *max,
            drag_step: None,
            unit,
        },
        ResolvedSchema::IntWithDrag {
            min,
            max,
            drag_step,
            unit,
        } => ResolvedSchemaDto::Int {
            min: *min,
            max: *max,
            drag_step: Some(*drag_step),
            unit,
        },
        ResolvedSchema::SteppedInt {
            min,
            max,
            step,
            drag_step,
            unit,
        } => ResolvedSchemaDto::SteppedInt {
            min: *min,
            max: *max,
            step: *step,
            drag_step: *drag_step,
            unit,
        },
        ResolvedSchema::Float { bounds, display } => float_schema_dto(*bounds, *display),
        ResolvedSchema::Enum { variants } => ResolvedSchemaDto::Enum {
            variants: variants.iter().copied().map(variant_dto).collect(),
        },
        ResolvedSchema::Color => ResolvedSchemaDto::Color { format: "#rrggbb" },
    }
}

fn float_schema_dto(bounds: FloatBounds, display: FloatDisplay) -> ResolvedSchemaDto {
    ResolvedSchemaDto::Float {
        min: bounds.min,
        max: bounds.max,
        exclusive_min: bounds.exclusive_min,
        excluded: bounds.excluded,
        excluded_magnitude: bounds.excluded_magnitude,
        log: matches!(display, FloatDisplay::Log10(_)),
        unit: display.unit(),
        display: display.as_str(),
    }
}

fn variant_dto(variant: &'static EnumVariant) -> EnumVariantDto {
    EnumVariantDto {
        id: variant.id,
        canonical_label: variant.canonical_label,
    }
}

/// The values the user set by hand that still apply, for an agent to read
/// before it edits: one reading per property, at most `limit` from `offset`.
pub fn hand_edit_readings(app: &PlotiumApp, offset: usize, limit: usize) -> serde_json::Value {
    let edits = app.live_hand_edits();
    let items: Vec<_> = edits
        .iter()
        .skip(offset)
        .take(limit)
        .filter_map(|address| {
            let resolved = app.resolve_property(address).ok()?;
            let definition = crate::properties::definition(address.definition)?;
            Some(serde_json::json!({
                "target": address.target,
                "key": address.definition.as_str(),
                "label": definition.canonical_label,
                "value": aggregate_dto(&resolved.value),
            }))
        })
        .collect();
    let end = offset.saturating_add(limit).min(edits.len());
    serde_json::json!({
        "items": items,
        "total": edits.len(),
        "next_offset": (end < edits.len()).then_some(end),
    })
}
