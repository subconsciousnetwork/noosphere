use std::collections::BTreeMap;

use anyhow::Result;
use async_trait::async_trait;
use cid::Cid;
use futures_util::TryStreamExt;
use noosphere_core::data::{Did, Link, MemoIpld};
use noosphere_storage::{KeyValueStore, Storage};

use crate::{HasSphereContext, SphereWalker};

/// Anything that provides read access to petnames in a sphere should implement
/// [SpherePetnameRead]. A blanket implementation is provided for any container
/// that implements [HasSphereContext].
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait SpherePetnameRead<S>
where
    S: Storage + 'static,
{
    /// Get the [Did] that is assigned to a petname, if any
    async fn get_petname(&self, name: &str) -> Result<Option<Did>>;

    /// Resolve the petname via its assigned [Did] to a [Cid] that refers to a
    /// point in history of a sphere
    async fn resolve_petname(&self, name: &str) -> Result<Option<Link<MemoIpld>>>;

    /// Given a [Did], get all the petnames that have been assigned to it
    /// in this sphere
    async fn get_assigned_petnames(&self, did: &Did) -> Result<Vec<String>>;
}

fn assigned_petnames_cache_key(origin: &Did, peer: &Did, origin_version: &Cid) -> String {
    format!(
        "noosphere:cache:petname:assigned:{}:{}:{}",
        origin, peer, origin_version
    )
}

fn sphere_checkpoint_cache_key(origin: &Did, origin_version: &Cid) -> String {
    format!(
        "noosphere:cache:petname:checkpoint:{}:{}",
        origin, origin_version
    )
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<C, S> SpherePetnameRead<S> for C
where
    C: HasSphereContext<S>,
    S: Storage + 'static,
{
    #[instrument(skip(self))]
    async fn get_assigned_petnames(&self, peer: &Did) -> Result<Vec<String>> {
        let version = self.version().await?;
        let origin = self.identity().await?;

        debug!("Getting petnames assigned in {origin} at version {version}");

        let mut db = self.sphere_context().await?.db().clone();
        let key = assigned_petnames_cache_key(&origin, peer, &version);

        if let Some(names) = db.get_key::<_, Vec<String>>(key).await? {
            return Ok(names);
        }

        let checkpoint_key = sphere_checkpoint_cache_key(&origin, &version);

        if db.get_key::<_, u8>(&checkpoint_key).await?.is_some() {
            warn!("No names were assigned to {peer}",);
            return Ok(vec![]);
        }

        let walker = SphereWalker::from(self);
        let petname_stream = walker.petname_stream();
        let mut did_petnames: BTreeMap<Did, Vec<String>> = BTreeMap::new();

        tokio::pin!(petname_stream);

        while let Some((petname, identity)) = petname_stream.try_next().await? {
            match did_petnames.get_mut(&identity.did) {
                Some(petnames) => {
                    petnames.push(petname);
                }
                None => {
                    did_petnames.insert(identity.did, vec![petname]);
                }
            };
        }

        let mut assigned_petnames = None;

        for (did, petnames) in did_petnames {
            if &did == peer {
                assigned_petnames = Some(petnames.clone());
            }

            let key = assigned_petnames_cache_key(&origin, &did, &version);
            db.set_key(key, petnames).await?;
        }

        db.set_key(checkpoint_key, 1u8).await?;

        Ok(assigned_petnames.unwrap_or_default())
    }

    async fn get_petname(&self, name: &str) -> Result<Option<Did>> {
        let sphere = self.to_sphere().await?;
        let identities = sphere.get_address_book().await?.get_identities().await?;
        let address_ipld = identities.get(&name.to_string()).await?;

        Ok(address_ipld.map(|ipld| ipld.did.clone()))
    }

    async fn resolve_petname(&self, name: &str) -> Result<Option<Link<MemoIpld>>> {
        let sphere = self.to_sphere().await?;
        let identities = sphere.get_address_book().await?.get_identities().await?;
        let address_ipld = identities.get(&name.to_string()).await?;

        trace!("Recorded address for {name}: {:?}", address_ipld);

        Ok(match address_ipld {
            Some(identity) => {
                let link_record = identity
                    .link_record(self.sphere_context().await?.db())
                    .await;

                match link_record {
                    Some(link_record) => link_record.get_link(),
                    None => None,
                }
            }
            None => None,
        })
    }
}
