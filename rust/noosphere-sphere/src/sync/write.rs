use anyhow::Result;
use async_trait::async_trait;
use noosphere_core::data::{Link, MemoIpld};
use noosphere_storage::Storage;
use ucan::crypto::KeyMaterial;

use crate::{HasMutableSphereContext, SyncError, SyncRecovery};

use crate::GatewaySyncStrategy;

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
    async fn sync(&mut self, recovery: SyncRecovery) -> Result<Link<MemoIpld>, SyncError>;
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<C, K, S> SphereSync<K, S> for C
where
    C: HasMutableSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    #[instrument(level = "debug", skip(self))]
    async fn sync(&mut self, recovery: SyncRecovery) -> Result<Link<MemoIpld>, SyncError> {
        let sync_strategy = GatewaySyncStrategy::default();

        let version = match recovery {
            SyncRecovery::None => sync_strategy.sync(self).await?,
            SyncRecovery::Retry(max_retries) => {
                let mut retries = 0;
                let version;

                loop {
                    match sync_strategy.sync(self).await {
                        Ok(result) => {
                            debug!("Sync success with {retries} retries");
                            version = result;
                            break;
                        }
                        Err(SyncError::Conflict) => {
                            if retries < max_retries {
                                warn!(
                                    "Sync conflict; {} retries remaining...",
                                    max_retries - retries
                                );
                                retries += 1;
                            } else {
                                warn!("Sync conflict; no retries remaining!");
                                return Err(SyncError::Conflict);
                            }
                        }
                        Err(other) => {
                            return Err(other);
                        }
                    }
                }

                version
            }
        };

        self.sphere_context_mut().await?.reset_access();

        Ok(version)
    }
}
