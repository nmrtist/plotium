//! `properties.set_many`: several exact `(target, key, value)` writes in one
//! plan, one atomic commit and one undo step.
//!
//! A single-key `properties.set` writes one value to many targets; styling six
//! curves with six colours needed six plans. Each write here names an exact
//! target, so a write that does not apply is a caller error and refuses the
//! whole plan. Only a write whose target already holds the value is skipped.

use super::properties::{decode_value, property_error, skipped_results};
use super::registry::parse;
use super::*;
use crate::properties::{PropertyAccess, PropertyId, PropertyValue, SkipReason, definition_by_key};
use crate::state::PlotiumApp;
use serde::Deserialize;

pub const TOOL_SET_MANY: &str = "properties.set_many";
pub const MAX_WRITES: usize = 200;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PropertyBatchParams {
    pub writes: Vec<PropertyBatchWrite>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PropertyBatchWrite {
    pub target: TargetRef,
    pub key: String,
    pub value: serde_json::Value,
}

pub(super) fn schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "writes": {
                "type": "array",
                "minItems": 1,
                "maxItems": MAX_WRITES,
                "items": {
                    "type": "object",
                    "properties": {
                        "target": {"type": "object", "description": "exact TargetRef; include the Series component for series properties"},
                        "key": {"type": "string"},
                        "value": {"description": "typed per the addressed property's value schema"},
                    },
                    "required": ["target", "key", "value"],
                    "additionalProperties": false,
                },
            },
        },
        "required": ["writes"],
        "additionalProperties": false,
    })
}

type Write = (TargetRef, PropertyId, PropertyValue);

/// Decode every write, refusing the batch on the first malformed one.
pub(super) fn decode(parameters: &serde_json::Value) -> Result<Vec<Write>, AutomationError> {
    let params: PropertyBatchParams = parse(TOOL_SET_MANY, parameters.clone())?;
    if params.writes.is_empty() || params.writes.len() > MAX_WRITES {
        return Err(invalid(format!(
            "writes must contain 1..={MAX_WRITES} items"
        )));
    }
    let mut writes: Vec<Write> = Vec::with_capacity(params.writes.len());
    for (index, write) in params.writes.into_iter().enumerate() {
        let definition = definition_by_key(&write.key)
            .ok_or_else(|| invalid(format!("writes[{index}]: unknown property '{}'", write.key)))?;
        if definition.access == PropertyAccess::ReadOnly {
            return Err(invalid(format!(
                "writes[{index}]: property '{}' is read-only",
                definition.id
            )));
        }
        // Two values for one address would make the result depend on order,
        // which a reviewer of the plan cannot see.
        if writes
            .iter()
            .any(|(target, id, _)| *target == write.target && *id == definition.id)
        {
            return Err(invalid(format!(
                "writes[{index}]: '{}' is written twice to the same target",
                definition.id
            )));
        }
        let value = decode_value(TOOL_SET_MANY, definition, &write.value)
            .map_err(|error| prefix(index, error))?;
        writes.push((write.target, definition.id, value));
    }
    Ok(writes)
}

/// The targets a batch addresses, frozen like any other component selection.
pub(super) fn freeze(
    provider: &impl ResourceProvider,
    request: &ToolRequest,
) -> Result<FrozenTargetSet, AutomationError> {
    if request.targets != (TargetSelector::Explicit { ids: Vec::new() }) {
        return Err(AutomationError::InvalidSelector(
            "properties.set_many names exact targets inside writes; omit the top-level selector"
                .into(),
        ));
    }
    let mut targets: Vec<TargetRef> = Vec::new();
    let mut resources: Vec<ResourceRef> = Vec::new();
    for (index, (target, _, _)) in decode(&request.parameters)?.into_iter().enumerate() {
        // Same identity rule as a component selector; a resource-level write
        // (an axis title, a legend setting) names no component.
        let valid = provider
            .inspect(&target.resource.id)
            .is_some_and(|descriptor| descriptor.resource == target.resource)
            && (target.component.is_none() || provider.component_exists(&target));
        if !valid {
            return Err(AutomationError::InvalidSelector(format!(
                "writes[{index}]: invalid target {}",
                target.describe()
            )));
        }
        if !resources.contains(&target.resource) {
            resources.push(target.resource.clone());
        }
        if !targets.contains(&target) {
            targets.push(target);
        }
    }
    Ok(FrozenTargetSet {
        revision: provider.revision(),
        total_matches: targets.len(),
        targets: resources,
        component_targets: targets,
        reasons: vec![],
        truncated: false,
    })
}

