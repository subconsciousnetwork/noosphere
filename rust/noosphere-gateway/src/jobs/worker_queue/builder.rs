use super::{Processor, WorkerQueue};
use anyhow::{anyhow, Result};
use std::time::Duration;

/// Builder helper for [WorkerQueue].
pub struct WorkerQueueBuilder<P: Processor> {
    worker_count: usize,
    retries: Option<usize>,
    timeout: Option<Duration>,
    context: Option<P::Context>,
}

impl<P> WorkerQueueBuilder<P>
where
    P: Processor,
{
    /// Creates a new [WorkerQueueBuilder].
    pub fn new() -> Self {
        Self {
            worker_count: 1,
            retries: Some(1),
            timeout: Some(Duration::from_secs(60 * 3)),
            context: None,
        }
    }

    /// Number of worker threads.
    pub fn with_worker_count(mut self, worker_count: usize) -> Self {
        self.worker_count = worker_count;
        self
    }

    /// How long in seconds before a task is considered timed out.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Default number of retries per failed job.
    pub fn with_retries(mut self, retries: usize) -> Self {
        self.retries = Some(retries);
        self
    }

    /// Context to use when processing jobs.
    pub fn with_context(mut self, context: P::Context) -> Self {
        self.context = Some(context);
        self
    }

    /// Build a [WorkerQueue] from collected parameters.
    pub fn build(self) -> Result<WorkerQueue<P>> {
        if self.worker_count < 1 {
            return Err(anyhow!("worker_count must be greater than 0."));
        }
        if self.context.is_none() {
            return Err(anyhow!("context must be provided."));
        }

        Ok(WorkerQueue::spawn(
            self.worker_count,
            self.context.unwrap(),
            self.retries,
            self.timeout,
        ))
    }
}

impl<P> Default for WorkerQueueBuilder<P>
where
    P: Processor,
{
    fn default() -> Self {
        Self::new()
    }
}
