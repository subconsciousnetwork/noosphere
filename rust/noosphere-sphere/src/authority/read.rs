use std::sync::Arc;

use anyhow::{anyhow, Result};
use noosphere_core::{
    authority::{Author, Authorization},
    data::{Did, Link, Mnemonic},
    error::NoosphereError,
};
use noosphere_storage::Storage;

use tokio_stream::StreamExt;

use crate::{HasSphereContext, SphereContextKey};
use async_trait::async_trait;

/// Anything that can read the authority section from a sphere should implement
/// [SphereAuthorityRead]. A blanket implementation is provided for anything
/// that implements [HasSphereContext].
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait SphereAuthorityRead<S>
where
    S: Storage + 'static,
    Self: Sized,
{
    /// For a given [Authorization], checks that the authorization and all of its
    /// ancester proofs are valid and have not been revoked
    async fn verify_authorization(
        &self,
        authorization: &Authorization,
    ) -> Result<(), NoosphereError>;

    /// Look up an authorization by a [Did].
    async fn get_authorization(&self, did: &Did) -> Result<Option<Authorization>>;

    /// Derive a root sphere key from a mnemonic and return a version of this
    /// [SphereAuthorityRead] whose inner [SphereContext]'s [Author] is using
    /// that root sphere key.
    async fn escalate_authority(&self, mnemonic: &Mnemonic) -> Result<Self>;
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<C, S> SphereAuthorityRead<S> for C
where
    C: HasSphereContext<S>,
    S: Storage + 'static,
{
    async fn verify_authorization(
        &self,
        authorization: &Authorization,
    ) -> Result<(), NoosphereError> {
        self.to_sphere()
            .await?
            .verify_authorization(authorization)
            .await
    }

    async fn get_authorization(&self, did: &Did) -> Result<Option<Authorization>> {
        let sphere = self.to_sphere().await?;
        let authority = sphere.get_authority().await?;
        let delegations = authority.get_delegations().await?;
        let delegations_stream = delegations.into_stream().await?;

        tokio::pin!(delegations_stream);

        while let Some((Link { cid, .. }, delegation)) = delegations_stream.try_next().await? {
            let ucan = delegation.resolve_ucan(sphere.store()).await?;
            let authorized_did = ucan.audience();

            if authorized_did == did {
                return Ok(Some(Authorization::Cid(cid)));
            }
        }

        Ok(None)
    }

    async fn escalate_authority(&self, mnemonic: &Mnemonic) -> Result<Self> {
        let root_key: SphereContextKey = Arc::new(Box::new(mnemonic.to_credential()?));
        let root_author = Author {
            key: root_key,
            authorization: None,
        };

        let root_identity = root_author.did().await?;
        let sphere_identity = self.identity().await?;

        if sphere_identity != root_identity {
            return Err(anyhow!(
                "Provided mnemonic did not produce the expected credential"
            ));
        }

        Ok(Self::wrap(
            self.sphere_context()
                .await?
                .with_author(&root_author)
                .await?,
        )
        .await)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use noosphere_core::data::Did;

    use ucan::crypto::KeyMaterial;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use crate::helpers::{simulated_sphere_context, SimulationAccess};
    use crate::{HasSphereContext, SphereAuthorityRead};

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_get_an_authorization_by_did() -> Result<()> {
        let (sphere_context, _) =
            simulated_sphere_context(SimulationAccess::ReadWrite, None).await?;

        let author_did = Did(sphere_context
            .sphere_context()
            .await?
            .author()
            .key
            .get_did()
            .await?);

        let authorization = sphere_context
            .get_authorization(&author_did)
            .await?
            .unwrap();

        let _ucan = authorization
            .as_ucan(sphere_context.sphere_context().await?.db())
            .await?;

        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_verify_an_authorization_to_write_to_a_sphere() -> Result<()> {
        let (sphere_context, _) =
            simulated_sphere_context(SimulationAccess::ReadWrite, None).await?;

        let author_did = Did(sphere_context
            .sphere_context()
            .await?
            .author()
            .key
            .get_did()
            .await?);

        let authorization = sphere_context
            .get_authorization(&author_did)
            .await?
            .unwrap();

        sphere_context.verify_authorization(&authorization).await?;

        Ok(())
    }
}
