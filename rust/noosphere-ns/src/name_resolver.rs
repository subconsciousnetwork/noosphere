use crate::DhtClient;
use anyhow::Result;
use async_trait::async_trait;
use noosphere_core::data::{Did, LinkRecord};

#[async_trait]
pub trait NameResolver: Send + Sync {
    /// Publishes a record to the name system.
    async fn publish(&self, record: LinkRecord) -> Result<()>;
    /// Retrieves a record from the name system.
    async fn resolve(&self, identity: &Did) -> Result<Option<LinkRecord>>;
}

#[async_trait]
impl<T: DhtClient> NameResolver for T {
    async fn publish(&self, record: LinkRecord) -> Result<()> {
        self.put_record(record, 0).await
    }

    async fn resolve(&self, identity: &Did) -> Result<Option<LinkRecord>> {
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
    use noosphere_core::{
        authority::{generate_capability, generate_ed25519_key, SphereAbility},
        data::{Did, LINK_RECORD_FACT_NAME},
        tracing::initialize_tracing,
        view::SPHERE_LIFETIME,
    };
    use ucan::{builder::UcanBuilder, crypto::KeyMaterial};

    pub async fn test_name_resolver_simple<N: NameResolver>(resolver: N) -> Result<()> {
        initialize_tracing(None);
        let sphere_key = generate_ed25519_key();
        let sphere_identity = Did::from(sphere_key.get_did().await?);
        let link: Cid = "bafyr4iagi6t6khdrtbhmyjpjgvdlwv6pzylxhuhstxhkdp52rju7er325i"
            .parse()
            .unwrap();
        let ucan = UcanBuilder::default()
            .issued_by(&sphere_key)
            .for_audience(&sphere_identity)
            .claiming_capability(&generate_capability(
                &sphere_identity,
                SphereAbility::Publish,
            ))
            .with_fact(LINK_RECORD_FACT_NAME, link.to_string())
            .with_lifetime(SPHERE_LIFETIME)
            .build()?
            .sign()
            .await?;
        let record = LinkRecord::try_from(ucan)?;

        resolver.publish(record).await?;
        let resolved = resolver.resolve(&sphere_identity).await?.unwrap();
        assert_eq!(resolved.get_link().unwrap(), link.into());
        Ok(())
    }
}
