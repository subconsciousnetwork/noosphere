use super::{orchestrator::WorkerQueueOrchestrator, Processor};
use anyhow::Result;
use std::time::Duration;
use tokio::{
    sync::mpsc::{unbounded_channel, UnboundedSender},
    task::JoinHandle,
};

/// [WorkerQueue] is a handle to a pool of workers,
/// and provides an interface to submit jobs to process.
///
/// All processing and threads are terminated upon
/// dropping [WorkerQueue].
#[derive(Debug)]
pub struct WorkerQueue<P: Processor + 'static> {
    handle: JoinHandle<Result<()>>,
    request_tx: UnboundedSender<P::Job>,
}

impl<P> WorkerQueue<P>
where
    P: Processor + 'static,
{
    /// Creates a new [WorkerQueue] and starts its worker threads.
    ///
    /// By default, `worker_count` is 1, `retries` is 0,
    /// and `timeout` is 5 minutes.
    pub fn spawn(
        worker_count: usize,
        worker_context: P::Context,
        retries: Option<usize>,
        timeout: Option<Duration>,
    ) -> Result<Self> {
        let (request_tx, request_rx) = unbounded_channel();
        let workers = WorkerQueueOrchestrator::<P>::new(
            worker_count,
            worker_context,
            retries.unwrap_or(0),
            timeout.unwrap_or_else(|| Duration::from_secs(60 * 5)),
            request_rx,
        )?;
        let handle = tokio::spawn(async move {
            workers.start().await.map_err(|error| {
                error!("Unrecoverable WorkerQueueOrchestrator error: {}", error);
                error
            })
        });

        Ok(Self { handle, request_tx })
    }

    /// Submit a job to be performed on an available worker thread.
    pub fn submit(&self, job: P::Job) -> Result<()> {
        self.request_tx
            .send(job)
            .map_err(|_| anyhow::anyhow!("Error submitting job."))
    }
}

impl<P> Drop for WorkerQueue<P>
where
    P: Processor + 'static,
{
    fn drop(&mut self) {
        self.handle.abort();
    }
}
