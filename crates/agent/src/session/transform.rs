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
                "Switch to Read and modify before preparing a transform",
            ));
        }
        if blocked {
            return Err(Failure::new(
                "busy",
                "Resolve recovery or save operation and retry",
            ));
        }
        ensure_idle(app)?;
        let reserved: u64 = self
            .transforms
            .values()
            .map(PreparedTableTransform::reserved_bytes)
            .sum();
        if self.plans.len() >= 32
            || self.transforms.len() >= TABLE_TRANSFORM_PREPARED
            || reserved.saturating_add(reservation) > TABLE_TRANSFORM_MEMORY
        {
            return Err(Failure::new(
                "plan_limit",
                "Execute pending plans first; transforms retain at most four candidates and reserve at most 512 MiB",
            ));
        }
        Ok(())
    }
    pub(crate) fn prepare_transform_plan(
        &self,
        app: &PlotiumApp,
        mut arguments: Value,
        blocked: bool,
    ) -> Result<(ToolPlan, TableTransformSnapshot), Failure> {
        check_context(app, &mut arguments)?;
        // Permission precedes potentially expensive IR decoding.
        self.check_transform_ready(app, blocked, 0)?;
        let args: Plan = decode(arguments)?;
        if args.tool_id != "data.transform"
            || args.parameters.get("mode").and_then(Value::as_str) != Some("table_data_only")
        {
            return Err(Failure::new(
                "unsupported_operation",
                "Only data.transform mode table_data_only is exposed",
            ));
        }
        let params = parse_table_transform(args.parameters.clone())?;
        self.check_transform_ready(app, blocked, params.memory_limit_bytes)?;
        if args.targets.is_some() || args.ids.as_ref().is_none_or(|ids| ids.len() != 1) {
            return Err(Failure::new(
                "invalid_parameters",
                "Transform requires exactly one DatasetId in ids, without targets",
            ));
        }
        let request = request(
            "data.transform",
            args.parameters,
            args.ids.expect("validated ids"),
            args.expected_revision,
        );
        let snapshot = TableTransformSnapshot::capture(app, &request)?;
        Ok((plan_tool(app, request)?, snapshot))
    }
    pub(crate) fn finish_transform_plan(
        &mut self,
        app: &PlotiumApp,
        plan: ToolPlan,
        prepared: PreparedTableTransform,
        blocked: bool,
    ) -> Result<Value, Failure> {
        self.check_transform_ready(app, blocked, prepared.reserved_bytes())?;
        let mut plan = plan_tool(app, plan.request)?;
        plan.estimated_changes =
            vec!["Create one native derived table; no canvas or plot is created.".into()];
        let id = format!("{}:{}", app.session.project_context, uuid::Uuid::new_v4());
        let value = json!({"plan_id":id,"preflight":plan,"transform":prepared.preflight()?});
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
