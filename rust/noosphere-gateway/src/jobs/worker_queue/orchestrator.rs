use super::{
    worker::{JobRequest, Worker, WorkerResponse},
    Processor,
};
use anyhow::{anyhow, Result};
use std::{
    collections::VecDeque,
    time::{Duration, SystemTime},
};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

#[cfg(doc)]
use super::WorkerQueue;

/// [WorkerQueueOrchestrator] is where work is orchestrated
/// from a [WorkerQueue].
///
/// The orchestrator spins up worker threads, receives job requests
/// over a message channel, and sends work to available worker
/// threads for processing. The orchestrator is also responsible
/// for terminating/restarting worker threads that surpass
/// timeout configuration, and handles retrying failed jobs up
/// to the configured limit.
pub struct WorkerQueueOrchestrator<P: Processor> {
    workers: Vec<Worker<P>>,
    retries: usize,
    timeout: Duration,
    job_queue: VecDeque<JobRequest<P>>,
    request_rx: Option<UnboundedReceiver<P::Job>>,
    response_rx: Option<UnboundedReceiver<WorkerResponse<P::Job>>>,
    worker_context: P::Context,
    response_tx: UnboundedSender<WorkerResponse<P::Job>>,
}

impl<P> WorkerQueueOrchestrator<P>
where
    P: Processor,
{
    /// Creates a new [WorkerQueueOrchestrator], creating
    /// `worker_count` threads.
    pub fn new(
        worker_count: usize,
        worker_context: P::Context,
        retries: usize,
        timeout: Duration,
        request_rx: UnboundedReceiver<P::Job>,
    ) -> Result<Self> {
        if worker_count == 0 {
            return Err(anyhow!("worker_count must be greater than 0."));
        }

        let (response_tx, response_rx) = unbounded_channel();

        let mut workers = vec![];
        for i in 0..worker_count {
            let worker = Worker::spawn(i, worker_context.clone(), response_tx.clone());
            workers.push(worker);
        }

        Ok(Self {
            job_queue: VecDeque::new(),
            workers,
            retries,
            timeout,
            request_rx: Some(request_rx),
            response_rx: Some(response_rx),
            response_tx,
            worker_context,
        })
    }

    /// Submits unprocessed jobs to available workers.
    fn process_queue(&mut self) -> Result<()> {
        if self.job_queue.is_empty() {
            return Ok(());
        }

        for worker in self.workers.iter_mut() {
            if worker.idle() {
                if let Some(job_request) = self.job_queue.pop_front() {
                    worker.process_job(job_request)?;
                } else {
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    /// Given a [JobRequest] that has failed (via error result or timeout),
    /// either discard with a message if sufficient retries have been attempted,
    /// or requeue the job.
    fn process_failed_job(&mut self, mut job_request: JobRequest<P>) {
        match job_request.mark_attempt_failed(self.retries) {
            true => self.job_queue.push_back(job_request),
            false => {
                error!("Job reached retry limit: {:#?}", job_request);
            }
        }
    }

    /// Process a result from a [Worker], marking the worker as idle,
    /// and potentially queueing a subsequently requested job, or
    /// retrying the failed attempt.
    fn process_result(&mut self, worker_id: usize, result: Result<Option<P::Job>>) -> Result<()> {
        let worker = self
            .workers
            .get_mut(worker_id)
            .ok_or_else(|| anyhow!("Worker index out of bounds."))?;
        let job_request = match worker.clear() {
            Some(job) => Ok(job),
            None => Err(anyhow!("Worker result found for a worker without a job.")),
        }?;

        match result {
            Ok(Some(next_job)) => {
                self.job_queue.push_back(JobRequest::<P>::new(next_job));
            }
            Err(e) => {
                error!("Error processing job: {}", e);
                self.process_failed_job(job_request);
            }
            _ => {}
        };
        Ok(())
    }

    /// Check for timed out workers, restart them, and attempt to
    /// reprocess their failed jobs.
    fn process_timed_out_jobs(&mut self) -> Result<()> {
        /// Terminate and recreate worker at `index`.
        fn cycle_worker<P: Processor>(
            queue_thread: &mut WorkerQueueOrchestrator<P>,
            index: usize,
        ) -> Option<JobRequest<P>> {
            let worker = Worker::spawn(
                index,
                queue_thread.worker_context.clone(),
                queue_thread.response_tx.clone(),
            );
            let mut old_worker = std::mem::replace(&mut queue_thread.workers[index], worker);
            old_worker.terminate()
        }

        let timeout = self.timeout;
        let now = SystemTime::now();
        let mut timed_out_worker_indices = vec![];
        for (index, worker) in self.workers.iter().enumerate() {
            if let Some(elapsed) = worker.job_elapsed_time(&now) {
                if elapsed >= timeout {
                    timed_out_worker_indices.push(index);
                }
            }
        }
        for index in timed_out_worker_indices {
            if let Some(active_job) = cycle_worker(self, index) {
                self.process_failed_job(active_job);
            }
        }
        Ok(())
    }

    /// Returns a [Duration] of when the next check for job timeouts
    /// should occur.
    ///
    /// For example, if the timeout is set to 3 minutes, and the current
    /// longest running job is currently 1 minute into processing, the
    /// next time to check for timed out jobs is in 2 minutes.
    fn get_timeout_check_duration(&self) -> Duration {
        let mut max_duration = self.timeout;
        let now = SystemTime::now();
        for worker in self.workers.iter() {
            if let Some(elapsed) = worker.job_elapsed_time(&now) {
                max_duration = max_duration.min(elapsed);
            }
        }
        max_duration
    }

    /// Start the processing of incoming requests
    /// on the current thread.
    pub async fn start(mut self) -> Result<()> {
        // Take our receivers so this loop doesn't need
        // a mutable reference.
        let mut response_rx = self.response_rx.take().unwrap();
        let mut request_rx = self.request_rx.take().unwrap();
        loop {
            let timeout_check = tokio::time::sleep(self.get_timeout_check_duration());
            tokio::pin!(timeout_check);

            tokio::select! {
                // Request to process a new job
                Some(job) = request_rx.recv() => {
                    self.job_queue.push_back(JobRequest::<P>::new(job));
                }
                // Response from a worker
                Some((worker_id, result)) = response_rx.recv() => {
                    self.process_result(worker_id, result)?;
                }
                // Wait for the most recent job to hit the timeout
                _ = &mut timeout_check => {
                    self.process_timed_out_jobs()?;
                }
            }
            self.process_queue()?;
        }
        #[allow(unreachable_code)]
        Ok(())
    }
}
