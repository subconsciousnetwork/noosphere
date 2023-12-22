use crate::IpfsClient;
use anyhow::Result;
use async_trait::async_trait;
use cid::Cid;
use noosphere_common::ConditionalSync;
use noosphere_storage::{BlockStore, EphemeralStorage, EphemeralStore, Storage};
use std::sync::Arc;
use tokio::sync::RwLock;

#[cfg(doc)]
use noosphere_storage::KeyValueStore;

/// [IpfsStorage] is an implementation of [Storage] that wraps another
/// implementation of [Storage] and an [IpfsClient].
/// [IpfsStorage] is generic over [BlockStore] and [KeyValueStore]
/// but will produce a [IpfsStore] wrapped [BlockStore]
#[derive(Clone, Debug)]
pub struct IpfsStorage<S, C>
where
    S: Storage,
    C: IpfsClient,
{
    local_storage: S,
    ipfs_client: Option<C>,
}

impl<S, C> IpfsStorage<S, C>
where
    S: Storage,
    C: IpfsClient,
{
    pub fn new(local_storage: S, ipfs_client: Option<C>) -> Self {
        IpfsStorage {
            local_storage,
            ipfs_client,
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S, C> Storage for IpfsStorage<S, C>
where
    S: Storage + ConditionalSync,
    C: IpfsClient + ConditionalSync,
{
    type BlockStore = IpfsStore<S::BlockStore, C>;
    type KeyValueStore = S::KeyValueStore;

    async fn get_block_store(&self, name: &str) -> Result<Self::BlockStore> {
        let store = self.local_storage.get_block_store(name).await?;
        Ok(IpfsStore::new(store, self.ipfs_client.clone()))
    }

    async fn get_key_value_store(&self, name: &str) -> Result<Self::KeyValueStore> {
        self.local_storage.get_key_value_store(name).await
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S, C> EphemeralStorage for IpfsStorage<S, C>
where
    S: Storage + EphemeralStorage + ConditionalSync,
    C: IpfsClient + ConditionalSync,
{
    type EphemeralStoreType = <S as EphemeralStorage>::EphemeralStoreType;

    async fn get_ephemeral_store(&self) -> Result<EphemeralStore<Self::EphemeralStoreType>> {
        self.local_storage.get_ephemeral_store().await
    }
}

/// An implementation of [BlockStore] that wraps some other implementation of
/// same. It forwards most behavior to its wrapped implementation, except when
/// reading blocks. In that case, if a block cannot be found locally, it will
/// attempt to fail-over by requesting the block from a configured IPFS gateway
/// API. If the block is found, it is added to local storage and then returned
/// as normal
#[derive(Clone)]
pub struct IpfsStore<B, C>
where
    B: BlockStore,
    C: IpfsClient + ConditionalSync,
{
    local_store: Arc<RwLock<B>>,
    ipfs_client: Option<C>,
}

impl<B, C> IpfsStore<B, C>
where
    B: BlockStore,
    C: IpfsClient + ConditionalSync,
{
    pub fn new(block_store: B, ipfs_client: Option<C>) -> Self {
        IpfsStore {
            local_store: Arc::new(RwLock::new(block_store)),
            ipfs_client,
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<B, C> BlockStore for IpfsStore<B, C>
where
    B: BlockStore,
    C: IpfsClient + ConditionalSync,
{
    #[instrument(skip(self), level = "trace")]
    async fn put_block(&mut self, cid: &Cid, block: &[u8]) -> Result<()> {
        let mut local_store = self.local_store.write().await;
        local_store.put_block(cid, block).await
    }

    #[instrument(skip(self), level = "trace")]
    async fn get_block(&self, cid: &Cid) -> Result<Option<Vec<u8>>> {
        trace!("Looking up block locally...");
        let maybe_block = {
            let local_store = self.local_store.read().await;
            local_store.get_block(cid).await?
        };

        if let Some(block) = maybe_block {
            trace!("Found block locally!");
            return Ok(Some(block));
        }

        trace!("Block not available locally...");

        if let Some(ipfs_client) = self.ipfs_client.as_ref() {
            trace!("Looking up block in IPFS...");
            if let Some(bytes) = ipfs_client.get_block(cid).await? {
                trace!("Found block in IPFS!");
                let mut local_store = self.local_store.write().await;
                local_store.put_block(cid, &bytes).await?;
                return Ok(Some(bytes));
            }
        }
        Ok(None)
    }
}

// Note that these tests require that there is a locally available IPFS Kubo
// node running with the RPC API enabled
#[cfg(all(test, feature = "test-kubo", not(target_arch = "wasm32")))]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::KuboClient;
    use libipld_cbor::DagCborCodec;
    use noosphere_core::tracing::initialize_tracing;
    use noosphere_storage::{block_serialize, BlockStoreRetry, MemoryStore};
    use rand::prelude::*;
    use serde::{Deserialize, Serialize};
    use url::Url;

    #[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
    struct TestData {
        value_a: i64,
        value_b: i64,
    }

    /// Fetching a block from IPFS that isn't already on IPFS can hang
    /// indefinitely. This test ensures that [BlockStoreRetry] wraps
    /// [IpfsStore] successfully, producing an error.
    #[tokio::test]
    pub async fn it_fails_gracefully_if_block_not_found() {
        initialize_tracing(None);

        let mut rng = thread_rng();
        let foo = TestData {
            // uniquely generate value such that
            // it is not found on the IPFS network.
            value_a: rng.gen(),
            value_b: rng.gen(),
        };

        let (foo_cid, _) = block_serialize::<DagCborCodec, _>(foo.clone()).unwrap();

        let ipfs_url = Url::parse("http://127.0.0.1:5001").unwrap();
        let kubo_client = KuboClient::new(&ipfs_url).unwrap();
        let ipfs_store = {
            let inner = MemoryStore::default();
            let inner = IpfsStore::new(inner, Some(kubo_client));
            BlockStoreRetry {
                store: inner,
                maximum_retries: 1,
                attempt_window: Duration::from_millis(100),
                minimum_delay: Duration::from_millis(100),
                backoff: None,
            }
        };

        assert!(ipfs_store.get_block(&foo_cid).await.is_err());
    }
}
