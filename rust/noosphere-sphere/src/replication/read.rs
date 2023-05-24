use anyhow::Result;
use async_trait::async_trait;
use noosphere_storage::Storage;
use ucan::crypto::KeyMaterial;

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait SphereReplicaRead<K, S>: Sized
where
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    async fn traverse_by_petnames(&self, petnames: &[String]) -> Result<Option<Self>>;
}
