//! Scientific calls use core descriptors and the existing plan/execute gate.
use super::*;

impl Session {
    pub(super) fn scientific_tasks(
        &mut self,
        app: &mut PlotiumApp,
        name: &str,
        arguments: Value,
    ) -> Result<Value, Failure> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Query {
            task_id: TaskId,
        }
        if name == "plotium_tasks_list" {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Page {
                #[serde(default)]
                offset: usize,
                #[serde(default = "default_limit")]
                limit: usize,
            }
            fn default_limit() -> usize {
                100
            }
            let page: Page = decode(arguments)?;
            if page.limit == 0 || page.limit > 128 {
                return Err(Failure::new(
                    "invalid_parameters",
                    "Task limit must be 1..=128.",
                ));
            }
            return Ok(json!({"tasks":app.session.scientific_tasks.list(page.offset,page.limit)}));
        }
        let query: Query = decode(arguments)?;
        let receipt = if name == "plotium_tasks_cancel" {
            if !self.allow_modify {
                return Err(Failure::new(
                    "permission_denied",
                    "Task cancellation requires modify permission.",
                ));
            }
            app.cancel_scientific_task(&query.task_id)
        } else {
            app.session.scientific_tasks.get(&query.task_id).cloned()
        };
        receipt.map(|receipt| json!({"task":receipt})).ok_or_else(|| Failure::new("task_not_found", "Task receipt is unknown or evicted. Do not automatically resubmit the operation."))
    }
    pub(super) fn scientific_plan(
        &mut self,
        app: &PlotiumApp,
        args: Plan,
    ) -> Result<Value, Failure> {
        if !self.allow_modify {
            return Err(Failure::new(
                "permission_denied",
                "Switch to Read and modify before planning scientific edits.",
            ));
        }
        if self.pending_plan_count() >= 32 {
            return Err(Failure::new(
                "plan_limit",
                "Execute a pending plan before preparing another.",
            ));
        }
        let targets = selector(args.ids, args.targets)?;
        if !matches!(&targets,TargetSelector::Explicit {ids} if !ids.is_empty()) {
            return Err(Failure::new(
                "invalid_parameters",
                "Scientific edits require explicit dataset IDs.",
            ));
        }
        let mut req = request(
            &args.tool_id,
            args.parameters,
            vec![],
            args.expected_revision,
        );
        req.targets = targets;
        let plan = plan_tool(app, req)?;
        let id = format!("{}:{}", app.session.project_context, uuid::Uuid::new_v4());
        let value = json!({"plan_id":id,"preflight":plan});
        if serde_json::to_vec(&value)
            .map_err(|e| Failure::new("execution_failed", e.to_string()))?
            .len()
            > DOCUMENT_RESPONSE_BYTES
        {
            return Err(Failure::new(
                "response_too_large",
                "Scientific preflight exceeds the response budget.",
            ));
        }
        self.plans.insert(id, plan);
        Ok(value)
    }
    pub(super) fn scientific_read(
        &mut self,
        app: &mut PlotiumApp,
        name: &str,
        args: Value,
    ) -> Result<Value, Failure> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Query {
            expected_revision: DocumentRevision,
            ids: Vec<String>,
            input: Option<plotium_core::DiffusionInput>,
            method: Option<String>,
            offset: Option<usize>,
            limit: Option<usize>,
        }
        let query: Query = decode(args)?;
        let id = if name == "plotium_analysis_results_inspect" {
            ANALYSIS_RESULTS_INSPECT
        } else if name == "plotium_diffusion_inspect" {
            DIFFUSION_INSPECT
        } else {
            PROCESSING_INSPECT
        };
        if (id != DIFFUSION_INSPECT && query.input.is_some())
            || (id != ANALYSIS_RESULTS_INSPECT
                && (query.method.is_some() || query.offset.is_some() || query.limit.is_some()))
        {
            return Err(Failure::new(
                "invalid_parameters",
                "Parameters are not supported by this scientific read operation.",
            ));
        }
        let parameters = if id == ANALYSIS_RESULTS_INSPECT {
            json!({"method":query.method,"offset":query.offset.unwrap_or(0),"limit":query.limit.unwrap_or(100)})
        } else if id == DIFFUSION_INSPECT {
            query
                .input
                .map_or(json!({}), |input| json!({"input":input}))
        } else {
            json!({})
        };
        let plan = plan_tool(
            app,
            request(id, parameters, query.ids, query.expected_revision),
        )?;
        let result = json!(execute_tool(app, plan, ExecutionAuthority::Read)?);
        if serde_json::to_vec(&result)
            .map_err(|e| Failure::new("execution_failed", e.to_string()))?
            .len()
            > DOCUMENT_RESPONSE_BYTES
        {
            return Err(Failure::new(
                "response_too_large",
                "Reduce limit to read scientific results within the response budget.",
            ));
        }
        Ok(result)
    }
}
