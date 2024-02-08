use crate::context::{internal::SphereContextInternal, HasMutableSphereContext, HasSphereContext};

use super::SphereAuthorityRead;
use crate::{
    authority::{generate_capability, Authorization, SphereAbility},
    data::{DelegationIpld, Did, Jwt, Link, RevocationIpld},
    view::SPHERE_LIFETIME,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use cid::Cid;
use noosphere_storage::{Storage, UcanStore};
use noosphere_ucan::{builder::UcanBuilder, crypto::KeyMaterial};
use tokio_stream::StreamExt;

/// Any type which implements [SphereAuthorityWrite] is able to manipulate the
/// [AuthorityIpld] section of a sphere. This includes authorizing other keys
/// and revoking prior authorizations.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait SphereAuthorityWrite<S>: SphereAuthorityRead<S>
where
    S: Storage + 'static,
{
    /// Authorize another key by its [Did], associating the authorization with a
    /// provided display name
    async fn authorize(&mut self, name: &str, identity: &Did) -> Result<Authorization>;

    /// Revoke a previously granted authorization.
    ///
    /// Note that correctly revoking an authorization requires signing with a credential
    /// that is in the chain of authority that ultimately granted the authorization being
    /// revoked. Attempting to revoke a credential with any credential that isn't in that
    /// chain of authority will fail.
    async fn revoke_authorization(&mut self, authorization: &Authorization) -> Result<()>;

    /// Recover authority by revoking all previously delegated authorizations
    /// and creating a new one that delegates authority to the specified key
    /// (given by its [Did]).
    ///
    /// Note that correctly recovering authority requires signing with the root
    /// sphere credential, so generally can only be performed on a type that
    /// implements [SphereAuthorityEscalate]
    async fn recover_authority(&mut self, new_owner: &Did) -> Result<Authorization>;
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<C, S> SphereAuthorityWrite<S> for C
where
    C: HasSphereContext<S> + HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    // TODO(#423): We allow optional human-readable names for authorizations,
    // but this will bear the consequence of leaking personal information about
    // the user (e.g., a list of their authorized devices). We should encrypt
    // these names so that they are only readable by the user themselves.
    // TODO(#560): We should probably enforce that each [Did] only gets one
    // authorization, from a hygeine perspective; elsewhere we need to assume
    // multiple authorizations for the same [Did] are possible.
    async fn authorize(&mut self, name: &str, identity: &Did) -> Result<Authorization> {
        self.assert_write_access().await?;

        let author = self.sphere_context().await?.author().clone();
        let mut sphere = self.to_sphere().await?;
        let authorization = author.require_authorization()?;

        self.verify_authorization(authorization).await?;

        let authorization_expiry: Option<u64> = {
            let ucan = authorization
                .as_ucan(&UcanStore(sphere.store().clone()))
                .await?;
            *ucan.expires_at()
        };

        let mut builder = UcanBuilder::default()
            .issued_by(&author.key)
            .for_audience(identity)
            .claiming_capability(&generate_capability(
                &sphere.get_identity().await?,
                SphereAbility::Authorize,
            ))
            .with_nonce();

        // TODO(ucan-wg/rs-ucan#114): Clean this up when
        // `UcanBuilder::with_expiration` accepts `Option<u64>`
        if let Some(expiry) = authorization_expiry {
            builder = builder.with_expiration(expiry);
        }

        // TODO(ucan-wg/rs-ucan#32): Clean this up when we can use a CID as an authorization
        let mut signable = builder.build()?;

        signable
            .proofs
            .push(Cid::try_from(authorization)?.to_string());

        let jwt = signable.sign().await?.encode()?;

        let delegation = DelegationIpld::register(name, &jwt, sphere.store_mut()).await?;

        self.sphere_context_mut()
            .await?
            .mutation_mut()
            .delegations_mut()
            .set(&Link::new(delegation.jwt), &delegation);

        Ok(Authorization::Cid(delegation.jwt))
    }

    async fn revoke_authorization(&mut self, authorization: &Authorization) -> Result<()> {
        self.assert_write_access().await?;

        let mut sphere_context = self.sphere_context_mut().await?;

        if !sphere_context
            .author()
            .is_authorizer_of(authorization, sphere_context.db())
            .await?
        {
            let author_did = sphere_context.author().did().await?;

            return Err(anyhow!(
                "{} cannot revoke authorization {} (not a delegating authority)",
                author_did,
                authorization
            ));
        }

        let authorization_cid = Link::<Jwt>::from(Cid::try_from(authorization)?);
        let delegations = sphere_context
            .sphere()
            .await?
            .get_authority()
            .await?
            .get_delegations()
            .await?;

        if delegations.get(&authorization_cid).await?.is_none() {
            return Err(anyhow!(
                "No authority has been delegated to the authorization being revoked"
            ));
        }

        let revocation =
            RevocationIpld::revoke(&authorization_cid, &sphere_context.author().key).await?;

        sphere_context
            .mutation_mut()
            .delegations_mut()
            .remove(&authorization_cid);

        sphere_context
            .mutation_mut()
            .revocations_mut()
            .set(&authorization_cid, &revocation);

        // TODO(#424): Recursively remove any sub-delegations here (and revoke them?)

        Ok(())
    }

    async fn recover_authority(&mut self, new_owner: &Did) -> Result<Authorization> {
        self.assert_write_access().await?;

        let mut sphere_context = self.sphere_context_mut().await?;
        let author_did = Did(sphere_context.author().key.get_did().await?);
        let sphere_identity = sphere_context.identity().clone();

        if author_did != sphere_identity {
            return Err(anyhow!(
                "Only the root sphere credential can be used to recover authority"
            ));
        }

        let sphere = sphere_context.sphere().await?;
        let authority = sphere.get_authority().await?;
        let delegations = authority.get_delegations().await?;
        let delegation_stream = delegations.into_stream().await?;

        tokio::pin!(delegation_stream);

        // First: revoke all current authority
        while let Some((link, _)) = delegation_stream.try_next().await? {
            let revocation = RevocationIpld::revoke(&link, &sphere_context.author().key).await?;

            sphere_context
                .mutation_mut()
                .delegations_mut()
                .remove(&link);
            sphere_context
                .mutation_mut()
                .revocations_mut()
                .set(&link, &revocation);
        }

        // Then: bless a new owner
        let ucan = UcanBuilder::default()
            .issued_by(&sphere_context.author().key)
            .for_audience(new_owner)
            .with_lifetime(SPHERE_LIFETIME)
            .with_nonce()
            .claiming_capability(&generate_capability(
                &sphere_identity,
                SphereAbility::Authorize,
            ))
            .build()?
            .sign()
            .await?;

        let jwt = ucan.encode()?;
        let delegation = DelegationIpld::register("(OWNER)", &jwt, sphere_context.db()).await?;
        let link = Link::new(delegation.jwt);

        sphere_context
            .mutation_mut()
            .delegations_mut()
            .set(&link, &delegation);

        Ok(Authorization::Cid(link.into()))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::authority::{generate_ed25519_key, Access, Author};
    use crate::data::Did;
    use anyhow::Result;

    use noosphere_ucan::crypto::KeyMaterial;
    use tokio::sync::Mutex;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use crate::{
        context::{
            HasMutableSphereContext, HasSphereContext, SphereAuthorityRead, SphereAuthorityWrite,
            SphereContextKey,
        },
        helpers::simulated_sphere_context,
    };

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_allows_an_authorized_key_to_authorize_other_keys() -> Result<()> {
        let (mut sphere_context, _) = simulated_sphere_context(Access::ReadWrite, None).await?;

        let other_key = generate_ed25519_key();
        let other_did = Did(other_key.get_did().await?);

        let other_authorization = sphere_context.authorize("other", &other_did).await?;
        sphere_context.save(None).await?;

        assert!(sphere_context
            .verify_authorization(&other_authorization)
            .await
            .is_ok());

        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_implicitly_revokes_transitive_authorizations() -> Result<()> {
        let (mut sphere_context, mnemonic) =
            simulated_sphere_context(Access::ReadWrite, None).await?;

        let other_key: SphereContextKey = Arc::new(Box::new(generate_ed25519_key()));
        let other_did = Did(other_key.get_did().await?);

        let other_authorization = sphere_context.authorize("other", &other_did).await?;
        sphere_context.save(None).await?;

        let mut sphere_context_with_other_credential = Arc::new(Mutex::new(
            sphere_context
                .sphere_context()
                .await?
                .with_author(&Author {
                    key: other_key.clone(),
                    authorization: Some(other_authorization.clone()),
                })
                .await?,
        ));

        let third_key = generate_ed25519_key();
        let third_did = Did(third_key.get_did().await?);

        let third_authorization = sphere_context_with_other_credential
            .authorize("third", &third_did)
            .await?;
        sphere_context_with_other_credential.save(None).await?;

        let mut root_sphere_context = sphere_context.escalate_authority(&mnemonic).await?;

        root_sphere_context
            .revoke_authorization(&other_authorization)
            .await?;
        root_sphere_context.save(None).await?;

        assert!(sphere_context
            .verify_authorization(&third_authorization)
            .await
            .is_err());

        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_catches_revoked_authorizations_when_verifying() -> Result<()> {
        let (mut sphere_context, mnemonic) =
            simulated_sphere_context(Access::ReadWrite, None).await?;

        let other_key = generate_ed25519_key();
        let other_did = Did(other_key.get_did().await?);

        let other_authorization = sphere_context.authorize("other", &other_did).await?;
        sphere_context.save(None).await?;

        let mut root_sphere_context = sphere_context.escalate_authority(&mnemonic).await?;

        root_sphere_context
            .revoke_authorization(&other_authorization)
            .await?;
        root_sphere_context.save(None).await?;

        assert!(sphere_context
            .verify_authorization(&other_authorization)
            .await
            .is_err());

        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_perform_access_recovery_given_a_mnemonic() -> Result<()> {
        let (mut sphere_context, mnemonic) =
            simulated_sphere_context(Access::ReadWrite, None).await?;

        let owner = sphere_context.sphere_context().await?.author().clone();

        let other_key = generate_ed25519_key();
        let other_did = Did(other_key.get_did().await?);

        let other_authorization = sphere_context.authorize("other", &other_did).await?;
        sphere_context.save(None).await?;

        let next_owner_key = generate_ed25519_key();
        let next_owner_did = Did(next_owner_key.get_did().await?);

        let mut root_sphere_context = sphere_context.escalate_authority(&mnemonic).await?;

        root_sphere_context
            .recover_authority(&next_owner_did)
            .await?;
        root_sphere_context.save(None).await?;

        assert!(sphere_context
            .verify_authorization(&other_authorization)
            .await
            .is_err());

        assert!(sphere_context
            .verify_authorization(&owner.authorization.unwrap())
            .await
            .is_err());

        sphere_context
            .verify_authorization(
                &sphere_context
                    .get_authorization(&next_owner_did)
                    .await?
                    .unwrap(),
            )
            .await?;

        Ok(())
    }
}
