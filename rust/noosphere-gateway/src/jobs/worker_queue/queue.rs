use super::{queue_thread::WorkerQueueThread, Processor};
use anyhow::Result;
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::mpsc::{unbounded_channel, UnboundedSender},
    task::JoinHandle,
};

/// An abstraction around managing several worker threads, and distributing
/// work amongst them.
///
/// To terminate all processing, all references to the [WorkerQueue]
/// must be dropped.
#[derive(Debug, Clone)]
pub struct WorkerQueue<P: Processor> {
    handle: Option<Arc<JoinHandle<Result<()>>>>,
    request_tx: UnboundedSender<P::Job>,
}

impl<P> WorkerQueue<P>
where
    P: Processor,
{
    /// Creates a new [WorkerQueue] and starts its worker threads.
    ///
    /// By default, `retries` is set to 1 and `timeout` is 3 minutes.
    pub fn spawn(
        worker_count: usize,
        worker_context: P::Context,
        retries: Option<usize>,
        timeout: Option<Duration>,
    ) -> Self {
        let (request_tx, request_rx) = unbounded_channel();
        let handle = Some(Arc::new(tokio::spawn(async move {
            let workers = WorkerQueueThread::<P>::new(
                worker_count,
                worker_context,
                retries.unwrap_or(1),
                timeout.unwrap_or_else(|| Duration::from_secs(60 * 5)),
                request_rx,
            );
            workers.start().await.map_err(|error| {
                error!("Unrecoverable WorkerQueueThread error: {}", error);
                error
            })
        })));

        Self { handle, request_tx }
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
    P: Processor,
{
    fn drop(&mut self) {
        if let Some(probably_handle) = self.handle.take() {
            if let Some(handle) = Arc::into_inner(probably_handle) {
                handle.abort();
            }
        }
    }
}
