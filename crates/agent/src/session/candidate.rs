//! Candidate permissions and plan ownership remain in the existing desktop session.
use super::*;

pub(crate) struct CandidateTicket {
    params: PlanPreviewParams,
    context: plotium_core::state::ProjectContextId,
    permission_epoch: u64,
}
impl Session {
    fn candidate_plan<'a>(
        &'a self,
        app: &PlotiumApp,
        p: &PlanPreviewParams,
        blocked: bool,
    ) -> Result<&'a ToolPlan, Failure> {
        if blocked {
            return Err(Failure::new(
                "busy",
                "Resolve recovery or save operation and retry",
            ));
        }
        ensure_not_busy(app)?;
        if !self.allow_modify {
            return Err(Failure::new(
                "permission_denied",
                "Candidate previews require the original reversible-modify permission",
            ));
        }
        if let Some((context, _)) = p.plan_id.split_once(':')
            && context != app.session.project_context.to_string()
        {
            return Err(project_changed());
        }
        if p.expected_revision.0 != app.doc.automation_revision {
            return Err(AutomationError::StaleRevision {
                expected: p.expected_revision.0,
                actual: app.doc.automation_revision,
            }
            .into());
        }
        if self.saves.contains_key(&p.plan_id) || self.exports.contains_key(&p.plan_id) {
            return Err(Failure::new(
                "unsupported_operation",
                "Disk operations cannot be previewed",
            ));
        }
        let plan = self.plans.get(&p.plan_id).ok_or_else(|| {
            Failure::new(
                "expired_plan",
                "Plan unavailable or already submitted; inspect state and retained receipts before considering a new operation",
            )
        })?;
        if plan.frozen_targets.revision != p.expected_revision {
            return Err(AutomationError::StaleRevision {
                expected: plan.frozen_targets.revision.0,
                actual: p.expected_revision.0,
            }
            .into());
        }
        validate_plan_preview(app, plan, &p.image.target)?;
        Ok(plan)
    }
    pub(crate) fn prepare_candidate(
        &self,
        app: &mut PlotiumApp,
        mut args: Value,
        blocked: bool,
    ) -> Result<(ImagePreviewSnapshot, PreparedToolRun, CandidateTicket), Failure> {
        check_context(app, &mut args)?;
        let params: PlanPreviewParams = decode(args)?;
        let plan = self.candidate_plan(app, &params, blocked)?;
        let audit = plan_tool(
            app,
            request(
                IMAGE_PREVIEW,
                json!(params.image),
                vec![params.image.target.resource.id.clone()],
                params.expected_revision,
            ),
        )?;
        let run = PreparedToolRun::begin(app, &audit);
        match capture_plan_preview(app, plan, &params) {
            Ok(snapshot) => Ok((
                snapshot,
                run,
                CandidateTicket {
                    params,
                    context: app.session.project_context,
                    permission_epoch: self.permission_epoch,
                },
            )),
            Err(error) => {
                let failure = Failure::from(&error);
                run.finish(app, &Err(error))?;
                Err(failure)
            }
        }
    }
    pub(crate) fn check_candidate_delivery(
        &self,
        app: &PlotiumApp,
        ticket: &CandidateTicket,
        blocked: bool,
    ) -> Result<(), Failure> {
        if ticket.context != app.session.project_context {
            return Err(project_changed());
        }
        if ticket.permission_epoch != self.permission_epoch {
            return Err(Failure::new(
                "permission_denied",
                "Candidate permission changed during rendering; preview again",
            ));
        }
        self.candidate_plan(app, &ticket.params, blocked)
            .map(|_| ())
    }
}
