// TODO(#629): Remove this when we migrate off of `release-please`
extern crate noosphere_cli_dev as noosphere_cli;
extern crate noosphere_ns_dev as noosphere_ns;

use anyhow::{anyhow, Result};
use noosphere_cli::workspace::CliSphereContext;
use noosphere_core::{
    context::{SpherePetnameRead, SphereSync},
    data::{Link, MemoIpld},
};
use std::sync::Arc;
use tokio::{sync::Mutex, time};

/// After adding a petname and pushing changes, the gateway will queue
/// up a job to resolve the petname and add the resolved value
/// to the gateway sphere. Afterwards, a sync will pull the resolved
/// petname. This function repeatedly polls until the petname is resolved
/// to the `expected` value, until `timeout` in seconds has been reached (default 5 seconds).
pub async fn wait_for_petname(
    mut ctx: Arc<Mutex<CliSphereContext>>,
    petname: &str,
    expected: Option<Link<MemoIpld>>,
    timeout: Option<u64>,
) -> Result<()> {
    let timeout = timeout.unwrap_or(5);
    let mut attempts = timeout;
    loop {
        if attempts < 1 {
            return Err(anyhow!(
                "Context failed to resolve \"{}\" to expected value after {} seconds.",
                petname,
                timeout
            ));
        }

        let resolved = {
            time::sleep(time::Duration::from_secs(1)).await;
            ctx.sync().await?;
            ctx.resolve_petname(petname).await?
        };

        if resolved == expected {
            return Ok(());
        }
        attempts -= 1;
    }
}