/// One planned row per write, in write order. Planning fails unless every
/// write applies, so the preview is exactly what execution will commit.
pub(super) fn refine_plan(
    app: &PlotiumApp,
    request: &ToolRequest,
) -> Result<Vec<PlannedTarget>, AutomationError> {
    let writes = decode(&request.parameters)?;
    for (index, (target, property, _)) in writes.iter().enumerate() {
        app.resolve_property(&crate::properties::PropertyAddress::new(
            target.clone(),
            *property,
        ))
        .map_err(|error| invalid(format!("writes[{index}] ({property}): {error}")))?;
    }
    let outcomes = outcomes(&writes, &plan(app, &writes)?)?;
    Ok(writes
        .iter()
        .zip(outcomes)
        .map(|((target, property, _), outcome)| PlannedTarget {
            target: target.clone(),
            status: TargetCompatibility::Compatible,
            reason: match outcome {
                Outcome::Applied => format!("{property} will be set"),
                Outcome::Unchanged(message) => format!("{property}: {message}"),
            },
        })
        .collect())
}

pub(super) fn execute(
    app: &mut PlotiumApp,
    plan: &ToolPlan,
) -> Result<ToolResult, AutomationError> {
    let writes = decode(&plan.request.parameters)?;
    let commit = self::plan(app, &writes)?;
    let outcomes = outcomes(&writes, &commit)?;
    let changes_document = commit.has_document_action();
    let nothing_applied = commit.applied.is_empty();
    let mut skipped = skipped_results(&commit.skipped).into_iter();
    let before = DocumentRevision(app.doc.automation_revision);
    if !nothing_applied {
        app.commit_property(commit);
    }
    let after = DocumentRevision(app.doc.automation_revision);
    let mut targets = Vec::with_capacity(writes.len());
    let mut report = Vec::with_capacity(writes.len());
    let mut modified: Vec<ResourceRef> = Vec::new();
    for (index, ((target, property, _), outcome)) in writes.iter().zip(&outcomes).enumerate() {
        let applied = matches!(outcome, Outcome::Applied);
        if applied {
            if !modified.contains(&target.resource) {
                modified.push(target.resource.clone());
            }
            targets.push(TargetResult {
                target: target.clone(),
                outcome: TargetOutcome::Succeeded,
                message: format!("{property} set"),
                skip_reason: None,
                fingerprints: Vec::new(),
            });
        } else if let Some(result) = skipped.next() {
            targets.push(result);
        }
        report.push(serde_json::json!({
            "index": index,
            "key": property.as_str(),
            "outcome": if applied { "succeeded" } else { "skipped" },
        }));
    }
    Ok(ToolResult {
        tool_id: plan.request.tool_id.clone(),
        before_revision: before,
        after_revision: after,
        targets,
        produced: Vec::new(),
        modified,
        diagnostics: Vec::new(),
        verification: vec![VerificationRecord {
            check: "revision_advanced".to_owned(),
            passed: nothing_applied || !changes_document || after > before,
            message: if nothing_applied {
                "every target already held its value; nothing was committed".to_owned()
            } else {
                "atomic document commit completed".to_owned()
            },
        }],
        value: serde_json::json!({ "writes": report }),
    })
}

fn plan(
    app: &PlotiumApp,
    writes: &[Write],
) -> Result<crate::properties::PropertyCommit, AutomationError> {
    app.plan_property_writes(writes)
        .map_err(|error| property_error(TOOL_SET_MANY, error))
}

enum Outcome {
    Applied,
    Unchanged(String),
}

/// Pair each write with its outcome. The planner pushes every write onto
/// exactly one of `applied` or `skipped`, in write order, so walking both
/// lists in step recovers the pairing without keying skips by property.
fn outcomes(
    writes: &[Write],
    commit: &crate::properties::PropertyCommit,
) -> Result<Vec<Outcome>, AutomationError> {
    let mut applied = commit.applied.iter().peekable();
    let mut skipped = commit.skipped.iter();
    writes
        .iter()
        .enumerate()
        .map(|(index, (target, property, _))| {
            if applied
                .peek()
                .is_some_and(|a| a.target == *target && a.definition == *property)
            {
                applied.next();
                return Ok(Outcome::Applied);
            }
            match skipped.next() {
                Some(skip) if skip.reason == SkipReason::AlreadyAtValue => {
                    Ok(Outcome::Unchanged(skip.message.clone()))
                }
                Some(skip) => Err(invalid(format!(
                    "writes[{index}] ({property}): {}; nothing was committed",
                    skip.message
                ))),
                None => Err(AutomationError::Execution(format!(
                    "writes[{index}] ({property}) has no planner outcome"
                ))),
            }
        })
        .collect()
}

fn invalid(message: String) -> AutomationError {
    AutomationError::InvalidParameters {
        tool_id: TOOL_SET_MANY.to_owned(),
        message,
    }
}

fn prefix(index: usize, error: AutomationError) -> AutomationError {
    match error {
        AutomationError::InvalidParameters { tool_id, message } => {
            AutomationError::InvalidParameters {
                tool_id,
                message: format!("writes[{index}]: {message}"),
            }
        }
        other => other,
    }
}

#[cfg(test)]
#[path = "property_batch_tests.rs"]
mod tests;
