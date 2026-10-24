//! `properties.set_many` within this connection's display-property subset.
use super::*;
use plotium_core::properties::guide;

impl Session {
    pub(super) fn plan_property_batch(
        &mut self,
        app: &PlotiumApp,
        args: Plan,
    ) -> Result<Value, Failure> {
        if args.ids.is_some() || args.targets.is_some() {
            return Err(Failure::new(
                "invalid_parameters",
                "properties.set_many names exact targets inside writes; omit ids/targets",
            ));
        }
        for (index, write) in args
            .parameters
            .get("writes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            if check_property(write).is_err() {
                let mut failure = Failure::new(
                    "unsupported_property",
                    format!(
                        "writes[{index}]: property is outside the connection's supported display properties"
                    ),
                );
                failure.details = Some(
                    json!({"write_index":index,"key":write.get("key"),"field":format!("parameters.writes[{index}].key")}),
                );
                return Err(failure);
            }
            // The single-key guide request enforces the same bound.
            if write.get("key").and_then(Value::as_str) == Some(guide::TITLE.as_str())
                && write
                    .get("value")
                    .and_then(Value::as_str)
                    .is_some_and(|s| s.len() > 16384)
            {
                return Err(Failure::new(
                    "invalid_parameters",
                    format!("writes[{index}]: guide title exceeds 16384 UTF-8 bytes"),
                ));
            }
        }
        if self.pending_plan_count() >= 32 {
            return Err(Failure::new(
                "plan_limit",
                "32 plans are pending; execute a plan or restart the connection",
            ));
        }
        let plan = plan_tool(
            app,
            request(
                TOOL_SET_MANY,
                args.parameters,
                vec![],
                args.expected_revision,
            ),
        )?;
        let id = format!("{}:{}", app.session.project_context, uuid::Uuid::new_v4());
        let result = compact::plan_response(&id, &plan);
        if serde_json::to_vec(&result)
            .map_err(|e| Failure::new("execution_failed", e.to_string()))?
            .len()
            > DOCUMENT_RESPONSE_BYTES
        {
            return Err(Failure::new(
                "response_too_large",
                "Property batch preflight exceeds 256 KiB; split the writes",
            ));
        }
        self.plans.insert(id, plan);
        Ok(result)
    }
}
