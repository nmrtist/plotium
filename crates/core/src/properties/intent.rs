//! The author's stated intent of a plot: what it should show and the one
//! claim it should support. Nothing reads these while drawing.
use super::provider::PropertyProvider;
use super::target::require_plot_object_target;
use super::*;
use crate::state::{PlotIntent, PlotPurpose, PlotiumApp};

pub const PURPOSE: PropertyId = PropertyId("object.intent.purpose");
pub const CLAIM: PropertyId = PropertyId("object.intent.claim");
const UNSPECIFIED: &str = "unspecified";
/// `unspecified` first, then every `PlotPurpose` in declaration order.
const PURPOSES: &[EnumVariant] = &[
    EnumVariant::new(UNSPECIFIED, "Not stated"),
    EnumVariant::new(PlotPurpose::Compare.id(), "Compare groups"),
    EnumVariant::new(PlotPurpose::Trend.id(), "Trend"),
    EnumVariant::new(PlotPurpose::Distribution.id(), "Distribution"),
    EnumVariant::new(PlotPurpose::Relationship.id(), "Relationship"),
    EnumVariant::new(PlotPurpose::Composition.id(), "Composition"),
    EnumVariant::new(PlotPurpose::Change.id(), "Change or ratio"),
    EnumVariant::new(PlotPurpose::Detail.id(), "Local detail"),
];

const fn definition(
    id: PropertyId,
    value_schema: ValueSchema,
    default: PropertyValue,
    label: &'static str,
    aliases: &'static [&'static str],
) -> PropertyDefinition {
    PropertyDefinition {
        id,
        scope_kind: ScopeKind::Object,
        value_schema,
        access: PropertyAccess::ReadWrite,
        applicability: Applicability::component(ComponentKind::None),
        default_policy: DefaultPolicy::Fixed(default),
        tier: Tier::Advanced,
        copies: ValueCopies::PerTarget,
        canonical_label: label,
        canonical_aliases: aliases,
    }
}

const DEFINITIONS: &[PropertyDefinition] = &[
    definition(
        PURPOSE,
        ValueSchema::Enum { variants: PURPOSES },
        PropertyValue::Enum(UNSPECIFIED),
        "Purpose",
        &["plot purpose", "chart intent"],
    ),
    definition(
        CLAIM,
        ValueSchema::Text,
        PropertyValue::Text(String::new()),
        "Claim",
        &["plot claim", "key message"],
    ),
];

pub(crate) struct IntentProvider;
pub(crate) static PROVIDER: IntentProvider = IntentProvider;

fn invalid(id: PropertyId, message: impl Into<String>) -> PropertyError {
    PropertyError::InvalidValue {
        property: id,
        message: message.into(),
    }
}

fn purpose_value(intent: &PlotIntent) -> PropertyValue {
    PropertyValue::Enum(intent.purpose.map_or(UNSPECIFIED, PlotPurpose::id))
}

impl PropertyProvider for IntentProvider {
    fn definitions(&self) -> &'static [PropertyDefinition] {
        DEFINITIONS
    }

    fn read(
        &self,
        app: &PlotiumApp,
        address: &PropertyAddress,
    ) -> Result<ResolvedProperty, PropertyError> {
        let d = definition_of(address)?;
        let (canvas, object) = require_plot_object_target(app, &address.target, d)?;
        let intent = &app.doc.canvases[canvas]
            .object(object)
            .and_then(|o| o.plot())
            .ok_or_else(|| PropertyError::UnknownTarget(address.target.describe()))?
            .intent;
        let (value, modified, schema) = if d.id == PURPOSE {
            (
                purpose_value(intent),
                intent.purpose.is_some(),
                ResolvedSchema::Enum {
                    variants: PURPOSES.iter().collect(),
                },
            )
        } else {
            (
                PropertyValue::Text(intent.claim.clone()),
                !intent.claim.is_empty(),
                ResolvedSchema::Text,
            )
        };
        let DefaultPolicy::Fixed(default) = &d.default_policy else {
            unreachable!("intent defaults are fixed")
        };
        Ok(ResolvedProperty {
            address: address.clone(),
            value: AggregateValue::Uniform(value),
            default_value: Some(default.clone()),
            modified: Some(modified),
            availability: Availability::Editable,
            schema,
        })
    }

    fn edit(
        &self,
        app: &PlotiumApp,
        tx: &mut PropertyTransaction,
        address: &PropertyAddress,
        op: &EditOp<'_>,
    ) -> Result<(), PropertyError> {
        let d = definition_of(address)?;
        let (canvas, object) = require_plot_object_target(app, &address.target, d)?;
        let intent = tx.plot_intent(app, canvas, object)?;
        match (d.id, op) {
            (PURPOSE, EditOp::Reset) => intent.purpose = None,
            (PURPOSE, EditOp::Set(PropertyValue::Enum(UNSPECIFIED))) => intent.purpose = None,
            (PURPOSE, EditOp::Set(PropertyValue::Enum(id))) => {
                intent.purpose = Some(
                    PlotPurpose::from_id(id)
                        .ok_or_else(|| invalid(d.id, format!("Unknown purpose {id}.")))?,
                );
            }
            (CLAIM, EditOp::Reset) => intent.claim.clear(),
            (CLAIM, EditOp::Set(PropertyValue::Text(text))) => {
                intent.claim = PlotIntent::normalized_claim(text).map_err(|e| invalid(d.id, e))?;
            }
            (_, EditOp::Step(_)) => return Err(invalid(d.id, "No step gesture")),
            _ => return Err(invalid(d.id, "Value does not match this property.")),
        }
        Ok(())
    }
}

fn definition_of(address: &PropertyAddress) -> Result<&'static PropertyDefinition, PropertyError> {
    DEFINITIONS
        .iter()
        .find(|d| d.id == address.definition)
        .ok_or_else(|| PropertyError::UnknownProperty(address.definition.to_string()))
}
