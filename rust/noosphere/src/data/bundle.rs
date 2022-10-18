use std::{collections::BTreeMap, str::FromStr};

use anyhow::Result;
use async_trait::async_trait;
use cid::Cid;

use futures::{pin_mut, StreamExt};
use libipld_cbor::DagCborCodec;
use libipld_core::{raw::RawCodec, serde::to_ipld};
use noosphere_cbor::{TryDagCbor, TryDagCborSendSync};
use noosphere_storage::{
    encoding::{block_deserialize, block_serialize},
    interface::BlockStore,
};
use serde::{Deserialize, Serialize};

use crate::{
    data::{
        BodyChunkIpld, ChangelogIpld, ContentType, Header, LinksIpld, MapOperation, MemoIpld,
        SphereIpld, VersionedMapIpld,
    },
    view::Timeslice,
};

use super::{AllowedIpld, AuthorizationIpld, RevokedIpld, VersionedMapKey, VersionedMapValue};

// TODO: This should maybe only collect CIDs, and then streaming-serialize to
// a CAR (https://ipld.io/specs/transport/car/carv2/)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Bundle(BTreeMap<String, Vec<u8>>);

impl Bundle {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn contains(&self, cid: &Cid) -> bool {
        self.0.contains_key(&cid.to_string())
    }

    pub async fn load_into<S: BlockStore>(&self, store: &mut S) -> Result<()> {
        // TODO: Parrallelize this
        for (cid_string, block_bytes) in self.0.iter() {
            let cid = Cid::from_str(cid_string)?;

            store.put_block(&cid, block_bytes).await?;

            match cid.codec() {
                codec_id if codec_id == u64::from(DagCborCodec) => {
                    store.put_links::<DagCborCodec>(&cid, &block_bytes).await?;
                }
                codec_id if codec_id == u64::from(RawCodec) => {
                    store.put_links::<RawCodec>(&cid, &block_bytes).await?;
                }
                codec_id => warn!("Unrecognized codec {}; skipping...", codec_id),
            }

            // TODO: Verify CID is correct, maybe?
        }

        Ok(())
    }

