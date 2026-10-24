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
use crate::properties::{
    PropertyAccess, PropertyId, PropertySkip, PropertyValue, SkipReason, definition_by_key,
};
use crate::state::PlotiumApp;
use serde::Deserialize;

pub const TOOL_SET_MANY: &str = "properties.set_many";
pub const MAX_WRITES: usize = 200;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PropertyBatchParams {
    pub writes: Vec<PropertyBatchWrite>,
    #[serde(default)]
    pub override_hand_edits: bool,
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
            "override_hand_edits": {"type": "boolean", "description": super::registry::OVERRIDE_HAND_EDITS},
        },
        "required": ["writes"],
        "additionalProperties": false,
    })
}

type Write = (TargetRef, PropertyId, PropertyValue);

/// Whether the batch carries the user's request to change hand-set values.
fn overrides_hand_edits(parameters: &serde_json::Value) -> Result<bool, AutomationError> {
    Ok(parse::<PropertyBatchParams>(TOOL_SET_MANY, parameters.clone())?.override_hand_edits)
}

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
    let planned = self::plan(app, &writes, overrides_hand_edits(&request.parameters)?)?;
    let outcomes = outcomes(&writes, &planned)?;
    Ok(writes
        .iter()
        .zip(outcomes)
        .map(|((target, property, _), outcome)| PlannedTarget {
            target: target.clone(),
            status: if matches!(outcome, Outcome::HandSet(_)) {
                TargetCompatibility::Skipped
            } else {
                TargetCompatibility::Compatible
            },
            reason: match outcome {
                Outcome::Applied => format!("{property} will be set"),
                Outcome::Unchanged(message) | Outcome::HandSet(message) => {
                    format!("{property}: {message}")
                }
            },
        })
        .collect())
}

pub(super) fn execute(
    app: &mut PlotiumApp,
    plan: &ToolPlan,
) -> Result<ToolResult, AutomationError> {
    let writes = decode(&plan.request.parameters)?;
    let on_users_behalf = overrides_hand_edits(&plan.request.parameters)?;
    let planned = self::plan(app, &writes, on_users_behalf)?;
    let outcomes = outcomes(&writes, &planned)?;
    let Planned { commit, held } = planned;
    let changes_document = commit.has_document_action();
    let nothing_applied = commit.applied.is_empty();
    let mut skipped = skipped_results(&commit.skipped).into_iter();
    let before = DocumentRevision(app.doc.automation_revision);
    if !nothing_applied && on_users_behalf {
        app.commit_property(commit);
    } else if !nothing_applied {
        app.commit_automation_property(commit);
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
        } else if let Some(skip) = &held[index] {
            targets.extend(skipped_results(std::slice::from_ref(skip)));
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
                "nothing was committed; see each target's reason".to_owned()
            } else {
                "atomic document commit completed".to_owned()
            },
        }],
        value: serde_json::json!({ "writes": report }),
    })
}

/// A batch's commit, and for each write the skip that kept it off a value the
/// user set by hand.
struct Planned {
    commit: crate::properties::PropertyCommit,
    held: Vec<Option<PropertySkip>>,
}

fn plan(
    app: &PlotiumApp,
    writes: &[Write],
    on_users_behalf: bool,
) -> Result<Planned, AutomationError> {
    let held: Vec<_> = writes
        .iter()
        .map(|(target, property, _)| {
            if on_users_behalf {
                return None;
            }
            let (_, mut skips) = app.withhold_hand_edits(*property, std::slice::from_ref(target));
            skips.pop()
        })
        .collect();
    let free: Vec<_> = writes
        .iter()
        .zip(&held)
        .filter(|(_, held)| held.is_none())
        .map(|(write, _)| write.clone())
        .collect();
    let commit = app
        .plan_property_writes(&free)
        .map_err(|error| property_error(TOOL_SET_MANY, error))?;
    if !on_users_behalf {
        super::properties::refuse_hand_set_fan_out(app, TOOL_SET_MANY, &commit)?;
    }
    Ok(Planned { commit, held })
}

/// The commit a batch would make, planned against `app` without running it.
pub(super) fn candidate_commit(
    app: &PlotiumApp,
    plan: &ToolPlan,
) -> Result<crate::properties::PropertyCommit, AutomationError> {
    let writes = decode(&plan.request.parameters)?;
    let on_users_behalf = overrides_hand_edits(&plan.request.parameters)?;
    let planned = self::plan(app, &writes, on_users_behalf)?;
    // A write the copy cannot apply must fail here, as it would on execution.
    outcomes(&writes, &planned)?;
    Ok(planned.commit)
}

enum Outcome {
    Applied,
    Unchanged(String),
    HandSet(String),
}

/// Pair each write with its outcome. The planner pushes every write onto
/// exactly one of `applied` or `skipped`, in write order, so walking both
/// lists in step recovers the pairing without keying skips by property.
fn outcomes(writes: &[Write], planned: &Planned) -> Result<Vec<Outcome>, AutomationError> {
    let mut applied = planned.commit.applied.iter().peekable();
    let mut skipped = planned.commit.skipped.iter();
    writes
        .iter()
        .enumerate()
        .map(|(index, (target, property, _))| {
            if let Some(skip) = &planned.held[index] {
                return Ok(Outcome::HandSet(skip.message.clone()));
            }
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
