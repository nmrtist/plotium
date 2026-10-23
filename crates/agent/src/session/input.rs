use super::*;

impl Plan {
    pub(super) fn parse(arguments: Value) -> Result<Self, Failure> {
        field::<String>(&arguments, "tool_id")?;
        field::<serde_json::Map<String, Value>>(&arguments, "parameters")?;
        field::<DocumentRevision>(&arguments, "expected_revision")?;
        for key in ["ids", "targets"] {
            if arguments.get(key).is_some_and(|v| !v.is_array()) {
                return Err(
                    Failure::new("invalid_parameters", format!("{key} must be an array"))
                        .at_field(key),
                );
            }
        }
        let args: Self = decode(arguments)?;
        use crate::capabilities::{EDIT_OPERATIONS, NO_SELECTOR, SINGLE_DATASET};
        // Unknown operations are diagnosed by dispatch, without guessing a selector policy.
        if !EDIT_OPERATIONS.contains(&args.tool_id.as_str()) {
            return Ok(args);
        }
        let valid = if NO_SELECTOR.contains(&args.tool_id.as_str()) {
            args.ids.is_none() && args.targets.is_none()
        } else if SINGLE_DATASET.contains(&args.tool_id.as_str()) {
            args.targets.is_none() && args.ids.as_ref().is_some_and(|ids| ids.len() == 1)
        } else {
            match (&args.ids, &args.targets) {
                (Some(ids), None) => !ids.is_empty(),
                (None, Some(targets)) => {
                    matches!(args.tool_id.as_str(), "properties.set" | "properties.reset")
                        && !targets.is_empty()
                        && targets
                            .iter()
                            .all(|t| matches!(t.component, Some(ComponentRef::Series(_))))
                }
                _ => false,
            }
        };
        if !valid {
            return Err(Failure::new(
                "invalid_parameters",
                "Invalid selector: omit ids/targets for operations with embedded targets; otherwise supply exactly one nonempty selector. Only properties.set/reset accept Series targets; table operations require one dataset id.",
            ));
        }
        Ok(args)
    }
}

fn field<T: serde::de::DeserializeOwned>(arguments: &Value, key: &str) -> Result<(), Failure> {
    let value = arguments.get(key).ok_or_else(|| {
        Failure::new("invalid_parameters", format!("Missing field {key}")).at_field(key)
    })?;
    decode::<T>(value.clone())
        .map(|_| ())
        .map_err(|e| e.at_field(key))
}

impl Execute {
    pub(super) fn parse(arguments: Value) -> Result<Self, Failure> {
        field::<String>(&arguments, "plan_id")?;
        decode(arguments)
    }
}
