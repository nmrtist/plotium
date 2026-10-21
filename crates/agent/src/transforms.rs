//! A connection-scoped immutable transform preflight; no live state reaches workers.
use crate::{Connection, Failure, PendingRequest};
use plotium_core::{automation::*, state::PlotiumApp};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
};
pub(crate) struct TransformFlight {
    worker: JoinHandle<Result<PreparedTableOperation, AutomationError>>,
    pub(super) cancel: Arc<AtomicBool>,
    plan: ToolPlan,
    request: PendingRequest,
}
impl TransformFlight {
    pub(crate) fn cancel(self) {
        self.cancel.store(true, Ordering::Relaxed);
        let result = self.request.prepare_response(Err(Failure::new(
            "project_changed",
            "Project changed; reread state and replan",
        )));
        if self.request.response.send(result).is_err() { /* Client disconnected. */ }
    }
}
impl Connection {
    pub(crate) fn dispatch_transform(
        &mut self,
        app: &mut PlotiumApp,
        request: PendingRequest,
        blocked: bool,
    ) -> Result<(), Failure> {
        if self.transform.is_some() {
            return respond(
                request,
                Err(Failure::new(
                    "busy",
                    "A transform preflight is still running; retry after completion",
                )),
            );
        }
        let (plan, snapshot) =
            match self
                .session
                .prepare_transform_plan(app, request.arguments.clone(), blocked)
            {
                Ok(v) => v,
                Err(e) => return respond(request, Err(e)),
            };
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        match std::thread::Builder::new()
            .name("plotium-table-transform".into())
            .spawn(move || snapshot.prepare(&worker_cancel))
        {
            Ok(worker) => {
                self.transform = Some(TransformFlight {
                    worker,
                    cancel,
                    plan,
                    request,
                });
                Ok(())
            }
            Err(e) => respond(
                request,
                Err(Failure::new(
                    "execution_failed",
                    format!("Could not start transform worker: {e}"),
                )),
            ),
        }
    }
    pub fn poll_transform(&mut self, app: &mut PlotiumApp, blocked: bool) -> Result<(), Failure> {
        self.synchronize_project(app);
        let Some(flight) = &self.transform else {
            return Ok(());
        };
        if flight.request.is_cancelled() {
            flight.cancel.store(true, Ordering::Relaxed);
        }
        if !flight.worker.is_finished() {
            return Ok(());
        }
        let flight = self.transform.take().expect("flight checked above");
        let TransformFlight {
            worker,
            cancel,
            plan,
            request,
        } = flight;
        let result = worker
            .join()
            .map_err(|_| Failure::new("execution_failed", "Transform worker stopped unexpectedly"))
            .and_then(|r| r.map_err(Failure::from));
        let result = if request.is_cancelled() || cancel.load(Ordering::Relaxed) {
            Err(Failure::new(
                "request_cancelled",
                "Transform cancelled; no data was created",
            ))
        } else {
            result.and_then(|prepared| {
                self.session
                    .finish_transform_plan(app, plan, prepared, blocked)
            })
        };
        let id = result
            .as_ref()
            .ok()
            .and_then(|v| v.get("plan_id"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let result = request.prepare_response(result);
        let status = result.as_ref().map(|_| ()).map_err(Clone::clone);
        if request.response.send(result).is_err()
            && let Some(id) = id
        {
            self.session.discard_transform_plan(&id);
        }
        status
    }
}
fn respond(
    request: PendingRequest,
    result: Result<serde_json::Value, Failure>,
) -> Result<(), Failure> {
    let result = request.prepare_response(result);
    let status = result.as_ref().map(|_| ()).map_err(Clone::clone);
    if request.response.send(result).is_err() { /* Disconnected client; no document mutation. */ }
    status
}
#[cfg(test)]
#[path = "transform_tests.rs"]
mod tests;
