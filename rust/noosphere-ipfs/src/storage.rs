use crate::IpfsClient;
use anyhow::Result;
use async_trait::async_trait;
use cid::Cid;
use noosphere_storage::{BlockStore, Storage};
use std::sync::Arc;
use tokio::sync::RwLock;

#[cfg(doc)]
use noosphere_storage::KeyValueStore;

/// [IpfsStorage] is an implementation of [Storage] that wraps another
/// implementation of [Storage] and an [IpfsClient].
/// [IpfsStorage] is generic over [BlockStore] and [KeyValueStore]
/// but will produce a [IpfsStore] wrapped [BlockStore]
#[derive(Clone)]
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

#[cfg(not(target_arch = "wasm32"))]
pub trait IpfsStorageConditionalSendSync: Send + Sync {}

#[cfg(not(target_arch = "wasm32"))]
impl<S> IpfsStorageConditionalSendSync for S where S: Send + Sync {}

#[cfg(target_arch = "wasm32")]
pub trait IpfsStorageConditionalSendSync {}

#[cfg(target_arch = "wasm32")]
impl<S> IpfsStorageConditionalSendSync for S {}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S, C> Storage for IpfsStorage<S, C>
where
    S: Storage + IpfsStorageConditionalSendSync,
    C: IpfsClient + IpfsStorageConditionalSendSync,
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
    C: IpfsClient + IpfsStorageConditionalSendSync,
{
    local_store: Arc<RwLock<B>>,
    ipfs_client: Option<C>,
}

impl<B, C> IpfsStore<B, C>
where
    B: BlockStore,
    C: IpfsClient + IpfsStorageConditionalSendSync,
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
    C: IpfsClient + IpfsStorageConditionalSendSync,
{
    async fn put_block(&mut self, cid: &Cid, block: &[u8]) -> Result<()> {
        let mut local_store = self.local_store.write().await;
        local_store.put_block(cid, block).await
    }

    async fn get_block(&self, cid: &Cid) -> Result<Option<Vec<u8>>> {
        let maybe_block = {
            let local_store = self.local_store.read().await;
            local_store.get_block(cid).await?
        };

        if let Some(block) = maybe_block {
            return Ok(Some(block));
        }

        if let Some(ipfs_client) = self.ipfs_client.as_ref() {
            if let Some(bytes) = ipfs_client.get_block(cid).await? {
                let mut local_store = self.local_store.write().await;
                local_store.put_block(cid, &bytes).await?;
                return Ok(Some(bytes));
            }
        }
        Ok(None)
    }
}
