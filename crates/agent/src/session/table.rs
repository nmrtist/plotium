//! Import planning retains frozen native data, not paths to re-read at execute.
use super::*;
impl Session {
    pub(crate) fn remaining_import_bytes(&self) -> usize {
        NATIVE_IMPORT_BYTES.saturating_sub(
            self.imports
                .values()
                .map(PreparedDataImport::reserved_bytes)
                .sum(),
        )
    }
    pub(crate) fn prepare_import_plan(
        &self,
        app: &PlotiumApp,
        mut arguments: Value,
        blocked: bool,
    ) -> Result<(ToolPlan, DataImportParams), Failure> {
        check_context(app, &mut arguments)?;
        self.check_import_ready(app, blocked)?;
        let args = Plan::parse(arguments)?;
        if args.tool_id != "data.import" || args.ids.is_some() || args.targets.is_some() {
            return Err(Failure::new(
                "invalid_parameters",
                "Import takes explicit input paths, without ids or targets",
            ));
        }
        let params = DataImportParams::parse(args.parameters.clone())?;
        let plan = plan_tool(
            app,
            request(
                "data.import",
                args.parameters,
                vec![],
                args.expected_revision,
            ),
        )?;
        Ok((plan, params))
    }
    fn check_import_ready(&self, app: &PlotiumApp, blocked: bool) -> Result<(), Failure> {
        if !self.allow_modify {
            return Err(Failure::new(
                "permission_denied",
                "Switch the connection to Read and modify before importing",
            ));
        }
        if blocked {
            return Err(Failure::new(
                "busy",
                "Resolve recovery or save operation and retry",
            ));
        }
        ensure_not_busy(app)?;
        if self.pending_plan_count() >= 32 || self.imports.len() >= 4 {
            return Err(Failure::new(
                "plan_limit",
                "Execute a pending plan or restart the connection; at most four prepared imports are retained",
            ));
        }
        Ok(())
    }
    pub(crate) fn finish_import_plan(
        &mut self,
        app: &PlotiumApp,
        plan: ToolPlan,
        prepared: impl Into<PreparedDataImport>,
        blocked: bool,
    ) -> Result<Value, Failure> {
        self.check_import_ready(app, blocked)?;
        let prepared = prepared.into();
        if self
            .imports
            .values()
            .map(PreparedDataImport::reserved_bytes)
            .sum::<usize>()
            .saturating_add(prepared.reserved_bytes())
            > NATIVE_IMPORT_BYTES
        {
            return Err(Failure::new(
                "import_budget_exceeded",
                "Pending native imports exceed 512 MiB; execute or discard a pending import first",
            ));
        }
        // Revalidate after background preparation against the actual live document.
        let mut plan = plan_tool(app, plan.request)?;
        let summary = prepared.preflight();
        plan.estimated_changes = vec![format!(
            "Import {} inputs; {} failed inputs.",
            summary["prepared_files"], summary["failed_files"]
        )];
        let id = format!("{}:{}", app.session.project_context, uuid::Uuid::new_v4());
        let value = json!({"plan_id":id,"preflight":plan,"import":summary});
        if serde_json::to_vec(&value)
            .map_err(|e| Failure::new("execution_failed", e.to_string()))?
            .len()
            > TABLE_RESPONSE_BYTES
        {
            return Err(Failure::new(
                "response_too_large",
                "Import preflight exceeds 256 KiB; use a smaller batch/schema",
            ));
        }
        self.imports.insert(id.clone(), prepared);
        self.plans.insert(id, plan);
        Ok(value)
    }
    pub(crate) fn discard_import_plan(&mut self, id: &str) {
        self.plans.remove(id);
        self.imports.remove(id);
    }
    pub(super) fn table_read(
        &self,
        app: &mut PlotiumApp,
        name: &str,
        mut arguments: Value,
    ) -> Result<Value, Failure> {
        check_context(app, &mut arguments)?;
        let revision: DocumentRevision = decode(
            arguments
                .as_object_mut()
                .and_then(|args| args.remove("expected_revision"))
                .ok_or_else(|| {
                    Failure::new("invalid_parameters", "expected_revision is required")
                })?,
        )?;
        let tool = if name == "plotium_table_columns" {
            TABLE_COLUMNS
        } else {
            TABLE_READ_ROWS
        };
        let id = arguments
            .get("dataset_id")
            .and_then(Value::as_str)
            .ok_or_else(|| Failure::new("invalid_parameters", "dataset_id is required"))?
            .to_owned();
        let plan = plan_tool(app, request(tool, arguments, vec![id], revision))?;
        Ok(json!(execute_tool(app, plan, ExecutionAuthority::Read)?))
    }
}