    pub async fn try_from_timeslice<'a, S: BlockStore>(
        timeslice: &Timeslice<'a, S>,
        store: &S,
    ) -> Result<Bundle> {
        let stream = timeslice.try_stream();
        let mut bundle = Bundle::default();

        pin_mut!(stream);

        while let Some(ancestor) = stream.next().await {
            let (_, memo) = ancestor?;
            memo.try_extend_bundle(&mut bundle, store).await?;
        }

        Ok(bundle)
    }

    pub fn add(&mut self, cid: Cid, bytes: Vec<u8>) -> bool {
        let cid_string = cid.to_string();
        match self.0.contains_key(&cid_string) {
            true => false,
            false => {
                self.0.insert(cid_string, bytes);
                true
            }
        }
    }

    pub fn merge(&mut self, mut other: Bundle) {
        self.0.append(&mut other.0);
    }

    pub fn map(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.0
    }

    pub async fn extend<CanBundle: TryBundle, S: BlockStore>(
        &mut self,
        cid: &Cid,
        store: &S,
    ) -> Result<()> {
        CanBundle::try_extend_bundle_with_cid(cid, self, store).await?;
        Ok(())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait TryBundle: TryDagCborSendSync {
    async fn try_extend_bundle<S: BlockStore>(
        &self,
        bundle: &mut Bundle,
        _store: &S,
    ) -> Result<()> {
        let (self_cid, self_bytes) = block_serialize::<DagCborCodec, _>(self)?;
        bundle.add(self_cid, self_bytes);
        Ok(())
    }

    async fn try_extend_bundle_with_cid<S: BlockStore>(
        cid: &Cid,
        bundle: &mut Bundle,
        store: &S,
    ) -> Result<()> {
        let item = store.load::<DagCborCodec, Self>(cid).await?;
        item.try_extend_bundle(bundle, store).await?;

        Ok(())
    }

    async fn try_bundle<S: BlockStore>(&self, store: &S) -> Result<Bundle> {
        let mut bundle = Bundle::default();
        self.try_extend_bundle(&mut bundle, store).await?;
        Ok(bundle)
    }

    async fn try_bundle_with_cid<S: BlockStore>(cid: &Cid, store: &S) -> Result<Bundle> {
        let mut bundle = Bundle::default();
        Self::try_extend_bundle_with_cid(cid, &mut bundle, store).await?;
        Ok(bundle)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl TryBundle for BodyChunkIpld {
    async fn try_extend_bundle_with_cid<S: BlockStore>(
        cid: &Cid,
        bundle: &mut Bundle,
        store: &S,
    ) -> Result<()> {
        let mut next_cid = Some(*cid);

        while let Some(cid) = next_cid {
            let bytes = store.require_block(&cid).await?;
            let chunk = BodyChunkIpld::try_from_dag_cbor(&bytes)?;
            bundle.add(cid, bytes);
            next_cid = chunk.next;
        }

        Ok(())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<K, V> TryBundle for ChangelogIpld<MapOperation<K, V>>
where
    K: VersionedMapKey,
    V: VersionedMapValue,
{
    async fn try_extend_bundle_with_cid<S: BlockStore>(
        cid: &Cid,
        bundle: &mut Bundle,
        store: &S,
    ) -> Result<()> {
        let bytes = store.require_block(cid).await?;
        let mut cids = Vec::new();
        let changelog = Self::try_from_dag_cbor(&bytes)?;

        bundle.add(*cid, bytes);

        for op in changelog.changes {
            match op {
                MapOperation::Add { .. } => to_ipld(&op)?.references(&mut cids),
                _ => (),
            };
        }

        for cid in cids {
            bundle.add(cid, store.require_block(&cid).await?);
        }

        Ok(())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl TryBundle for MemoIpld {
    async fn try_extend_bundle<S: BlockStore>(&self, bundle: &mut Bundle, store: &S) -> Result<()> {
        let (self_cid, self_bytes) = block_serialize::<DagCborCodec, _>(self)?;

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

    async fn try_extend_bundle_with_cid<S: BlockStore>(
        cid: &Cid,
        bundle: &mut Bundle,
        store: &S,
    ) -> Result<()> {
        let memo = store.load::<DagCborCodec, MemoIpld>(cid).await?;
        println!("BUNDLING MEMO {} {:?}", cid, memo);
        store
            .load::<DagCborCodec, MemoIpld>(cid)
            .await?
            .try_extend_bundle(bundle, store)
            .await?;
        Ok(())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<K, V> TryBundle for VersionedMapIpld<K, V>
where
    K: VersionedMapKey,
    V: VersionedMapValue,
{
    async fn try_extend_bundle<S: BlockStore>(&self, bundle: &mut Bundle, store: &S) -> Result<()> {
        let (self_cid, self_bytes) = block_serialize::<DagCborCodec, _>(self)?;

        ChangelogIpld::<MapOperation<K, V>>::try_extend_bundle_with_cid(
            &self.changelog,
            bundle,
            store,
        )
        .await?;

        bundle.add(self_cid, self_bytes);

        Ok(())
    }

    async fn try_extend_bundle_with_cid<S: BlockStore>(
        cid: &Cid,
        bundle: &mut Bundle,
        store: &S,
    ) -> Result<()> {
        let map: Self = store.load::<DagCborCodec, _>(cid).await?;
        map.try_extend_bundle(bundle, store).await?;
        Ok(())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl TryBundle for SphereIpld {
    async fn try_extend_bundle_with_cid<S: BlockStore>(
        cid: &Cid,
        bundle: &mut Bundle,
        store: &S,
    ) -> Result<()> {
        let self_bytes = store.require_block(cid).await?;
        let sphere = SphereIpld::try_from_dag_cbor(&self_bytes)?;

        bundle.add(*cid, self_bytes);

        match sphere.links {
            Some(cid) => {
                LinksIpld::try_extend_bundle_with_cid(&cid, bundle, store).await?;
            }
            _ => (),
        }

        match sphere.authorization {
            Some(cid) => {
                AuthorizationIpld::try_extend_bundle_with_cid(&cid, bundle, store).await?;
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

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl TryBundle for AuthorizationIpld {
    async fn try_extend_bundle_with_cid<S: BlockStore>(
        cid: &Cid,
        bundle: &mut Bundle,
        store: &S,
    ) -> Result<()> {
        let self_bytes = store.require_block(cid).await?;
        let authorization_ipld = block_deserialize::<DagCborCodec, AuthorizationIpld>(&self_bytes)?;

        AllowedIpld::try_extend_bundle_with_cid(&authorization_ipld.allowed, bundle, store).await?;
        RevokedIpld::try_extend_bundle_with_cid(&authorization_ipld.revoked, bundle, store).await?;

        bundle.add(*cid, self_bytes);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use libipld_cbor::DagCborCodec;
    use libipld_core::raw::RawCodec;
    use noosphere_storage::{interface::BlockStore, memory::MemoryStore};
    use serde_bytes::Bytes;
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

        assert!(bundle.contains(sphere.cid()));

        let memo = sphere.try_as_memo().await.unwrap();

        assert!(bundle.contains(&memo.body));
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_bundles_a_sphere_with_links() {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (sphere, ucan, _) = Sphere::try_generate(&owner_did, &mut store).await.unwrap();

        let foo_key = String::from("foo");
        let foo_cid = store.save::<RawCodec, _>(Bytes::new(b"foo")).await.unwrap();
        let mut mutation = SphereMutation::new(&owner_did);
        mutation.links_mut().set(&foo_key, &foo_cid);

        let mut revision = sphere.try_apply_mutation(&mutation).await.unwrap();
        let new_cid = revision.try_sign(&owner_key, Some(&ucan)).await.unwrap();

        let bundle = MemoIpld::try_bundle_with_cid(&new_cid, &store)
            .await
            .unwrap();

        assert_eq!(bundle.map().keys().len(), 11);

        let sphere = Sphere::at(&new_cid, &store);

        assert!(bundle.contains(sphere.cid()));

        let memo = sphere.try_as_memo().await.unwrap();

        assert!(bundle.contains(&memo.body));

        let sphere_ipld = sphere.try_as_body().await.unwrap();
        let links_cid = sphere_ipld.links.unwrap();

        assert!(bundle.contains(&links_cid));

        let links_ipld = store
            .load::<DagCborCodec, LinksIpld>(&links_cid)
            .await
            .unwrap();

        assert!(bundle.contains(&links_ipld.changelog));
        assert!(bundle.contains(&foo_cid));
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_only_bundles_the_revision_delta() {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (sphere, ucan, _) = Sphere::try_generate(&owner_did, &mut store).await.unwrap();

        let foo_key = String::from("foo");
        let foo_cid = store.save::<RawCodec, _>(Bytes::new(b"foo")).await.unwrap();
        let mut first_mutation = SphereMutation::new(&owner_did);
        first_mutation.links_mut().set(&foo_key, &foo_cid);

        let mut revision = sphere.try_apply_mutation(&first_mutation).await.unwrap();
        let new_cid = revision.try_sign(&owner_key, Some(&ucan)).await.unwrap();

        let sphere = Sphere::at(&new_cid, &store);

        let bar_key = String::from("bar");
        let bar_cid = store.save::<RawCodec, _>(Bytes::new(b"bar")).await.unwrap();
        let mut second_mutation = SphereMutation::new(&owner_did);
        second_mutation.links_mut().set(&bar_key, &bar_cid);

        let mut revision = sphere.try_apply_mutation(&second_mutation).await.unwrap();
        let new_cid = revision.try_sign(&owner_key, Some(&ucan)).await.unwrap();

        let bundle = MemoIpld::try_bundle_with_cid(&new_cid, &store)
            .await
            .unwrap();

        assert_eq!(bundle.map().keys().len(), 11);
        assert!(!bundle.contains(&foo_cid));
        assert!(bundle.contains(&bar_cid));
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_bundles_all_revisions_in_a_timeslice() {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (sphere, ucan, _) = Sphere::try_generate(&owner_did, &mut store).await.unwrap();

        let original_cid = *sphere.cid();

        let foo_key = String::from("foo");
        let foo_cid = store.save::<RawCodec, _>(Bytes::new(b"foo")).await.unwrap();
        let mut first_mutation = SphereMutation::new(&owner_did);
        first_mutation.links_mut().set(&foo_key, &foo_cid);

        let mut revision = sphere.try_apply_mutation(&first_mutation).await.unwrap();
        let second_cid = revision.try_sign(&owner_key, Some(&ucan)).await.unwrap();

        let sphere = Sphere::at(&second_cid, &store);

        let bar_key = String::from("bar");
        let bar_cid = store.save::<RawCodec, _>(Bytes::new(b"bar")).await.unwrap();
        let mut second_mutation = SphereMutation::new(&owner_did);
        second_mutation.links_mut().set(&bar_key, &bar_cid);

        let mut revision = sphere.try_apply_mutation(&second_mutation).await.unwrap();
        let final_cid = revision.try_sign(&owner_key, Some(&ucan)).await.unwrap();

        let timeline = Timeline::new(&store);

        let bundle =
            Bundle::try_from_timeslice(&timeline.slice(&final_cid, Some(&second_cid)), &store)
                .await
                .unwrap();

        assert_eq!(bundle.map().keys().len(), 16);

        assert!(bundle.contains(&foo_cid));
        assert!(bundle.contains(&bar_cid));
        assert!(bundle.contains(&final_cid));
        assert!(bundle.contains(&second_cid));
        assert!(!bundle.contains(&original_cid));
    }
}
