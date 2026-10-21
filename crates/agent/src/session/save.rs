//! Save planning schedules the real desktop host; it never serializes a project.
use super::*;
use plotium_core::project::SaveTarget;
use std::path::PathBuf;

pub(super) struct SavePlan {
    context: plotium_core::state::ProjectContextId,
    revision: u64,
    target: SaveTarget,
    snapshots: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Parameters {
    path: Option<PathBuf>,
    include_view_snapshots: Option<bool>,
}
pub(crate) fn schema() -> Value {
    json!({"type":"object","additionalProperties":false,"properties":{
        "path":{"type":"string","description":"Absolute .plotium path for first save or Save As; omit to use the current file. Parent directory must exist; symlink targets are rejected."},
        "include_view_snapshots":{"type":"boolean","description":"Optional native rendered-view snapshots; defaults to the current project's save profile."}
    }})
}
impl Session {
    pub(super) fn plan_save(&mut self, app: &PlotiumApp, args: Plan) -> Result<Value, Failure> {
        if args.ids.is_some() || args.targets.is_some() {
            return Err(Failure::new(
                "invalid_parameters",
                "project.save has no resource selectors",
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
        let params: Parameters = decode(args.parameters)?;
        let path = params
            .path
            .or_else(|| app.doc.project_path.clone())
            .ok_or_else(|| {
                Failure::new(
                    "invalid_parameters",
                    "First save requires an absolute .plotium path",
                )
            })?;
        let target = SaveTarget::inspect(&path)
            .map_err(|e| Failure::new("invalid_parameters", e.to_string()))?;
        let snapshots = params
            .include_view_snapshots
            .unwrap_or(app.doc.save_include_view_snapshots);
        let id = format!("{}:{}", app.session.project_context, uuid::Uuid::new_v4());
        let value = json!({"plan_id":id,"preflight":{"tool_id":"project.save","path":target.path,"overwrite_required":target.exists(),"expected_revision":args.expected_revision,"include_view_snapshots":snapshots,"permission":"per_request_desktop_save_authorization","undoable":false},"save_id":id});
        self.saves.insert(
            id,
            SavePlan {
                context: app.session.project_context,
                revision: args.expected_revision.0,
                target,
                snapshots,
            },
        );
        Ok(value)
    }
    pub(super) fn execute_save(
        &mut self,
        app: &mut PlotiumApp,
        id: &str,
        plan: SavePlan,
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
        ensure_not_busy(app)?;
        if plan.revision != app.doc.automation_revision {
            return Err(Failure::new(
                "stale_revision",
                "Document revision changed; reread state and replan",
            ));
        }
        plan.target
            .verify()
            .map_err(|e| Failure::new("save_target_changed", e.to_string()))?;
        let permit = app
            .request_external_save(id.into(), plan.target, plan.revision, plan.snapshots)
            .map_err(|e| Failure::new("busy", e))?;
        self.save_permits.retain(|p| !p.is_revoked());
        self.save_permits.push(permit);
        Ok(json!({"save":app.session.external_saves.receipt(id)}))
    }
    pub(super) fn save_status(
        &self,
        app: &mut PlotiumApp,
        arguments: Value,
    ) -> Result<Value, Failure> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Args {
            save_id: String,
        }
        let args: Args = decode(arguments)?;
        app.session.external_saves.reconcile();
        let receipt = app.session.external_saves.receipt(&args.save_id).ok_or_else(|| Failure::new("unknown_save", "No retained receipt. Inspect the target and current project; never replay an unknown submitted save."))?;
        Ok(
            json!({"save":receipt,"current_project_context":app.session.project_context,"current_revision":app.doc.automation_revision,"current_dirty":app.doc.dirty,"current_path":app.doc.project_path,"matches_current_project":receipt.project_context == app.session.project_context}),
        )
    }
    pub(crate) fn revoke_saves(&mut self) {
        for permission in self.save_permits.drain(..) {
            permission.revoke();
        }
        self.saves.clear();
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.revoke_saves();
        self.revoke_exports();
    }
}
