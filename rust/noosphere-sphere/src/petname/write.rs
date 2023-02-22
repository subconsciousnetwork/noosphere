use anyhow::Result;
use async_trait::async_trait;
use noosphere_core::data::{AddressIpld, Did, Jwt};
use noosphere_storage::Storage;
use ucan::crypto::KeyMaterial;

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
    async fn set_petname(&mut self, name: &str, address: Option<Did>) -> Result<()>;

    /// Configure a petname, assigning some [Did] to it and setting its
    /// associated [Jwt] to a known value. The [Jwt] must be a valid UCAN that
    /// publishes a name record and grants sufficient authority from the
    /// configured [Did] to the publisher.
    async fn adopt_petname(
        &mut self,
        name: &str,
        address: &Did,
        record: &Jwt,
    ) -> Result<Option<Did>>;
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<H, K, S> SpherePetnameWrite<K, S> for H
where
    H: HasMutableSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    async fn set_petname(&mut self, name: &str, address: Option<Did>) -> Result<()> {
        self.assert_write_access().await?;

        let current_address = self.get_petname(name).await?;

        if address != current_address {
            let mut context = self.sphere_context_mut().await?;
            match address {
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

    async fn adopt_petname(
        &mut self,
        _name: &str,
        _address: &Did,
        _record: &Jwt,
    ) -> Result<Option<Did>> {
        todo!();
    }
}
