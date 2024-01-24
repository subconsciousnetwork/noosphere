use super::Processor;
use anyhow::{anyhow, Result};
use std::{fmt::Debug, time::SystemTime};
use tokio::{
    sync::mpsc::{unbounded_channel, UnboundedSender},
    task::JoinHandle,
    time::Duration,
};

pub(crate) type WorkerResponse<J> = (usize, Result<Option<J>>);

/// A job with additional metadata to handle time outs and retries.
pub struct JobRequest<P: Processor> {
    pub job: P::Job,
    pub attempt: usize,
    pub start_time: Option<SystemTime>,
}

impl<P> JobRequest<P>
where
    P: Processor,
{
    /// Creates a new [JobRequest].
    pub fn new(job: P::Job) -> Self {
        Self {
            job,
            attempt: 0,
            start_time: None,
        }
    }

    /// Marks [JobRequest] as a failed attempt, clearing
    /// its last start time, and returns whether this job
    /// should be retried or not.
    pub fn mark_attempt_failed(&mut self, retries: usize) -> bool {
        self.start_time = None;
        self.attempt < retries
    }
}

impl<P> Debug for JobRequest<P>
where
    P: Processor,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobRequest")
            .field("job", &self.job)
            .field("attempts", &self.attempt)
            .finish()
    }
}

/// Provides an interface to perform a job via [Processor]
/// on another thread.
pub struct Worker<P: Processor> {
    active_job: Option<JobRequest<P>>,
    request_tx: UnboundedSender<P::Job>,
    handle: JoinHandle<Result<()>>,
}

impl<P> Worker<P>
where
    P: Processor,
{
    /// Creates a new [Worker] and starts the worker thread.
    pub fn spawn(
        worker_id: usize,
        context: P::Context,
        response_tx: UnboundedSender<WorkerResponse<P::Job>>,
    ) -> Self {
        let (request_tx, mut request_rx) = unbounded_channel();
        let handle = tokio::spawn(async move {
            while let Some(job) = request_rx.recv().await {
                let result = P::process(context.clone(), job).await;
                response_tx
                    .send((worker_id, result))
                    .map_err(|send_error| {
                        anyhow::anyhow!("Error sending worker message: {}", send_error)
                    })?;
            }
            Ok(())
        });

        Self {
            handle,
            request_tx,
            active_job: None,
        }
    }

    /// Whether this worker is currently processing or not.
    pub fn idle(&self) -> bool {
        self.active_job.is_none()
    }

    /// How long ago from `now` has this worker's active job
    /// been processing, if any.
    pub fn job_elapsed_time(&self, now: &SystemTime) -> Option<Duration> {
        if let Some(active_job) = &self.active_job {
            if let Some(start_time) = active_job.start_time {
                return match now.duration_since(start_time) {
                    Ok(job_elapsed) => Some(job_elapsed),
                    Err(_) => None,
                };
            }
        }
        None
    }

    /// Resets the worker's interface state to being idle,
    /// returning its active job if any.
    pub fn clear(&mut self) -> Option<JobRequest<P>> {
        self.active_job.take()
    }

    /// Submit job request to be performed on the given
    /// worker, and updating records accordingly.
    pub fn process_job(&mut self, mut job_request: JobRequest<P>) -> Result<()> {
        if !self.idle() {
            return Err(anyhow!("Worker is not idle, cannot process new jobs."));
        }

        let job = job_request.job.clone();
        job_request.attempt += 1;
        job_request.start_time = Some(SystemTime::now());

        self.active_job = Some(job_request);
        // This should only fail if all of our channels
        // are broken.
        self.request_tx
            .send(job)
            .map_err(|_| anyhow::anyhow!("Error sending job."))
    }

    /// Terminates the worker thread and returns the active job, if any.
    /// A terminated worker cannot process further jobs.
    pub fn terminate(&mut self) -> Option<JobRequest<P>> {
        let job_request = self.active_job.take();
        self.handle.abort();
        job_request
    }
}

impl<P> Drop for Worker<P>
where
    P: Processor,
{
    fn drop(&mut self) {
        self.terminate();
    }
}
