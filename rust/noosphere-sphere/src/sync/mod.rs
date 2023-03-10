mod gateway;

use anyhow::Result;
use async_trait::async_trait;
use noosphere_storage::Storage;
use ucan::crypto::KeyMaterial;

use crate::HasMutableSphereContext;

use self::gateway::GatewaySyncStrategy;

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait SphereSync<K, S>
where
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    /// If a gateway URL has been configured, attempt to synchronize local
    /// sphere data with the gateway. Changes on the gateway will first be
    /// fetched to local storage. Then, the local changes will be replayed on
    /// top of those changes. Finally, the synchronized local history will be
    /// pushed up to the gateway.
    async fn sync(&mut self) -> Result<()>;
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<H, K, S> SphereSync<K, S> for H
where
    H: HasMutableSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    async fn sync(&mut self) -> Result<()> {
        let sync_strategy = GatewaySyncStrategy::default();
        sync_strategy.sync(self).await?;
        self.sphere_context_mut().await?.reset_access();
        Ok(())
    }
}
