use anyhow::{anyhow, Result};
use async_trait::async_trait;
use noosphere_core::{
    authority::{SphereAction, SphereReference, SPHERE_SEMANTICS},
    data::{AddressIpld, Did, Jwt},
};
use noosphere_storage::Storage;
use ucan::{
    capability::{Capability, Resource, With},
    chain::ProofChain,
    crypto::KeyMaterial,
    Ucan,
};

use crate::{internal::SphereContextInternal, HasMutableSphereContext, SpherePetnameRead};

/// Anything that can write petnames to a sphere should implement
/// [SpherePetnameWrite]. A blanket implementation is provided for anything that
/// implements [HasMutableSphereContext]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait SpherePetnameWrite<K, S>: SpherePetnameRead<K, S>
where
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    /// Configure a petname, by assigning some [Did] to it or none. By assigning none,
    /// the petname is implicitly removed from the address space (note: this does not
    /// erase the name from historical versions of the sphere). If a name is set that
    /// already exists, the previous name shall be overwritten by the new one, and any
    /// associated [Jwt] shall be unset.
    async fn set_petname(&mut self, name: &str, identity: Option<Did>) -> Result<()>;

    /// Configure a petname, assigning some [Did] to it and setting its
    /// associated [Jwt] to a known value. The [Jwt] must be a valid UCAN that
    /// publishes a name record and grants sufficient authority from the
    /// configured [Did] to the publisher.
    async fn adopt_petname(&mut self, name: &str, record: &Jwt) -> Result<Option<Did>>;
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<H, K, S> SpherePetnameWrite<K, S> for H
where
    H: HasMutableSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    async fn set_petname(&mut self, name: &str, identity: Option<Did>) -> Result<()> {
        self.assert_write_access().await?;

        let current_address = self.get_petname(name).await?;

        if identity != current_address {
            let mut context = self.sphere_context_mut().await?;
            match identity {
                Some(identity) => {
                    context.mutation_mut().names_mut().set(
                        &name.to_string(),
                        &AddressIpld {
                            identity,
                            // TODO: We should backfill this if we have already resolved
                            // this address by another name
                            last_known_record: None,
                        },
                    );
                }
                None => context.mutation_mut().names_mut().remove(&name.to_string()),
            };
        }

        Ok(())
    }

    async fn adopt_petname(&mut self, name: &str, record: &Jwt) -> Result<Option<Did>> {
        self.assert_write_access().await?;

        let mut context = self.sphere_context_mut().await?;

        let ucan = Ucan::try_from(record.as_str())?;
        let identity = Did::from(ucan.audience());
        let db = context.db().clone();
        let chain = ProofChain::from_ucan(ucan, context.did_parser_mut(), &db).await?;

        let expected_capability = Capability {
            with: With::Resource {
                kind: Resource::Scoped(SphereReference {
                    did: identity.clone().into(),
                }),
            },
            can: SphereAction::Publish,
        };

        let capabilities = chain.reduce_capabilities(&SPHERE_SEMANTICS);
        let mut verified = false;

        for info in capabilities {
            if info.capability.enables(&expected_capability)
                && info.originators.contains(identity.as_str())
            {
                verified = true;
                break;
            }
        }

        if !verified {
            return Err(anyhow!("Record does not enable publishing to {}", identity));
        }

        debug!(
            "Adopting '{}' ({}), resolving to {}...",
            name, identity, record
        );

        // TODO: Verify that a record for an existing address is actually newer than the old one

        let new_address = AddressIpld {
            identity: identity.clone(),
            last_known_record: Some(record.clone()),
        };

        let names = self
            .sphere_context()
            .await?
            .sphere()
            .await?
            .get_names()
            .await?;
        let previous_address = names.get(&name.into()).await?;

        self.sphere_context_mut()
            .await?
            .mutation_mut()
            .names_mut()
            .set(&name.into(), &new_address);

        match previous_address {
            Some(previous_address) => {
                if identity != previous_address.identity {
                    return Ok(Some(previous_address.identity.to_owned()));
                }
            }
            _ => (),
        };

        Ok(None)
    }
}
