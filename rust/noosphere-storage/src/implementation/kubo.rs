use anyhow::Result;
use async_trait::async_trait;
use cid::Cid;
use reqwest::Client;
use reqwest::StatusCode;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
use url::Url;

#[cfg(doc)]
use noosphere_storage::KeyValueStore;

use crate::BlockStore;
use crate::Storage;

/// [KuboStorage] is an implementation of [Storage] that wraps another
/// implementation of [Storage]. [KuboStorage] is generic over [BlockStore]
/// and [KeyValueStore] but will produce a [KuboStore] wrapped [BlockStore]
#[derive(Clone)]
pub struct KuboStorage<S>
where
    S: Storage,
{
    local_storage: S,
    ipfs_api: Option<Url>,
}

impl<S> KuboStorage<S>
where
    S: Storage,
{
    pub fn new(local_storage: S, ipfs_api: Option<&Url>) -> Self {
        KuboStorage {
            local_storage,
            ipfs_api: ipfs_api.cloned(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub trait KuboStorageConditionalSendSync: Send + Sync {}

#[cfg(not(target_arch = "wasm32"))]
impl<S> KuboStorageConditionalSendSync for S where S: Send + Sync {}

#[cfg(target_arch = "wasm32")]
pub trait KuboStorageConditionalSendSync {}

#[cfg(target_arch = "wasm32")]
impl<S> KuboStorageConditionalSendSync for S {}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S> Storage for KuboStorage<S>
where
    S: Storage + KuboStorageConditionalSendSync,
{
    type BlockStore = KuboStore<S::BlockStore>;

    type KeyValueStore = S::KeyValueStore;

    async fn get_block_store(&self, name: &str) -> Result<Self::BlockStore> {
        let store = self.local_storage.get_block_store(name).await?;
        Ok(KuboStore::new(store, self.ipfs_api.as_ref()))
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
pub struct KuboStore<B>
where
    B: BlockStore,
{
    local_store: Arc<RwLock<B>>,
    ipfs_api: Option<Url>,
    client: Client,
}

impl<B> KuboStore<B>
where
    B: BlockStore,
{
    pub fn new(block_store: B, ipfs_api: Option<&Url>) -> Self {
        KuboStore {
            local_store: Arc::new(RwLock::new(block_store)),
            ipfs_api: ipfs_api.cloned(),
            client: Client::new(),
        }
    }

    /// Set the IPFS gateway API URL for this [KuboStore]. Note that this state
    /// is not retroactively shared among all clones of the same store.
    pub fn configure_ipfs_api_url(&mut self, ipfs_api: Option<&Url>) {
        self.ipfs_api = ipfs_api.cloned();
    }

    /// Some gateways will redirect you to a subdomain derived from the CID
    /// being requested. This method helps us to determine if we should use a
    /// subdomain-scoped URL or an origin-based URL to find the next block.
    fn make_block_url(&self, cid: &Cid) -> Option<Url> {
        let mut url = if let Some(url) = &self.ipfs_api {
            url.clone()
        } else {
            return None;
        };

        if let Some(domain) = url.domain() {
            let mut parts = domain.split('.');

            if let Some(fragment) = parts.nth(0) {
                if Cid::from_str(fragment).is_ok() {
                    let upper_domain: String = parts.collect();
                    let mut host = format!("{}://{}.{}", url.scheme(), cid, upper_domain);

                    if let Some(port) = url.port() {
                        host = format!("{}:{}", domain, port);
                    }

                    if let Ok(()) = url.set_host(Some(&host)) {
                        return Some(url);
                    }
                }
            }
        }

        url.set_path(&format!("/ipfs/{}", cid));

        Some(url)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<B> BlockStore for KuboStore<B>
where
    B: BlockStore,
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

        match maybe_block {
            Some(block) => Ok(Some(block)),
            None => match &self.make_block_url(cid) {
                Some(ipfs_api) => {
                    let response = self
                        .client
                        .get(ipfs_api.clone())
                        .header("Accept", "application/vnd.ipld.raw")
                        .send()
                        .await?;

                    match response.status() {
                        StatusCode::OK => {
                            let bytes = response.bytes().await?;
                            let mut local_store = self.local_store.write().await;
                            local_store.put_block(cid, &bytes).await?;

                            Ok(Some(bytes.into()))
                        }
                        _ => {
                            error!("Unable to retrieve block from gateway at {ipfs_api}!");
                            Ok(None)
                        }
                    }
                }
                None => Ok(None),
            },
        }
    }
}
