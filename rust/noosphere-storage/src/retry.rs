use anyhow::{anyhow, Result};
use async_trait::async_trait;
use cid::Cid;
use std::time::{Duration, Instant};
use tokio::select;

use crate::BlockStore;

const DEFAULT_MAX_RETRIES: u32 = 2u32;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(2);
const DEFAULT_MINIMUM_DELAY: Duration = Duration::from_secs(1);
const DEFAULT_BACKOFF: Backoff = Backoff::Exponential {
    exponent: 2f32,
    ceiling: Duration::from_secs(10),
};

/// Backoff configuration used to define how [BlockStoreRetry] should time
/// further attempts when store reads fail.
#[derive(Clone)]
pub enum Backoff {
    /// The time between retry attempts increases linearly
    Linear {
        /// Increment to increase the next time window by
        increment: Duration,
        /// The maximum time window length
        ceiling: Duration,
    },
    /// The time between retry attempts increases exponentially
    Exponential {
        /// The power to increase the next time window by
        exponent: f32,
        /// The maximum time window length
        ceiling: Duration,
    },
}

impl Backoff {
    /// Apply the backoff configuration to a given input [Duration] and return
    /// the result
    pub fn step(&self, duration: Duration) -> Duration {
        match self {
            Backoff::Linear { increment, ceiling } => (duration + *increment).min(*ceiling),
            Backoff::Exponential { exponent, ceiling } => {
                Duration::from_secs_f32(duration.as_secs_f32().powf(*exponent)).min(*ceiling)
            }
        }
    }
}

/// Implements retry and timeout logic for accessing blocks from a [BlockStore].
/// Any [BlockStore] can be wrapped by [BlockStoreRetry] to get retry and
/// timeout logic for free. Each attempt to lookup a block is time limited by to
/// a specified window with optional [Backoff], and at most `maximum_retries`
/// will be made to load the block.
///
/// Local [BlockStore] implementations won't benefit a lot from this, but
/// network implementations such as [IpfsStore] can be made more reliable with a
/// modest retry policy (and timeouts will help make sure we don't hang
/// indefinitely waiting for an implementation like Kubo to get its act
/// together).
#[derive(Clone)]
pub struct BlockStoreRetry<S>
where
    S: BlockStore,
{
    /// A [BlockStore] implementation that the [BlockStoreRetry] proxies
    /// reads to in order to implement retry behavior
    pub store: S,
    /// The maximum number of additional attempts to make if a read to the
    /// wrapped store should fail
    pub maximum_retries: u32,
    /// The maximum time that a read is allowed to take before it is considered
    /// failed
    pub attempt_window: Duration,
    /// The minimum time between attempts
    pub minimum_delay: Duration,
    /// If a [Backoff] is configured, the attempt window will grow with each
    /// attempt based on the configuration
    pub backoff: Option<Backoff>,
}

impl<S> From<S> for BlockStoreRetry<S>
where
    S: BlockStore,
{
    fn from(store: S) -> Self {
        Self {
            store,
            maximum_retries: DEFAULT_MAX_RETRIES,
            attempt_window: DEFAULT_TIMEOUT,
            minimum_delay: DEFAULT_MINIMUM_DELAY,
            backoff: Some(DEFAULT_BACKOFF),
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S> BlockStore for BlockStoreRetry<S>
where
    S: BlockStore,
{
    async fn put_block(&mut self, cid: &Cid, block: &[u8]) -> Result<()> {
        self.store.put_block(cid, block).await
    }

    async fn get_block(&self, cid: &Cid) -> Result<Option<Vec<u8>>> {
        let mut retry_count = 0;
        let mut next_timeout = self.attempt_window;

        loop {
            if retry_count > self.maximum_retries {
                break;
            }

            let window_start = Instant::now();

            select! {
                result = self.store.get_block(cid) => {
                    match result {
                        Ok(maybe_block) => {
                            return Ok(maybe_block);
                        },
                        Err(error) => {
                          warn!("Error while getting {}: {}", cid, error);
                        },
                    };
                },
                _ = tokio::time::sleep(next_timeout) => {
                    warn!("Timed out trying to get {} after {} seconds...", cid, next_timeout.as_secs());
                }
            }

            let spent_window_time = Instant::now() - window_start;
            let remaining_window_time = spent_window_time.max(self.minimum_delay);

            retry_count += 1;

            if let Some(backoff) = &self.backoff {
                next_timeout = backoff.step(next_timeout);
            }

            tokio::time::sleep(remaining_window_time).await;
        }

        Err(anyhow!(
            "Failed to get {} after {} tries...",
            cid,
            retry_count
        ))
    }
}
