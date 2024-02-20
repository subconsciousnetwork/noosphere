use anyhow::Result;
use instant::Duration;

use async_trait::async_trait;
use noosphere_storage::Storage;
use noosphere_ucan::{builder::UcanBuilder, store::UcanJwtStore};

use crate::{
    authority::{generate_capability, SphereAbility},
    context::{internal::SphereContextInternal, HasMutableSphereContext},
    data::{Jwt, LinkRecord, LINK_RECORD_FACT_NAME},
};

/// Implementors are able to write Noosphere data pertaining to replicating
/// their spheres across the network
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait SphereReplicaWrite<S>
where
    S: Storage + 'static,
{
    /// Produce a [LinkRecord] for this sphere pointing to the latest version
    /// according to the implementor, optionally valid for the specified
    /// lifetime.
    async fn create_link_record(&mut self, lifetime: Option<Duration>) -> Result<LinkRecord>;
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<C, S> SphereReplicaWrite<S> for C
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    async fn create_link_record(&mut self, lifetime: Option<Duration>) -> Result<LinkRecord> {
        self.assert_write_access().await?;

        let mut context = self.sphere_context_mut().await?;
        let author = context.author();
        let identity = context.identity();
        let version = context.version().await?;

        let mut builder = UcanBuilder::default()
            .issued_by(&author.key)
            .for_audience(identity)
            .claiming_capability(&generate_capability(identity, SphereAbility::Publish))
            .with_fact(LINK_RECORD_FACT_NAME, version.to_string())
            .with_nonce();

        if let Some(lifetime) = lifetime {
            builder = builder.with_lifetime(lifetime.as_secs());
        }

        // An authorization may not be present or required if the issuer is the root credential,
        // which may happen in recovery scenarios where the user provides their mnemonic
        if let Some(authorization) = &author.authorization {
            builder = builder.witnessed_by(&authorization.as_ucan(context.db()).await?, None)
        }

        let link_record = LinkRecord::from(builder.build()?.sign().await?);

        let jwt = Jwt(link_record.encode()?);
        context.db_mut().write_token(&jwt).await?;

        Ok(link_record)
    }
}
