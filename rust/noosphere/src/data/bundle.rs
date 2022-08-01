use std::{collections::BTreeMap, str::FromStr};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use cid::Cid;

use futures::StreamExt;
use noosphere_cbor::{TryDagCbor, TryDagCborSendSync};
use noosphere_storage::interface::{DagCborStore, Store};
use serde::{Deserialize, Serialize};

use crate::{
    data::{
        BodyChunkIpld, ChangelogIpld, ContentType, Header, LinksIpld, MapOperation, MemoIpld,
        SphereIpld, VersionedMapIpld,
    },
    view::Timeslice,
};

// TODO: This should maybe only collect CIDs, and then streaming-serialize to
// a CAR (https://ipld.io/specs/transport/car/carv2/)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Bundle(BTreeMap<Cid, Vec<u8>>);

impl Bundle {
    pub async fn load_into<Storage: Store>(&self, store: &mut Storage) -> Result<()> {
        // TODO: Parrallelize this
        for (cid, cbor_bytes) in self.0.iter() {
            let stored_cid = store.write_cbor(cbor_bytes).await?;
            if cid != &stored_cid {
                return Err(anyhow!(
                    "CID in bundle ({:?}) did not match CID as stored ({:?})",
                    cid,
                    stored_cid
                ));
            }
        }

        Ok(())
    }

