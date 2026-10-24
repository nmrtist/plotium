//! One bounded, revision-bound diagnostic scan per connection. Cursors are
//! one-use transport handles, not project identities or persisted artifacts.
use crate::{Connection, Failure, PendingRequest, Session};
use plotium_core::{
    automation::*,
    state::{PlotiumApp, ProjectContextId},
};
use serde_json::{Value, json};
use std::{thread::JoinHandle, time::Instant};
use tokio_util::sync::CancellationToken;

pub(crate) struct RetainedScan {
    cursor: String,
    touched: Instant,
    transport: Option<rmcp::Peer<rmcp::RoleServer>>,
    scan: FigureDiagnosticScan,
}
pub(crate) struct DiagnosticFlight {
    worker: JoinHandle<(FigureDiagnosticScan, Result<Value, AutomationError>)>,
    cancellation: CancellationToken,
    request: PendingRequest,
    run: PreparedToolRun,
    context: ProjectContextId,
    revision: DocumentRevision,
}
impl DiagnosticFlight {
    pub(crate) fn cancel(self, code: &'static str) {
        self.cancellation.cancel();
        respond_disconnected(
            self.request,
            Failure::new(
                code,
                "Diagnostic invalidated; reconnect/read state and start a new scan",
            ),
        );
    }
}
fn respond_disconnected(request: PendingRequest, error: Failure) {
    if request.response.send(Err(error)).is_err() { /* Already disconnected. */ }
}
impl Session {
    fn prepare_diagnostic(
        &mut self,
        app: &mut PlotiumApp,
        mut args: Value,
        blocked: bool,
    ) -> Result<(FigureDiagnosticScan, PreparedToolRun), Failure> {
        crate::session::check_context(app, &mut args)?;
        crate::session::short_refs::expand(app, &mut args)?;
        barrier(app, blocked)?;
        let revision: DocumentRevision = serde_json::from_value(
            args.as_object_mut()
                .and_then(|a| a.remove("expected_revision"))
                .ok_or_else(|| {
                    Failure::new("invalid_parameters", "expected_revision is required")
                })?,
        )
        .map_err(|e| Failure::new("invalid_parameters", e.to_string()))?;
        if revision.0 != app.doc.automation_revision {
            self.expire_diagnostic(app);
            return Err(AutomationError::StaleRevision {
                expected: revision.0,
                actual: app.doc.automation_revision,
            }
            .into());
        }
        let scan = if let Some(cursor) = args.get("cursor") {
            if args.as_object().is_none_or(|a| a.len() != 1) {
                return Err(Failure::new(
                    "invalid_parameters",
                    "Continuation accepts only cursor, expected_project_context and expected_revision",
                ));
            }
            let cursor = cursor
                .as_str()
                .ok_or_else(|| Failure::new("invalid_parameters", "cursor must be a string"))?;
            let stored = self.diagnostic.as_ref().ok_or_else(expired)?;
            if stored.cursor != cursor {
                return Err(expired());
            }
            let stored = self.diagnostic.take().expect("checked above");
            if stored.touched.elapsed().as_secs() >= DIAGNOSTIC_TTL_SECS
                || stored
                    .transport
                    .as_ref()
                    .is_some_and(|p| p.is_transport_closed())
            {
                return Err(expired());
            }
            if stored.scan.project_context != app.session.project_context {
                return Err(Failure::new(
                    "project_changed",
                    "Read state and restart diagnostics",
                ));
            }
            if stored.scan.revision != revision {
                return Err(AutomationError::StaleRevision {
                    expected: stored.scan.revision.0,
                    actual: revision.0,
                }
                .into());
            }
            stored.scan
        } else {
            let params: FigureDiagnosticParams = serde_json::from_value(args)
                .map_err(|e| Failure::new("invalid_parameters", e.to_string()))?;
            // A successful new initial query replaces the previous cursor.
            let scan = FigureDiagnosticScan::capture(app, revision, params)?;
            self.diagnostic = None;
            scan
        };
        let plan = plan_tool(
            app,
            ToolRequest {
                tool_id: FIGURE_DIAGNOSE.into(),
                tool_version: 1,
                parameters: scan.audit_parameters(),
                targets: TargetSelector::Explicit {
                    ids: vec![scan.canvas_target().resource.id],
                },
                expected_revision: revision,
                caller: CallerType::Agent,
            },
        )?;
        Ok((scan, PreparedToolRun::begin(app, &plan)))
    }
    fn finish_diagnostic(
        &mut self,
        app: &mut PlotiumApp,
        mut scan: FigureDiagnosticScan,
        run: PreparedToolRun,
        mut outcome: Result<Value, AutomationError>,
        blocked: bool,
    ) -> Result<Value, Failure> {
        if scan.project_context != app.session.project_context {
            return Err(Failure::new(
                "project_changed",
                "Read state and restart diagnostics",
            ));
        }
        if scan.revision.0 != app.doc.automation_revision {
            outcome = Err(AutomationError::StaleRevision {
                expected: scan.revision.0,
                actual: app.doc.automation_revision,
            });
        }
        if outcome.is_ok() {
            barrier(app, blocked)?;
        }
        let mut cursor = None;
        if let Ok(value) = &mut outcome {
            if !scan.is_complete() {
                cursor = Some(uuid::Uuid::new_v4().to_string());
            }
            value["continuation"] = json!(cursor);
            value["continuation_available"] = json!(cursor.is_some());
        }
        let outcome = outcome.map(|value| ToolResult {
            tool_id: FIGURE_DIAGNOSE.into(),
            before_revision: scan.revision,
            after_revision: scan.revision,
            targets: vec![],
            produced: vec![],
            modified: vec![],
            diagnostics: vec![],
            verification: vec![],
            value,
        });
        run.finish(app, &outcome)?;
        let mut result = outcome?;
        if scan.audit_parameters()["detail"] != "full" {
            crate::session::diagnostic_page(&mut result.value);
        }
        if let Some(cursor) = cursor {
            // Release completed rows and retain only the unread immutable tail.
            scan.release_completed_storage();
            self.diagnostic = Some(RetainedScan {
                cursor,
                touched: Instant::now(),
                transport: None,
                scan,
            });
        }
        Ok(json!(result))
    }
    pub(crate) fn call_diagnostic(
        &mut self,
        app: &mut PlotiumApp,
        args: Value,
        blocked: bool,
    ) -> Result<Value, Failure> {
        let (mut scan, run) = self.prepare_diagnostic(app, args, blocked)?;
        let outcome = scan.next_page(|| false);
        self.finish_diagnostic(app, scan, run, outcome, blocked)
    }
    pub(crate) fn expire_diagnostic(&mut self, app: &PlotiumApp) {
        if self.diagnostic.as_ref().is_some_and(|s| {
            s.transport
                .as_ref()
                .is_some_and(|p| p.is_transport_closed())
                || s.touched.elapsed().as_secs() >= DIAGNOSTIC_TTL_SECS
                || s.scan.revision.0 != app.doc.automation_revision
                || s.scan.project_context != app.session.project_context
        }) {
            self.diagnostic = None;
        }
    }
}
fn barrier(app: &PlotiumApp, blocked: bool) -> Result<(), Failure> {
    if blocked {
        return Err(Failure::new(
            "busy",
            "Resolve recovery or save operation and retry",
        ));
    }
    crate::session::ensure_project_ready(app)
}
fn expired() -> Failure {
    Failure::new(
        "expired_cursor",
        "Cursor expired, was consumed or replaced. Read state and start a new scan; never combine pages from different scans.",
    )
}
impl Connection {
    pub(crate) fn dispatch_diagnostic(
        &mut self,
        app: &mut PlotiumApp,
        request: PendingRequest,
        blocked: bool,
    ) -> Result<(), Failure> {
        if !self.is_running() {
            return respond(
                request,
                Err(Failure::new("connection_stopped", "Connection stopped")),
            );
        }
        if self.diagnostic.is_some() {
            return respond(
                request,
                Err(Failure::new("busy", "A diagnostic batch is still running")),
            );
        }
        let (mut scan, run) =
            match self
                .session
                .prepare_diagnostic(app, request.arguments.clone(), blocked)
            {
                Ok(v) => v,
                Err(e) => return respond(request, Err(e)),
            };
        let cancellation = request.cancellation.child_token();
        let cancel = cancellation.clone();
        let active = self.active.clone();
        let context = scan.project_context;
        let revision = scan.revision;
        match std::thread::Builder::new()
            .name("plotium-diagnostics".into())
            .spawn(move || {
                let result = scan.next_page(|| {
                    cancel.is_cancelled() || !active.load(std::sync::atomic::Ordering::Acquire)
                });
                (scan, result)
            }) {
            Ok(worker) => {
                self.diagnostic = Some(DiagnosticFlight {
                    worker,
                    cancellation,
                    request,
                    run,
                    context,
                    revision,
                });
                Ok(())
            }
            Err(e) => {
                let e =
                    AutomationError::Execution(format!("Could not start diagnostic worker: {e}"));
                let failure = Failure::from(&e);
                run.finish(app, &Err(e))?;
                respond(request, Err(failure))
            }
        }
    }
    pub fn poll_diagnostic(&mut self, app: &mut PlotiumApp, blocked: bool) -> Result<(), Failure> {
        self.synchronize_project(app);
        self.session.expire_diagnostic(app);
        let Some(flight) = &self.diagnostic else {
            return Ok(());
        };
        if flight.request.is_cancelled()
            || flight
                .request
                .transport
                .as_ref()
                .is_some_and(|p| p.is_transport_closed())
            || flight.revision.0 != app.doc.automation_revision
        {
            flight.cancellation.cancel();
        }
        if !flight.worker.is_finished() {
            return Ok(());
        }
        let flight = self.diagnostic.take().expect("checked above");
        if !self.is_running() {
            return respond(
                flight.request,
                Err(Failure::new("connection_stopped", "Connection stopped")),
            );
        }
        if flight.context != app.session.project_context {
            return respond(
                flight.request,
                Err(Failure::new(
                    "project_changed",
                    "Read state and restart diagnostics",
                )),
            );
        }
        let result = match flight.worker.join() {
            Ok((scan, mut outcome)) => {
                if flight.request.is_cancelled()
                    || flight
                        .request
                        .transport
                        .as_ref()
                        .is_some_and(|p| p.is_transport_closed())
                {
                    outcome = Err(AutomationError::DiagnosticCancelled);
                }
                self.session
                    .finish_diagnostic(app, scan, flight.run, outcome, blocked)
            }
            Err(_) => {
                let error = AutomationError::Execution(
                    "Diagnostic worker stopped; read state and retry".into(),
                );
                let failure = Failure::from(&error);
                flight.run.finish(app, &Err(error))?;
                Err(failure)
            }
        };
        if let Some(stored) = &mut self.session.diagnostic {
            stored.transport = flight.request.transport.clone();
        }
        let status = result.as_ref().map(|_| ()).map_err(Clone::clone);
        if flight.request.response.send(result).is_err() {
            self.session.diagnostic = None;
        }
        status
    }
}
fn respond(request: PendingRequest, result: Result<Value, Failure>) -> Result<(), Failure> {
    let status = result.as_ref().map(|_| ()).map_err(Clone::clone);
    if request.response.send(result).is_err() { /* Disconnection discards the result; no content mutation was committed. */
    }
    status
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    #[test]
    fn cursor_expiration_and_initial_query_replacement_release_the_scan() {
        let (mut app, mut session, _) = crate::guide_fixture::fixture();
        let q = json!({"expected_project_context":app.session.project_context,"expected_revision":app.doc.automation_revision,"canvas":TargetRef::resource(app.doc.canvases[0].resource_id.into()),"output":{"size_mm":app.doc.canvases[0].size_mm},"limit":1});
        let first = session
            .call(&mut app, "plotium_figure_diagnose", q.clone())
            .unwrap();
        let cursor = json!({"expected_project_context":app.session.project_context,"expected_revision":app.doc.automation_revision,"cursor":first["value"]["continuation"]});
        session.diagnostic.as_mut().unwrap().touched =
            Instant::now() - std::time::Duration::from_secs(DIAGNOSTIC_TTL_SECS + 1);
        session.expire_diagnostic(&app);
        assert!(session.diagnostic.is_none());
        assert_eq!(
            session
                .call(&mut app, "plotium_figure_diagnose", cursor.clone())
                .unwrap_err()
                .code,
            "expired_cursor"
        );
        let first = session
            .call(&mut app, "plotium_figure_diagnose", q.clone())
            .unwrap();
        let mut old = cursor;
        old["cursor"] = first["value"]["continuation"].clone();
        session
            .call(&mut app, "plotium_figure_diagnose", q)
            .unwrap();
        assert_eq!(
            session
                .call(&mut app, "plotium_figure_diagnose", old)
                .unwrap_err()
                .code,
            "expired_cursor"
        );
        assert!(session.diagnostic.is_some());
    }
}
