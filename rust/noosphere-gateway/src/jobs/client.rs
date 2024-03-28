use crate::jobs::GatewayJob;
use anyhow::Result;
use std::{ops::Deref, sync::Arc};

/// [JobClient] allows a gateway or other service
/// to submit jobs to be processed.
pub trait JobClient: Send + Sync {
    /// Submit a [GatewayJob] to be processed.
    fn submit(&self, job: GatewayJob) -> Result<()>;
}

impl<T> JobClient for Arc<T>
where
    T: JobClient,
{
    fn submit(&self, job: GatewayJob) -> Result<()> {
        self.deref().submit(job)
    }
}