    pub async fn try_from_timeslice<'a, Storage: Store>(
        timeslice: &Timeslice<'a, Storage>,
        store: &Storage,
    ) -> Result<Bundle> {
        let mut stream = Box::pin(timeslice.try_stream());
        let mut bundle = Bundle::default();

        while let Some(ancestor) = stream.next().await {
            let (_, memo) = ancestor?;
            memo.try_extend_bundle(&mut bundle, store).await?;
        }

        Ok(bundle)
    }

    pub fn add(&mut self, cid: Cid, bytes: Vec<u8>) -> bool {
        match self.0.contains_key(&cid) {
            true => false,
            false => {
                self.0.insert(cid, bytes);
                true
            }
        }
    }

    pub fn merge(&mut self, mut other: Bundle) {
        self.0.append(&mut other.0);
    }

    pub fn map(&self) -> &BTreeMap<Cid, Vec<u8>> {
        &self.0
    }

    pub async fn extend<CanBundle: TryBundle, Storage: Store>(
        &mut self,
        cid: &Cid,
        store: &Storage,
    ) -> Result<()> {
        CanBundle::try_extend_bundle_with_cid(cid, self, store).await?;
        Ok(())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait TryBundle: TryDagCborSendSync {
    async fn try_extend_bundle<Storage: Store>(
        &self,
        bundle: &mut Bundle,
        store: &Storage,
    ) -> Result<()> {
        let self_bytes = self.try_into_dag_cbor()?;
        let self_cid = Storage::make_cid(&self_bytes);
        Self::try_extend_bundle_with_cid(&self_cid, bundle, store).await
    }

    async fn try_extend_bundle_with_cid<Storage: Store>(
        cid: &Cid,
        bundle: &mut Bundle,
        store: &Storage,
    ) -> Result<()> {
        bundle.add(cid.clone(), store.require_cbor(cid).await?);
        Ok(())
    }

    async fn try_bundle<Storage: Store>(&self, store: &Storage) -> Result<Bundle> {
        let mut bundle = Bundle::default();
        self.try_extend_bundle(&mut bundle, store).await?;
        Ok(bundle)
    }

    async fn try_bundle_with_cid<Storage: Store>(cid: &Cid, store: &Storage) -> Result<Bundle> {
        let mut bundle = Bundle::default();
        Self::try_extend_bundle_with_cid(cid, &mut bundle, store).await?;
        Ok(bundle)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl TryBundle for BodyChunkIpld {
    async fn try_extend_bundle_with_cid<Storage: Store>(
        cid: &Cid,
        bundle: &mut Bundle,
        store: &Storage,
    ) -> Result<()> {
        let mut next_cid = Some(cid.clone());

        while let Some(cid) = next_cid {
            let bytes = store.require_cbor(&cid).await?;
            let chunk = BodyChunkIpld::try_from_dag_cbor(&bytes)?;
            bundle.add(cid, bytes);
            next_cid = chunk.next;
        }

        Ok(())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl TryBundle for ChangelogIpld<MapOperation<String, Cid>>
where
    Self: TryDagCborSendSync,
{
    async fn try_extend_bundle_with_cid<Storage: Store>(
        cid: &Cid,
        bundle: &mut Bundle,
        store: &Storage,
    ) -> Result<()> {
        let bytes = store.require_cbor(cid).await?;
        let changelog = Self::try_from_dag_cbor(&bytes)?;

        bundle.add(cid.clone(), bytes);

        for op in changelog.changes {
            match op {
                MapOperation::Add { value: cid, .. } => {
                    let bytes = store.require_cbor(&cid).await?;
                    bundle.add(cid, bytes);
                }
                _ => (),
            };
        }

        Ok(())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl TryBundle for MemoIpld {
    async fn try_extend_bundle<Storage: Store>(
        &self,
        bundle: &mut Bundle,
        store: &Storage,
    ) -> Result<()> {
        let self_bytes = self.try_into_dag_cbor()?;
        let self_cid = Storage::make_cid(&self_bytes);

        bundle.add(self_cid, self_bytes);

        match self.get_first_header(&Header::ContentType.to_string()) {
            Some(value) => match ContentType::from_str(&value)? {
                ContentType::Subtext | ContentType::Bytes => {
                    bundle.extend::<BodyChunkIpld, _>(&self.body, store).await?;
                }
                ContentType::Sphere => {
                    bundle.extend::<SphereIpld, _>(&self.body, store).await?;
                }
                ContentType::Unknown(_) => todo!(),
            },
            None => todo!(),
        }

        Ok(())
    }

    async fn try_extend_bundle_with_cid<Storage: Store>(
        cid: &Cid,
        bundle: &mut Bundle,
        store: &Storage,
    ) -> Result<()> {
        store
            .load::<MemoIpld>(cid)
            .await?
            .try_extend_bundle(bundle, store)
            .await?;
        Ok(())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl TryBundle for VersionedMapIpld<String, Cid>
where
    Self: TryDagCborSendSync,
{
    async fn try_extend_bundle<Storage: Store>(
        &self,
        bundle: &mut Bundle,
        store: &Storage,
    ) -> Result<()> {
        let self_bytes = self.try_into_dag_cbor()?;
        let self_cid = Storage::make_cid(&self_bytes);

        ChangelogIpld::<MapOperation<String, Cid>>::try_extend_bundle_with_cid(
            &self.changelog,
            bundle,
            store,
        )
        .await?;

        bundle.add(self_cid, self_bytes);

        Ok(())
    }

    async fn try_extend_bundle_with_cid<Storage: Store>(
        cid: &Cid,
        bundle: &mut Bundle,
        store: &Storage,
    ) -> Result<()> {
        let map: Self = store.load(cid).await?;
        map.try_extend_bundle(bundle, store).await?;
        Ok(())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl TryBundle for SphereIpld {
    async fn try_extend_bundle_with_cid<Storage: Store>(
        cid: &Cid,
        bundle: &mut Bundle,
        store: &Storage,
    ) -> Result<()> {
        let self_bytes = store.require_cbor(cid).await?;
        let sphere = SphereIpld::try_from_dag_cbor(&self_bytes)?;

        bundle.add(cid.clone(), self_bytes);

        match sphere.links {
            Some(cid) => {
                LinksIpld::try_extend_bundle_with_cid(&cid, bundle, store).await?;
            }
            _ => (),
        }

        match sphere.revocations {
            Some(_cid) => {
                todo!();
            }
            _ => (),
        }

        match sphere.sealed {
            Some(_cid) => {
                todo!();
            }
            _ => (),
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use noosphere_storage::{interface::DagCborStore, memory::MemoryStore};
    use ucan::crypto::KeyMaterial;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    use crate::{
        authority::generate_ed25519_key,
        data::{Bundle, LinksIpld, MemoIpld, TryBundle},
        view::{Sphere, SphereMutation, Timeline},
    };

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_bundles_an_empty_sphere() {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (sphere, _, _) = Sphere::try_generate(&owner_did, &mut store).await.unwrap();
        let bundle = MemoIpld::try_bundle_with_cid(sphere.cid(), &store)
            .await
            .unwrap();

        assert!(bundle.map().contains_key(sphere.cid()));

        let memo = sphere.try_as_memo().await.unwrap();

        assert!(bundle.map().contains_key(&memo.body));
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_bundles_a_sphere_with_links() {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (sphere, ucan, _) = Sphere::try_generate(&owner_did, &mut store).await.unwrap();

        let foo_cid = store.write_cbor(b"foo").await.unwrap();
        let mut mutation = SphereMutation::new(&owner_did);
        mutation.links_mut().set("foo", &foo_cid);

        let mut revision = sphere.try_apply(&mutation).await.unwrap();
        let new_cid = revision.try_sign(&owner_key, Some(&ucan)).await.unwrap();

        let bundle = MemoIpld::try_bundle_with_cid(&new_cid, &store)
            .await
            .unwrap();

        assert_eq!(bundle.map().keys().len(), 5);

        let sphere = Sphere::at(&new_cid, &store);

        assert!(bundle.map().contains_key(sphere.cid()));

        let memo = sphere.try_as_memo().await.unwrap();

        assert!(bundle.map().contains_key(&memo.body));

        let sphere_ipld = sphere.try_as_body().await.unwrap();
        let links_cid = sphere_ipld.links.unwrap();

        assert!(bundle.map().contains_key(&links_cid));

        let links_ipld = store.load::<LinksIpld>(&links_cid).await.unwrap();

        assert!(bundle.map().contains_key(&links_ipld.changelog));
        assert!(bundle.map().contains_key(&foo_cid));
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_only_bundles_the_revision_delta() {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (sphere, ucan, _) = Sphere::try_generate(&owner_did, &mut store).await.unwrap();

        let foo_cid = store.write_cbor(b"foo").await.unwrap();
        let mut first_mutation = SphereMutation::new(&owner_did);
        first_mutation.links_mut().set("foo", &foo_cid);

        let mut revision = sphere.try_apply(&first_mutation).await.unwrap();
        let new_cid = revision.try_sign(&owner_key, Some(&ucan)).await.unwrap();

        let sphere = Sphere::at(&new_cid, &store);

        let bar_cid = store.write_cbor(b"bar").await.unwrap();
        let mut second_mutation = SphereMutation::new(&owner_did);
        second_mutation.links_mut().set("bar", &bar_cid);

        let mut revision = sphere.try_apply(&second_mutation).await.unwrap();
        let new_cid = revision.try_sign(&owner_key, Some(&ucan)).await.unwrap();

        let bundle = MemoIpld::try_bundle_with_cid(&new_cid, &store)
            .await
            .unwrap();

        assert_eq!(bundle.map().keys().len(), 5);
        assert!(!bundle.map().contains_key(&foo_cid));
        assert!(bundle.map().contains_key(&bar_cid));
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_bundles_all_revisions_in_a_timeslice() {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (sphere, ucan, _) = Sphere::try_generate(&owner_did, &mut store).await.unwrap();

        let original_cid = sphere.cid().clone();

        let foo_cid = store.write_cbor(b"foo").await.unwrap();
        let mut first_mutation = SphereMutation::new(&owner_did);
        first_mutation.links_mut().set("foo", &foo_cid);

        let mut revision = sphere.try_apply(&first_mutation).await.unwrap();
        let second_cid = revision.try_sign(&owner_key, Some(&ucan)).await.unwrap();

        let sphere = Sphere::at(&second_cid, &store);

        let bar_cid = store.write_cbor(b"bar").await.unwrap();
        let mut second_mutation = SphereMutation::new(&owner_did);
        second_mutation.links_mut().set("bar", &bar_cid);

        let mut revision = sphere.try_apply(&second_mutation).await.unwrap();
        let final_cid = revision.try_sign(&owner_key, Some(&ucan)).await.unwrap();

        let timeline = Timeline::new(&store);

        let bundle = Bundle::try_from_timeslice(&timeline.slice(&final_cid, &second_cid), &store)
            .await
            .unwrap();

        assert_eq!(bundle.map().keys().len(), 10);

        assert!(bundle.map().contains_key(&foo_cid));
        assert!(bundle.map().contains_key(&bar_cid));
        assert!(bundle.map().contains_key(&final_cid));
        assert!(bundle.map().contains_key(&second_cid));
        assert!(!bundle.map().contains_key(&original_cid));
    }
}
