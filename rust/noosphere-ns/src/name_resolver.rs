use crate::{DhtClient, NsRecord};
use anyhow::Result;
use async_trait::async_trait;
use noosphere_core::data::Did;

#[async_trait]
pub trait NameResolver: Send + Sync {
    /// Publishes a record to the name system.
    async fn publish(&self, record: NsRecord) -> Result<()>;
    /// Retrieves a record from the name system.
    async fn resolve(&self, identity: &Did) -> Result<Option<NsRecord>>;
}

#[async_trait]
impl<T: DhtClient> NameResolver for T {
    async fn publish(&self, record: NsRecord) -> Result<()> {
        self.put_record(record, 0).await
    }

    async fn resolve(&self, identity: &Did) -> Result<Option<NsRecord>> {
        self.get_record(identity).await
    }
}

/// Helper macro for running agnostic [NameResolver] tests for
/// multiple implementations: [NameSystem] and [HttpClient].
#[cfg(test)]
#[macro_export]
macro_rules! name_resolver_tests {
    ($type:ty, $before_each:ident) => {
        #[tokio::test]
        async fn name_resolver_simple() -> Result<()> {
            let resolver = $before_each().await?;
            $crate::name_resolver::test::test_name_resolver_simple::<$type>(resolver).await
        }
    };
}

#[cfg(test)]
/// These tests are designed to run on implementations of the
/// [NameResolver] trait, both `NameSystem` and `server::HttpClient`.
pub mod test {
    use super::*;
    use cid::Cid;
    use noosphere_core::{authority::generate_ed25519_key, data::Did, tracing::initialize_tracing};
    use ucan::crypto::KeyMaterial;

    pub async fn test_name_resolver_simple<N: NameResolver>(resolver: N) -> Result<()> {
        initialize_tracing(None);
        let sphere_key = generate_ed25519_key();
        let sphere_id = Did::from(sphere_key.get_did().await?);
        let link: Cid = "bafy2bzacec4p5h37mjk2n6qi6zukwyzkruebvwdzqpdxzutu4sgoiuhqwne72"
            .parse()
            .unwrap();
        let record = NsRecord::from_issuer(&sphere_key, &sphere_id, &link, None).await?;

        resolver.publish(record).await?;
        let resolved = resolver.resolve(&sphere_id).await?.unwrap();
        assert_eq!(resolved.link().unwrap(), &link);
        Ok(())
    }
}
