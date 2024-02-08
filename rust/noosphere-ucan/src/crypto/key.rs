use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
pub trait KeyMaterialConditionalSendSync: Send + Sync {}

#[cfg(not(target_arch = "wasm32"))]
impl<K> KeyMaterialConditionalSendSync for K where K: KeyMaterial + Send + Sync {}

#[cfg(target_arch = "wasm32")]
pub trait KeyMaterialConditionalSendSync {}

#[cfg(target_arch = "wasm32")]
impl<K> KeyMaterialConditionalSendSync for K where K: KeyMaterial {}

/// This trait must be implemented by a struct that encapsulates cryptographic
/// keypair data. The trait represent the minimum required API capability for
/// producing a signed UCAN from a cryptographic keypair, and verifying such
/// signatures.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait KeyMaterial: KeyMaterialConditionalSendSync {
    /// The algorithm that will be used to produce the signature returned by the
    /// sign method in this implementation
    fn get_jwt_algorithm_name(&self) -> String;

    /// Provides a valid DID that can be used to solve the key
    async fn get_did(&self) -> Result<String>;

    /// Sign some data with this key
    async fn sign(&self, payload: &[u8]) -> Result<Vec<u8>>;

    /// Verify the alleged signature of some data against this key
    async fn verify(&self, payload: &[u8], signature: &[u8]) -> Result<()>;
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl KeyMaterial for Box<dyn KeyMaterial> {
    fn get_jwt_algorithm_name(&self) -> String {
        self.as_ref().get_jwt_algorithm_name()
    }

    async fn get_did(&self) -> Result<String> {
        self.as_ref().get_did().await
    }

    async fn sign(&self, payload: &[u8]) -> Result<Vec<u8>> {
        self.as_ref().sign(payload).await
    }

    async fn verify(&self, payload: &[u8], signature: &[u8]) -> Result<()> {
        self.as_ref().verify(payload, signature).await
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<K> KeyMaterial for Arc<K>
where
    K: KeyMaterial,
{
    fn get_jwt_algorithm_name(&self) -> String {
        (**self).get_jwt_algorithm_name()
    }

    async fn get_did(&self) -> Result<String> {
        (**self).get_did().await
    }

    async fn sign(&self, payload: &[u8]) -> Result<Vec<u8>> {
        (**self).sign(payload).await
    }

    async fn verify(&self, payload: &[u8], signature: &[u8]) -> Result<()> {
        (**self).verify(payload, signature).await
    }
}
