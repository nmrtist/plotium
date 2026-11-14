use super::*;

pub(crate) fn poll(app: &mut PlotiumApp, ctx: &egui::Context) {
    MANAGER.with(|manager| {
        let mut manager = manager.borrow_mut();
        let mut completed = Vec::new();
        for (index, job) in manager.jobs.iter_mut().enumerate() {
            if job.project_context != app.session.project_context {
                job.cancelled.store(true, Ordering::Relaxed);
                if job.consent.send(false).is_err() { log::debug!("Image consent receiver already closed"); }
                completed.push(index);
                continue;
            }
            for _ in 0..4 {
                let event = match job.receiver.try_recv() {
                    Ok(event) => event,
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        let mut staged = std::mem::take(&mut job.staged);
                        staged.push(Err(failure(
                            "<batch>",
                            "unknown",
                            "worker",
                            "the image import worker stopped before returning a result".to_owned(),
                            "Retry the import; if it still fails, review the diagnostic history.",
                        )));
                        // Report partial success and the worker failure together, so
                        // the diagnostic does not incorrectly claim zero imports.
                        commit(app, job, staged);
                        completed.push(index);
                        break;
                    }
                };
                match event {
                    WorkerEvent::State(state) => {
                        job.state = state;
                        app.session.status = format!("Image import: {state:?}.");
                        if state == ImportImageState::Cancelled {
                            let staged = std::mem::take(&mut job.staged);
                            if !staged.is_empty() { commit(app, job, staged); }
                            if job.state == ImportImageState::Committed {
                                app.session.status = "Image import cancelled; prepared images were imported and can be undone together.".to_owned();
                            }
                            completed.push(index);
                            break;
                        }
                    }
                    WorkerEvent::Candidate(mut result) => {
                        // Keep originals and metadata until atomic commit, never decoded
                        // previews for the entire batch. Visible previews are rebuilt on demand.
                        if let Ok(candidate) = &mut result { candidate.preview.rgba8 = Vec::new(); }
                        job.staged.push(result);
                    }
                    WorkerEvent::Finished => {
                        let results = std::mem::take(&mut job.staged);
                        job.state = ImportImageState::ReadyToCommit;
                        commit(app, job, results);
                        completed.push(index);
                        break;
                    }
                }
            }
        }
        completed.sort_unstable();
        completed.dedup();
        for index in completed.into_iter().rev() {
            manager.jobs.remove(index);
        }
        if !manager.jobs.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
    });
}
