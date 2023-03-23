use anyhow::{anyhow, Result};
use async_trait::async_trait;
use cid::Cid;
use std::time::Duration;
use tokio::select;

use crate::BlockStore;

/// Implements retry and timeout logic for accessing blocks from a [BlockStore].
/// Any [BlockStore] can be wrapped by [BlockStoreRetry] to get retry and
/// timeout logic for free. Each attempt to lookup a block is time limited by
/// the `timeout` value, and at most `max_retries` will be made to load the
/// block.
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
    store: S,
    timeout: Duration,
    max_retries: u32,
}

impl<S> BlockStoreRetry<S>
where
    S: BlockStore,
{
    pub fn new(store: S, max_retries: u32, timeout: Duration) -> Self {
        BlockStoreRetry {
            store,
            max_retries,
            timeout,
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
        loop {
            if retry_count > self.max_retries {
                break;
            }

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
                _ = tokio::time::sleep(self.timeout.clone()) => {
                    warn!("Timed out trying to get {} after {} seconds...", cid, self.timeout.as_secs());
                }
            }

            retry_count += 1;
        }

        Err(anyhow!(
            "Failed to get {} after {} tries...",
            cid,
            retry_count
        ))
    }
}
