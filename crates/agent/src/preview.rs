//! The existing desktop bridge owns capture/completion; workers own snapshots only.
use crate::{Connection, Failure, PendingRequest};
use plotium_core::{automation::*, state::PlotiumApp};
use std::thread::JoinHandle;
use tokio_util::sync::CancellationToken;

pub(crate) struct PreviewFlight {
    pub(super) worker: JoinHandle<Result<ToolResult, AutomationError>>,
    pub(super) run: PreparedToolRun,
    pub(super) request: PendingRequest,
    pub(super) cancellation: CancellationToken,
    pub(super) candidate: Option<crate::session::CandidateTicket>,
}
impl Connection {
    /// Dispatch all requests through one desktop entry point. Existing tools
    /// keep their synchronous contract; image work does not block GUI frames.
    pub fn dispatch_request(
        &mut self,
        app: &mut PlotiumApp,
        request: PendingRequest,
        blocked: bool,
    ) -> Result<(), Failure> {
        self.synchronize_project(app);
        if request.is_cancelled() {
            return Ok(());
        }
        if request.name == "plotium_plan"
            && request
                .arguments
                .get("tool_id")
                .and_then(serde_json::Value::as_str)
                == Some("data.import")
        {
            return self.dispatch_import(app, request, blocked);
        }
        if request.name == "plotium_plan"
            && request
                .arguments
                .get("tool_id")
                .and_then(serde_json::Value::as_str)
                .is_some_and(plotium_core::automation::is_table_operation)
        {
            return self.dispatch_transform(app, request, blocked);
        }
        if request.name == "plotium_figure_diagnose" {
            return self.dispatch_diagnostic(app, request, blocked);
        }
        if !matches!(
            request.name.as_str(),
            "plotium_render_preview" | "plotium_preview_plan"
        ) {
            let result = self.session.call_when_available(
                app,
                &request.name,
                request.arguments.clone(),
                blocked,
            );
            return respond(request, result);
        }
        if self.preview.is_some() || self.retiring_preview.is_some() {
            return respond(
                request,
                Err(Failure::new(
                    "busy",
                    "A preview is still rendering; retry after it finishes",
                )),
            );
        }
        let prepared = if request.name == "plotium_preview_plan" {
            self.session
                .prepare_candidate(app, request.arguments.clone(), blocked)
                .map(|(s, r, t)| (s, r, Some(t)))
        } else {
            self.session
                .prepare_preview(app, request.arguments.clone(), blocked)
                .map(|(s, r)| (s, r, None))
        };
        match prepared {
            Err(error) => respond(request, Err(error)),
            Ok((snapshot, run, candidate)) => {
                // A project switch cancels this child without cancelling the
                // HTTP request that must carry its project_changed response.
                let cancellation = request.cancellation.child_token();
                let worker_cancellation = cancellation.clone();
                let active = self.active.clone();
                let worker = std::thread::Builder::new()
                    .name("plotium-preview".into())
                    .spawn(move || {
                        if worker_cancellation.is_cancelled()
                            || !active.load(std::sync::atomic::Ordering::Acquire)
                        {
                            return Err(AutomationError::PreviewCancelled);
                        }
                        snapshot.render_cancellable(|| {
                            worker_cancellation.is_cancelled()
                                || !active.load(std::sync::atomic::Ordering::Acquire)
                        })
                    });
                match worker {
                    Ok(worker) => {
                        self.preview = Some(PreviewFlight {
                            worker,
                            run,
                            request,
                            cancellation,
                            candidate,
                        });
                        Ok(())
                    }
                    Err(error) => {
                        let error = AutomationError::Execution(format!(
                            "Could not start preview worker: {error}"
                        ));
                        let failure = Failure::from(&error);
                        let failure = match run.finish(app, &Err(error)) {
                            Ok(()) => failure,
                            Err(error) => error.into(),
                        };
                        respond(request, Err(failure))
                    }
                }
            }
        }
    }
    pub fn poll_preview(&mut self, app: &mut PlotiumApp) -> Result<(), Failure> {
        self.poll_preview_when_available(app, false)
    }
    pub fn poll_preview_when_available(
        &mut self,
        app: &mut PlotiumApp,
        blocked: bool,
    ) -> Result<(), Failure> {
        self.synchronize_project(app);
        if self
            .retiring_preview
            .as_ref()
            .is_some_and(|w| w.is_finished())
        {
            // Invalidated work has no consumer; retain its slot until the bounded renderer exits.
            if let Some(worker) = self.retiring_preview.take() {
                let _outcome = worker.join();
            }
        }
        if !self
            .preview
            .as_ref()
            .is_some_and(|p| p.worker.is_finished())
        {
            return Ok(());
        }
        let Some(flight) = self.preview.take() else {
            return Ok(());
        };
        let cancelled = flight.request.is_cancelled()
            || flight
                .request
                .transport
                .as_ref()
                .is_some_and(|p| p.is_transport_closed());
        let delivery = flight
            .candidate
            .as_ref()
            .map(|t| self.session.check_candidate_delivery(app, t, blocked))
            .transpose();
        let outcome = match flight.worker.join() {
            Ok(_) if cancelled => Err(AutomationError::PreviewCancelled),
            Ok(outcome) => outcome,
            Err(_) => Err(AutomationError::Execution(
                "preview worker failed; retry or reopen the project".into(),
            )),
        };
        if let Err(error) = delivery {
            // Never write an old-project audit into the newly opened project.
            if error.code != "project_changed" {
                flight
                    .run
                    .finish(app, &Err(AutomationError::Execution(error.message.clone())))?;
            }
            return respond(flight.request, Err(error));
        }
        if let Err(error) = flight.run.finish(app, &outcome) {
            return respond(flight.request, Err(error.into()));
        }
        let result = if cancelled {
            Err(Failure::new(
                "request_cancelled",
                "Preview request cancelled",
            ))
        } else {
            outcome.map(|v| serde_json::json!(v)).map_err(Failure::from)
        };
        respond(flight.request, result)
    }
}
fn respond(
    request: PendingRequest,
    result: Result<serde_json::Value, Failure>,
) -> Result<(), Failure> {
    let result = request.prepare_response(result);
    let status = result.as_ref().map(|_| ()).map_err(Clone::clone);
    if request.response.send(result).is_err() {
        // Disconnection is normal; no live document mutation is waiting to commit.
    }
    status
}
