use crate::jobs::GatewayJob;
use anyhow::Result;

/// [JobClient] allows a gateway or other service
/// to submit jobs to be processed.
pub trait JobClient: Clone + Send + Sync {
    /// Submit a [GatewayJob] to be processed.
    fn submit(&self, job: GatewayJob) -> Result<()>;
}
