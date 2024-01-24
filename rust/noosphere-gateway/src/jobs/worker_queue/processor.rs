use anyhow::Result;
use async_trait::async_trait;
use std::fmt::Debug;

#[cfg(doc)]
use super::WorkerQueue;

/// An interface to perform work within the context
/// of a [WorkerQueue].
#[async_trait]
pub trait Processor: Clone {
    /// Type passed to each job processor to derive additional
    /// data beyond the job details.
    type Context: Clone + Send + Sync + 'static;
    /// Type representing an individual unit of work.
    type Job: Debug + Clone + Send + 'static;

    /// Processes an asynchronous [Self::Job].
    ///
    /// On success, may optionally return a [Self::Job] to be
    /// subsequently queued.
    async fn process(context: Self::Context, job: Self::Job) -> Result<Option<Self::Job>>;
}
