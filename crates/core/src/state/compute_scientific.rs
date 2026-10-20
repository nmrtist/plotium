//! Prepared scientific work shares the existing pool and cancellation guards.
use super::*;
pub(crate) type ScientificWork =
    Box<dyn FnOnce(CancellationToken) -> Result<ScientificOutput, String> + Send>;
#[derive(Clone)]
pub struct ScientificOutput {
    pub(crate) create_canvas: bool,
    pub action: Box<crate::actions::Action>,
    pub summary: serde_json::Value,
    pub produced: Vec<crate::automation::ResourceRef>,
}
impl ComputeService {
    pub(crate) fn enqueue_scientific(
        &mut self,
        dataset: DatasetId,
        work: ScientificWork,
    ) -> Result<u64, EnqueueError> {
        if let Some(kind) = self.blocking_work_for(dataset) {
            return Err(EnqueueError::Busy(kind));
        }
        let kind = ComputeKind::Scientific;
        let generation = self.next_generation(dataset, kind);
        let token = CancellationToken::new();
        self.active.insert(
            (dataset, kind),
            ActiveJob {
                generation,
                started_at: Instant::now(),
                token: token.clone(),
                processing_input: None,
            },
        );
        if self
            .job_tx
            .send(Job::Scientific {
                generation,
                dataset,
                token,
                work,
            })
            .is_err()
        {
            self.cancel_failed_enqueue(dataset, kind, generation);
            return Err(EnqueueError::WorkersUnavailable);
        }
        Ok(generation)
    }
}
