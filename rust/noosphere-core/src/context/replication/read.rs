use anyhow::Result;
use async_trait::async_trait;
use noosphere_storage::Storage;

/// Implementors are able to traverse from one sphere to the next via
/// the address book entries found in those spheres
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait SphereReplicaRead<S>: Sized
where
    S: Storage + 'static,
{
    /// Accepts a linear sequence of petnames and attempts to recursively
    /// traverse through spheres using that sequence. The sequence is traversed
    /// from back to front. So, if the sequence is "gold", "cat", "bob", it will
    /// traverse to bob, then to bob's cat, then to bob's cat's gold.
    async fn traverse_by_petnames(&self, petnames: &[String]) -> Result<Option<Self>>;
}
