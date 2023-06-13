use crate::{HasMutableSphereContext, SphereContext};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use noosphere_core::{
    authority::Author,
    data::{Did, Link, MemoIpld, Mnemonic},
};
use noosphere_storage::Storage;
use std::future::Future;
use tokio::sync::Mutex;
use ucan::crypto::KeyMaterial;

#[allow(missing_docs)]
#[cfg(not(target_arch = "wasm32"))]
pub trait SphereAuthoritySend: Send {}

#[cfg(not(target_arch = "wasm32"))]
impl<T> SphereAuthoritySend for T where T: Send {}

#[allow(missing_docs)]
#[cfg(target_arch = "wasm32")]
pub trait SphereAuthoritySend {}

#[cfg(target_arch = "wasm32")]
impl<T> SphereAuthoritySend for T {}

/// Any wrapper over [SphereContext] that implements [SphereAuthorityEscalate]
/// has the ability to do some work on the [SphereContext] using a higher-privilege
/// credential (in most cases, the root sphere credential).
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait SphereAuthorityEscalate<C, K, S>
where
    C: HasMutableSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    /// Given the recovery mnemonic for a sphere and a callback, invoke the
    /// callback with a mutable [SphereContext] wrapper. The optional return
    /// value of the callback should be the latest version of the sphere after
    /// the work of the callback is performed
    async fn with_root_authority<F, Fut>(&mut self, mnemonic: &Mnemonic, callback: F) -> Result<()>
    where
        Fut: Future<Output = Result<Option<Link<MemoIpld>>>> + SphereAuthoritySend,
        F: FnOnce(C) -> Fut + SphereAuthoritySend;
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<K, S>
    SphereAuthorityEscalate<
        Arc<Mutex<SphereContext<Arc<Box<dyn KeyMaterial>>, S>>>,
        Arc<Box<dyn KeyMaterial>>,
        S,
    > for Arc<Mutex<SphereContext<K, S>>>
where
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    async fn with_root_authority<F, Fut>(&mut self, mnemonic: &Mnemonic, callback: F) -> Result<()>
    where
        Fut: Future<Output = Result<Option<Link<MemoIpld>>>> + SphereAuthoritySend,
        F: FnOnce(Arc<Mutex<SphereContext<Arc<Box<dyn KeyMaterial>>, S>>>) -> Fut
            + SphereAuthoritySend,
    {
        let sphere_context = self.sphere_context_mut().await?;
        let root_sphere_author = Author {
            key: mnemonic.to_credential()?,
            authorization: None,
        };

        if &Did(root_sphere_author.key.get_did().await?) != sphere_context.identity() {
            return Err(anyhow!("Provided mnemonic produced an invalid credential"));
        }

        let root_sphere_context = Arc::new(Mutex::new(
            sphere_context.with_author(&root_sphere_author).await?,
        ));

        callback(root_sphere_context).await?;
        Ok(())
    }
}
