use crate::{
    authority::Authorization,
    context::{
        internal::SphereContextInternal, GatewaySyncStrategy, HasMutableSphereContext,
        HasSphereContext, SyncError, SyncExtent, SyncRecovery,
    },
    data::{Link, MemoIpld},
};
use anyhow::Result;
use async_trait::async_trait;
use noosphere_storage::Storage;

/// Implementors of [SphereSync] are able to sychronize with a Noosphere gateway
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait SphereSync<S>
where
    S: Storage + 'static,
{
    /// If a gateway URL has been configured, attempt to synchronize local
    /// sphere data with the gateway. Changes on the gateway will first be
    /// fetched to local storage. Then, the local changes will be replayed on
    /// top of those changes. Finally, the synchronized local history will be
    /// pushed up to the gateway.
    ///
    /// The returned [Link] is the latest version of the local
    /// sphere lineage after the sync has completed.
    async fn sync(&mut self) -> Result<Link<MemoIpld>, SyncError> {
        self.sync_with_options(SyncExtent::FetchAndPush, SyncRecovery::Retry(3))
            .await
    }

    /// Same as [SphereSync::sync], except it lets you customize the
    /// [SyncExtent] and [SyncRecovery] properties of the synchronization
    /// routine.
    async fn sync_with_options(
        &mut self,
        extent: SyncExtent,
        recovery: SyncRecovery,
    ) -> Result<Link<MemoIpld>, SyncError>;
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<C, S> SphereSync<S> for C
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    #[instrument(level = "debug", skip(self))]
    async fn sync_with_options(
        &mut self,
        extent: SyncExtent,
        recovery: SyncRecovery,
    ) -> Result<Link<MemoIpld>, SyncError> {
        debug!("Attempting to sync...");

        // Check that the author has write access to sync.
        // If a sphere was joined from another sphere, do not check,
        // but allow sync to proceed, as the local sphere does not have
        // local proof until after initial sync. If truly no write access is
        // available, the gateway will reject this sync.
        if !is_sphere_joined(self).await {
            self.assert_write_access()
                .await
                .map_err(|_| SyncError::InsufficientPermission)?;
        }

        let sync_strategy = GatewaySyncStrategy::default();

        let version = match recovery {
            SyncRecovery::None => sync_strategy.sync(self, extent).await?,
            SyncRecovery::Retry(max_retries) => {
                let mut retries = 0;
                let version;

                loop {
                    match sync_strategy.sync(self, extent).await {
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

/// Given a `HasSphereContext<S>`, return a boolean indicating
/// whether or not this sphere has been joined from another sphere
/// (e.g. possibly lacking local authorization until syncing with a gateway).
async fn is_sphere_joined<C, S>(context: &C) -> bool
where
    C: HasSphereContext<S>,
    S: Storage + 'static,
{
    let context = {
        let context = context.sphere_context().await;
        if context.is_err() {
            return false;
        }
        context.unwrap()
    };

    let author = context.author();

    let auth = {
        let auth = author.require_authorization();
        if auth.is_err() {
            return false;
        }
        auth.unwrap()
    };
    matches!(auth, Authorization::Cid(_))
}
