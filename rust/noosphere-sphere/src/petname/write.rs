use anyhow::{anyhow, Result};
use async_trait::async_trait;
use noosphere_core::data::{Did, IdentityIpld, LinkRecord};
use noosphere_storage::Storage;
use ucan::{crypto::KeyMaterial, store::UcanJwtStore};

use crate::{internal::SphereContextInternal, HasMutableSphereContext, SpherePetnameRead};

fn validate_petname(petname: &str) -> Result<()> {
    if petname.is_empty() {
        Err(anyhow!("Petname must not be empty."))
    } else {
        Ok(())
    }
}

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
    /// Configure a petname, by assigning some [Did] to it or none. By assigning
    /// none, the petname is implicitly removed from the address space (note:
    /// this does not erase the name from historical versions of the sphere). If
    /// a name is set that already exists, the previous name shall be
    /// overwritten by the new one, and any associated [Jwt] shall be unset.
    async fn set_petname(&mut self, name: &str, identity: Option<Did>) -> Result<()>;

    /// Set the [LinkRecord] associated with a petname.  The [LinkRecord] must
    /// resolve a valid UCAN that authorizes the corresponding sphere to be
    /// published and grants sufficient authority from the configured [Did] to
    /// the publisher. The audience of the UCAN must match the [Did] that was
    /// most recently assigned the associated petname. Note that a petname
    /// _must_ be assigned to the audience [Did] in order for the record to be
    /// set.
    async fn set_petname_record(&mut self, name: &str, record: &LinkRecord) -> Result<Option<Did>>;

    #[deprecated(note = "Use set_petname_record instead")]
    async fn adopt_petname(&mut self, name: &str, record: &LinkRecord) -> Result<Option<Did>>;
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<C, K, S> SpherePetnameWrite<K, S> for C
where
    C: HasMutableSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    async fn set_petname(&mut self, name: &str, identity: Option<Did>) -> Result<()> {
        self.assert_write_access().await?;
        validate_petname(name)?;

        let current_address = self.get_petname(name).await?;

        if identity != current_address {
            let mut context = self.sphere_context_mut().await?;
            match identity {
                Some(identity) => {
                    context.mutation_mut().identities_mut().set(
                        &name.to_string(),
                        &IdentityIpld {
                            did: identity,
                            // TODO: We should backfill this if we have already resolved
                            // this address by another name
                            link_record: None,
                        },
                    );
                }
                None => context
                    .mutation_mut()
                    .identities_mut()
                    .remove(&name.to_string()),
            };
        }

        Ok(())
    }

    async fn adopt_petname(&mut self, name: &str, record: &LinkRecord) -> Result<Option<Did>> {
        self.set_petname_record(name, record).await
    }

    async fn set_petname_record(&mut self, name: &str, record: &LinkRecord) -> Result<Option<Did>> {
        // NOTE: it is not safe for us to blindly adopt link records that don't
        // match up with the petname we are adopting them against. For example,
        // consider the following sequence of events:
        //
        //  1. A petname is assigned to a DID
        //  2. During sync, the gateway kicks off a parallel job to resolve the
        //     petname
        //  3. Meanwhile, we unassign the petname
        //  4. We sync, the gateway takes no action (no new names to resolve)
        //  5. Then, the original resolve job finishes and comes back with a
        //     record
        //
        // Record adoption is not able to disambiguate between between a new
        // record being added and a race condition like the one described above.
        self.assert_write_access().await?;
        validate_petname(name)?;

        let identity = record.to_sphere_identity();
        let expected_identity = self.get_petname(name).await?;

        match expected_identity {
            Some(expected_identity) => {
                if expected_identity != identity {
                    return Err(anyhow!(
                        "Cannot adopt petname record for '{}'; expected record for {} but got record for {}",
                        name,
                        expected_identity,
                        identity
                    ));
                }
            }
            None => {
                return Err(anyhow!(
                    "Cannot adopt petname record for '{}' (not assigned to a sphere identity)",
                    name
                ));
            }
        };

        let cid = self
            .sphere_context_mut()
            .await?
            .db_mut()
            .write_token(&record.encode()?)
            .await?;

        // TODO: Verify that a record for an existing address is actually newer than the old one
        // TODO: Validate the record as a UCAN

        debug!(
            "Adopting '{}' ({}), resolving to {}...",
            name, identity, record
        );

        let new_address = IdentityIpld {
            did: identity.clone(),
            link_record: Some(cid.into()),
        };

        let identities = self
            .sphere_context()
            .await?
            .sphere()
            .await?
            .get_address_book()
            .await?
            .get_identities()
            .await?;
        let previous_identity = identities.get(&name.into()).await?;

        self.sphere_context_mut()
            .await?
            .mutation_mut()
            .identities_mut()
            .set(&name.into(), &new_address);

        if let Some(previous_identity) = previous_identity {
            if identity != previous_identity.did {
                return Ok(Some(previous_identity.did.to_owned()));
            }
        }

        Ok(None)
    }
}
