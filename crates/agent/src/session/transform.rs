use super::*;
impl Session {
    fn check_transform_ready(
        &self,
        app: &PlotiumApp,
        blocked: bool,
        reservation: u64,
    ) -> Result<(), Failure> {
        if !self.allow_modify {
            return Err(Failure::new(
                "permission_denied",
                "Switch to Read and modify before preparing a table operation",
            ));
        }
        if blocked {
            return Err(Failure::new(
                "busy",
                "Resolve recovery or save operation and retry",
            ));
        }
        ensure_not_busy(app)?;
        let reserved: u64 = self
            .transforms
            .values()
            .map(PreparedTableOperation::reserved_bytes)
            .sum();
        if self.pending_plan_count() >= 32
            || self.transforms.len() >= TABLE_TRANSFORM_PREPARED
            || reserved.saturating_add(reservation) > TABLE_TRANSFORM_MEMORY
        {
            return Err(Failure::new(
                "plan_limit",
                "Execute pending plans first; table operations retain at most four candidates and reserve at most 512 MiB",
            ));
        }
        Ok(())
    }
    pub(crate) fn prepare_transform_plan(
        &self,
        app: &PlotiumApp,
        mut arguments: Value,
        blocked: bool,
    ) -> Result<(ToolPlan, TableOperationSnapshot), Failure> {
        check_context(app, &mut arguments)?;
        // Permission precedes potentially expensive IR decoding.
        self.check_transform_ready(app, blocked, 0)?;
        let args = Plan::parse(arguments)?;
        if !is_table_operation(&args.tool_id)
            || (args.tool_id == "data.transform"
                && args.parameters.get("mode").and_then(Value::as_str) != Some("table_data_only"))
        {
            return Err(Failure::new(
                "unsupported_operation",
                "Use data.transform mode table_data_only, table.patch or table.refresh",
            ));
        }
        let reservation = if args.tool_id == "data.transform" {
            parse_table_transform(args.parameters.clone())?.memory_limit_bytes
        } else {
            0
        };
        self.check_transform_ready(app, blocked, reservation)?;
        if args.targets.is_some() || args.ids.as_ref().is_none_or(|ids| ids.len() != 1) {
            return Err(Failure::new(
                "invalid_parameters",
                "Transform requires exactly one DatasetId in ids, without targets",
            ));
        }
        let request = request(
            &args.tool_id,
            args.parameters,
            args.ids.expect("validated ids"),
            args.expected_revision,
        );
        let snapshot = TableOperationSnapshot::capture(app, &request)?;
        if let TableOperationSnapshot::Update(update) = &snapshot {
            self.check_transform_ready(app, blocked, update.reserved_bytes())?;
        }
        Ok((plan_tool(app, request)?, snapshot))
    }
    pub(crate) fn finish_transform_plan(
        &mut self,
        app: &PlotiumApp,
        plan: ToolPlan,
        prepared: PreparedTableOperation,
        blocked: bool,
    ) -> Result<Value, Failure> {
        self.check_transform_ready(app, blocked, prepared.reserved_bytes())?;
        let mut plan = plan_tool(app, plan.request)?;
        let update = matches!(prepared, PreparedTableOperation::Update(_));
        plan.estimated_changes = vec![
            if update {
                "Update one existing table; preserve all plot identities and authored display."
            } else {
                "Create one native derived table; no canvas or plot is created."
            }
            .into(),
        ];
        let id = format!("{}:{}", app.session.project_context, uuid::Uuid::new_v4());
        let mut value = json!({"plan_id":id,"preflight":plan});
        value[if update { "update" } else { "transform" }] = prepared.preflight()?;
        if serde_json::to_vec(&value)
            .map_err(|e| Failure::new("execution_failed", e.to_string()))?
            .len()
            > TABLE_RESPONSE_BYTES
        {
            return Err(Failure::new(
                "response_too_large",
                "Transform preflight exceeds 256 KiB; use a smaller plan/schema",
            ));
        }
        self.transforms.insert(id.clone(), prepared);
        self.plans.insert(id, plan);
        Ok(value)
    }
    pub(crate) fn discard_transform_plan(&mut self, id: &str) {
        self.plans.remove(id);
        self.transforms.remove(id);
    }
}
