//! A document-scoped background preflight. Execute only commits frozen candidates.
use crate::{Connection, Failure, PendingRequest};
use plotium_core::{automation::*, state::PlotiumApp};
use std::thread::JoinHandle;
use tokio_util::sync::CancellationToken;
pub(crate) struct ImportFlight {
    worker: JoinHandle<Result<PreparedDataImport, AutomationError>>,
    plan: ToolPlan,
    request: PendingRequest,
    cancellation: CancellationToken,
}
impl ImportFlight {
    pub(crate) fn cancel(self) {
        self.cancellation.cancel();
        if self
            .request
            .response
            .send(Err(Failure::new(
                "project_changed",
                "Project changed; reread state and replan",
            )))
            .is_err()
        { /* Client disconnected. */ }
    }
}
impl Connection {
    pub(crate) fn dispatch_import(
        &mut self,
        app: &mut PlotiumApp,
        request: PendingRequest,
        blocked: bool,
    ) -> Result<(), Failure> {
        if self.import.is_some() {
            return respond(
                request,
                Err(Failure::new(
                    "busy",
                    "An import preflight is still running; retry after it finishes",
                )),
            );
        }
        let (plan, params) =
            match self
                .session
                .prepare_import_plan(app, request.arguments.clone(), blocked)
            {
                Ok(p) => p,
                Err(e) => return respond(request, Err(e)),
            };
        // Client cancellation reaches the worker, but a project change must
        // stop only the worker so HTTP can still deliver project_changed.
        let cancellation = request.cancellation.child_token();
        let worker_cancellation = cancellation.clone();
        let active = self.active.clone();
        let budget = self.session.remaining_import_bytes();
        let worker = std::thread::Builder::new()
            .name("plotium-data-import".into())
            .spawn(move || {
                PreparedDataImport::prepare_with_budget(params, budget, || {
                    worker_cancellation.is_cancelled()
                        || !active.load(std::sync::atomic::Ordering::Acquire)
                })
            });
        match worker {
            Ok(worker) => {
                self.import = Some(ImportFlight {
                    worker,
                    plan,
                    request,
                    cancellation,
                });
                Ok(())
            }
            Err(error) => respond(
                request,
                Err(Failure::new(
                    "execution_failed",
                    format!("Could not start import worker: {error}"),
                )),
            ),
        }
    }
    pub fn poll_import(&mut self, app: &mut PlotiumApp, blocked: bool) -> Result<(), Failure> {
        self.synchronize_project(app);
        if !self.import.as_ref().is_some_and(|f| f.worker.is_finished()) {
            return Ok(());
        }
        let Some(flight) = self.import.take() else {
            return Ok(());
        };
        let cancelled = flight.request.is_cancelled();
        let prepared = match flight.worker.join() {
            Ok(v) => v.map_err(Failure::from),
            Err(_) => Err(Failure::new(
                "execution_failed",
                "Import worker stopped unexpectedly; preflight again",
            )),
        };
        let result = if cancelled {
            Err(Failure::new(
                "request_cancelled",
                "Import preflight cancelled; no data was created",
            ))
        } else {
            prepared.and_then(|p| {
                self.session
                    .finish_import_plan(app, flight.plan, p, blocked)
            })
        };
        if let Err(error) = &result {
            app.session.status = format!("Import preflight failed: {}", error.message);
        } else if let Ok(value) = &result
            && let Some(error) = value["import"]["items"]
                .as_array()
                .and_then(|items| items.iter().find_map(|i| i["error"].as_str()))
        {
            app.session.status = format!(
                "Import preflight: {} ready, {} failed. {error}",
                value["import"]["prepared_files"], value["import"]["failed_files"]
            );
        }
        let id = result
            .as_ref()
            .ok()
            .and_then(|v| v.get("plan_id"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let status = result.as_ref().map(|_| ()).map_err(Clone::clone);
        if flight.request.response.send(result).is_err()
            && let Some(id) = id
        {
            self.session.discard_import_plan(&id);
        }
        status
    }
}
fn respond(
    request: PendingRequest,
    result: Result<serde_json::Value, Failure>,
) -> Result<(), Failure> {
    let status = result.as_ref().map(|_| ()).map_err(Clone::clone);
    if request.response.send(result).is_err() { /* Client disconnected; preflight did not mutate data. */
    }
    status
}

#[cfg(test)]
#[path = "import_tests.rs"]
mod tests;
