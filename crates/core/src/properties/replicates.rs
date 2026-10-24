//! What each plotted value summarizes: the replicate count and what its
//! error bars show. The user states both; nothing reads them while drawing.
use super::provider::PropertyProvider;
use super::target::series_context;
use super::*;
use crate::state::{ErrorBarKind, PlotiumApp, SeriesBinding};
use std::num::NonZeroU32;

pub const REPLICATES: PropertyId = PropertyId("series.replicates");
pub const ERROR_BARS: PropertyId = PropertyId("series.error_bars");
const MAX_REPLICATES: i64 = 100_000;
const UNSPECIFIED: &str = "unspecified";
/// `unspecified` first, then every `ErrorBarKind` in declaration order.
const ERROR_KINDS: &[EnumVariant] = &[
    EnumVariant::new(UNSPECIFIED, "Not stated"),
    EnumVariant::new(ErrorBarKind::Sd.id(), "SD"),
    EnumVariant::new(ErrorBarKind::Sem.id(), "SEM"),
    EnumVariant::new(ErrorBarKind::Ci95.id(), "95% CI"),
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
        applicability: Applicability::encoding(ComponentKind::Series, EncodingKind::Line),
        default_policy: DefaultPolicy::Fixed(default),
        tier: Tier::Advanced,
        copies: ValueCopies::PerTarget,
        canonical_label: label,
        canonical_aliases: aliases,
    }
}

const DEFINITIONS: &[PropertyDefinition] = &[
    definition(
        REPLICATES,
        ValueSchema::Int {
            min: 0,
            max: MAX_REPLICATES,
        },
        // Zero stands for a count nobody stated.
        PropertyValue::Int(0),
        "Replicates (n)",
        &["sample size", "number of replicates"],
    ),
    definition(
        ERROR_BARS,
        ValueSchema::Enum {
            variants: ERROR_KINDS,
        },
        PropertyValue::Enum(UNSPECIFIED),
        "Error bars show",
        &["error type", "sd or sem"],
    ),
];

pub(crate) struct ReplicatesProvider;
pub(crate) static PROVIDER: ReplicatesProvider = ReplicatesProvider;

fn invalid(id: PropertyId, message: impl Into<String>) -> PropertyError {
    PropertyError::InvalidValue {
        property: id,
        message: message.into(),
    }
}

fn definition_of(address: &PropertyAddress) -> Result<&'static PropertyDefinition, PropertyError> {
    DEFINITIONS
        .iter()
        .find(|d| d.id == address.definition)
        .ok_or_else(|| PropertyError::UnknownProperty(address.definition.to_string()))
}

fn binding<'a>(
    app: &'a PlotiumApp,
    address: &PropertyAddress,
    d: &'static PropertyDefinition,
) -> Result<&'a SeriesBinding, PropertyError> {
    let c = series_context(app, &address.target, d)?;
    app.doc.canvases[c.canvas]
        .object(c.object)
        .and_then(|o| o.plot())
        .and_then(|p| p.binding.series.iter().find(|s| s.id == c.series))
        .ok_or_else(|| PropertyError::UnknownTarget(address.target.describe()))
}

fn has_error_column(series: &SeriesBinding) -> bool {
    series
        .source
        .columns()
        .is_some_and(|c| c.y_error_column_id.is_some())
}

impl PropertyProvider for ReplicatesProvider {
    fn definitions(&self) -> &'static [PropertyDefinition] {
        DEFINITIONS
    }

    fn read(
        &self,
        app: &PlotiumApp,
        address: &PropertyAddress,
    ) -> Result<ResolvedProperty, PropertyError> {
        let d = definition_of(address)?;
        let series = binding(app, address, d)?;
        let (value, modified, availability, schema) = if d.id == REPLICATES {
            (
                PropertyValue::Int(series.replicates.map_or(0, |n| i64::from(n.get()))),
                series.replicates.is_some(),
                Availability::Editable,
                ResolvedSchema::Int {
                    min: 0,
                    max: MAX_REPLICATES,
                    unit: "",
                },
            )
        } else {
            (
                PropertyValue::Enum(series.error_bars.map_or(UNSPECIFIED, ErrorBarKind::id)),
                series.error_bars.is_some(),
                if has_error_column(series) {
                    Availability::Editable
                } else {
                    Availability::Disabled("Bind a y error column to this curve first.")
                },
                ResolvedSchema::Enum {
                    variants: ERROR_KINDS.iter().collect(),
                },
            )
        };
        let DefaultPolicy::Fixed(default) = &d.default_policy else {
            unreachable!("replicate defaults are fixed")
        };
        Ok(ResolvedProperty {
            address: address.clone(),
            value: AggregateValue::Uniform(value),
            default_value: Some(default.clone()),
            modified: Some(modified),
            availability,
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
        // Reset stays open so a value left from a removed error column can clear.
        if let Availability::Disabled(reason) = self.read(app, address)?.availability
            && !matches!(op, EditOp::Reset)
        {
            return Err(PropertyError::NotApplicable(reason.into()));
        }
        let c = series_context(app, &address.target, d)?;
        let series = tx
            .data_binding(app, c.canvas, c.object)?
            .series
            .iter_mut()
            .find(|s| s.id == c.series)
            .ok_or_else(|| PropertyError::UnknownTarget(address.target.describe()))?;
        match (d.id, op) {
            (REPLICATES, EditOp::Reset) => series.replicates = None,
            (REPLICATES, EditOp::Set(PropertyValue::Int(n))) => {
                if !(0..=MAX_REPLICATES).contains(n) {
                    return Err(invalid(d.id, format!("Replicates are 0–{MAX_REPLICATES}.")));
                }
                series.replicates = u32::try_from(*n).ok().and_then(NonZeroU32::new);
            }
            (ERROR_BARS, EditOp::Reset) => series.error_bars = None,
            (ERROR_BARS, EditOp::Set(PropertyValue::Enum(UNSPECIFIED))) => {
                series.error_bars = None;
            }
            (ERROR_BARS, EditOp::Set(PropertyValue::Enum(id))) => {
                series.error_bars = Some(
                    ErrorBarKind::from_id(id)
                        .ok_or_else(|| invalid(d.id, format!("Unknown error bar kind {id}.")))?,
                );
            }
            (_, EditOp::Step(_)) => return Err(invalid(d.id, "No step gesture")),
            _ => return Err(invalid(d.id, "Value does not match this property.")),
        }
        Ok(())
    }
}
