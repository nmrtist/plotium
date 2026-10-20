//! Adapter to desktop-owned native export consent and receipts, not a file writer.
use super::*;
use plotium_core::export::{FigureExportParams, PreparedFigureExport, prepare_figure_export};
pub(super) struct ExportPlan {
    context: plotium_core::state::ProjectContextId,
    revision: u64,
    prepared: PreparedFigureExport,
}
impl Session {
    pub(super) fn plan_export(&mut self, app: &PlotiumApp, args: Plan) -> Result<Value, Failure> {
        if args.ids.is_some() || args.targets.is_some() {
            return Err(Failure::new(
                "invalid_parameters",
                "figure.export has exact canvas inside parameters; omit ids/targets",
            ));
        }
        if args.expected_revision.0 != app.doc.automation_revision {
            return Err(Failure::new(
                "stale_revision",
                "Document revision changed; reread state",
            ));
        }
        if self.pending_plan_count() >= 32 {
            return Err(Failure::new("plan_limit", "Execute a pending plan first"));
        }
        let parameters: FigureExportParams = decode(args.parameters)?;
        let prepared = prepare_figure_export(app, parameters)
            .map_err(|e| Failure::new("invalid_parameters", e))?;
        let id = format!("{}:{}", app.session.project_context, uuid::Uuid::new_v4());
        let value = json!({"plan_id":id,"export_id":id,"preflight":prepared.preflight()});
        self.exports.insert(
            id,
            ExportPlan {
                context: app.session.project_context,
                revision: args.expected_revision.0,
                prepared,
            },
        );
        Ok(value)
    }
    pub(super) fn execute_export(
        &mut self,
        app: &mut PlotiumApp,
        id: &str,
        plan: ExportPlan,
        blocked: bool,
    ) -> Result<Value, Failure> {
        if plan.context != app.session.project_context {
            return Err(project_changed());
        }
        if blocked {
            return Err(Failure::new(
                "busy",
                "Wait for the current desktop operation",
            ));
        }
        ensure_idle(app)?;
        if plan.revision != app.doc.automation_revision {
            return Err(Failure::new(
                "stale_revision",
                "Document revision changed; rediscover and replan",
            ));
        }
        let permit = app
            .request_external_export(id.into(), plan.prepared, plan.revision)
            .map_err(|e| Failure::new("busy", e))?;
        self.export_permits.retain(|p| !p.is_revoked());
        self.export_permits.push(permit);
        Ok(json!({"export":app.session.external_exports.receipt(id)}))
    }
    pub(super) fn export_status(
        &self,
        app: &mut PlotiumApp,
        args: Value,
    ) -> Result<Value, Failure> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Args {
            export_id: String,
        }
        let args: Args = decode(args)?;
        app.poll_external_export();
        let receipt=app.session.external_exports.receipt(&args.export_id).ok_or_else(||Failure::new("unknown_export","No retained receipt. Inspect exact output paths; never replay an unknown submitted export."))?;
        Ok(
            json!({"export":receipt,"current_project_context":app.session.project_context,"current_revision":app.doc.automation_revision,"matches_current_project":receipt.project_context==app.session.project_context}),
        )
    }
    pub(crate) fn revoke_exports(&mut self) {
        for p in self.export_permits.drain(..) {
            p.revoke();
        }
        self.exports.clear();
    }
    /// A change to access invalidates pending external consent, even though export
    /// itself can be requested in either access mode with fresh desktop consent.
    pub fn set_allow_modify(&mut self, allow: bool) {
        if self.allow_modify != allow {
            self.permission_epoch = self.permission_epoch.wrapping_add(1);
            self.revoke_exports();
            self.scientific_permission.cancel();
            if allow {
                self.scientific_permission = TaskCancellation::default();
            }
        }
        self.allow_modify = allow;
    }
}
