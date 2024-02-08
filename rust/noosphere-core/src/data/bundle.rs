// We are removing this module, so not gonna bother documenting...
#![allow(missing_docs)]

use std::{collections::BTreeMap, str::FromStr};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use cid::Cid;

use futures::{pin_mut, StreamExt};
use libipld_cbor::DagCborCodec;
use libipld_core::raw::RawCodec;
use noosphere_storage::{block_deserialize, block_serialize, BlockStore, UcanStore};
use noosphere_ucan::{store::UcanJwtStore, Ucan};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{
    data::{
        AuthorityIpld, BodyChunkIpld, ChangelogIpld, ContentIpld, ContentType, DelegationIpld,
        DelegationsIpld, Header, IdentityIpld, MapOperation, MemoIpld, RevocationIpld,
        RevocationsIpld, VersionedMapIpld, VersionedMapKey, VersionedMapValue,
    },
    view::{Sphere, Timeslice},
};

use super::{AddressBookIpld, IdentitiesIpld, Jwt, Link, LinkRecord};

// TODO: This should maybe only collect CIDs, and then streaming-serialize to
// a CAR (https://ipld.io/specs/transport/car/carv2/)
#[derive(PartialEq, Eq, Debug, Clone, Default, Serialize, Deserialize)]
pub struct Bundle(BTreeMap<String, Vec<u8>>);

impl Bundle {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn contains(&self, cid: &Cid) -> bool {
        self.0.contains_key(&cid.to_string())
    }

    pub async fn load_into<S: BlockStore>(&self, store: &mut S) -> Result<()> {
        debug!("Loading {} blocks into store...", self.0.len());

        // TODO: Parrallelize this
        for (cid_string, block_bytes) in self.0.iter() {
            let cid = Cid::from_str(cid_string)?;

            store.put_block(&cid, block_bytes).await?;

            match cid.codec() {
                codec_id if codec_id == u64::from(DagCborCodec) => {
                    store.put_links::<DagCborCodec>(&cid, block_bytes).await?;
                }
                codec_id if codec_id == u64::from(RawCodec) => {
                    store.put_links::<RawCodec>(&cid, block_bytes).await?;
                }
                codec_id => warn!("Unrecognized codec {}; skipping...", codec_id),
            }

            // TODO: Verify CID is correct, maybe?
        }

        Ok(())
    }

    pub async fn from_timeslice<'a, S: BlockStore>(
        timeslice: &Timeslice<'a, S>,
        store: &S,
    ) -> Result<Bundle> {
        let stream = timeslice.stream();
        let mut bundle = Bundle::default();

        pin_mut!(stream);

        while let Some(ancestor) = stream.next().await {
            let (_, memo) = ancestor?;
            memo.extend_bundle(&mut bundle, store).await?;
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
        CanBundle::extend_bundle_with_cid(cid, self, store).await?;
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub trait TryBundleSendSync: Send + Sync {}

#[cfg(not(target_arch = "wasm32"))]
impl<T> TryBundleSendSync for T where T: Send + Sync {}

#[cfg(target_arch = "wasm32")]
pub trait TryBundleSendSync {}

#[cfg(target_arch = "wasm32")]
impl<T> TryBundleSendSync for T {}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait TryBundle: TryBundleSendSync + Serialize + DeserializeOwned {
    async fn extend_bundle<S: BlockStore>(&self, bundle: &mut Bundle, _store: &S) -> Result<()> {
        let (self_cid, self_bytes) = block_serialize::<DagCborCodec, _>(self)?;
        bundle.add(self_cid, self_bytes);
        Ok(())
    }

    async fn extend_bundle_with_cid<S: BlockStore>(
        cid: &Cid,
        bundle: &mut Bundle,
        store: &S,
    ) -> Result<()> {
        let item = store.load::<DagCborCodec, Self>(cid).await?;
        item.extend_bundle(bundle, store).await?;

        Ok(())
    }

    async fn bundle<S: BlockStore>(&self, store: &S) -> Result<Bundle> {
        let mut bundle = Bundle::default();
        self.extend_bundle(&mut bundle, store).await?;
        Ok(bundle)
    }

    async fn bundle_with_cid<S: BlockStore>(cid: &Cid, store: &S) -> Result<Bundle> {
        let mut bundle = Bundle::default();
        Self::extend_bundle_with_cid(cid, &mut bundle, store).await?;
        Ok(bundle)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl TryBundle for BodyChunkIpld {
    async fn extend_bundle_with_cid<S: BlockStore>(
        cid: &Cid,
        bundle: &mut Bundle,
        store: &S,
    ) -> Result<()> {
        let mut next_cid = Some(*cid);

        while let Some(cid) = next_cid {
            trace!(?cid, "Bundling BodyChunkIpld...");

            let bytes = store.require_block(&cid).await?;
            let chunk = block_deserialize::<DagCborCodec, BodyChunkIpld>(&bytes)?;
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
    V: VersionedMapValue + TryBundle,
{
    async fn extend_bundle_with_cid<S: BlockStore>(
        cid: &Cid,
        bundle: &mut Bundle,
        store: &S,
    ) -> Result<()> {
        let bytes = store.require_block(cid).await?;
        let changelog = block_deserialize::<DagCborCodec, Self>(&bytes)?;

        trace!(?cid, "Bundling ChangeLogIpld...");

        bundle.add(*cid, bytes);

        for op in changelog.changes {
            if let MapOperation::Add { value, key } = op {
                trace!("...added entry {key}");
                value.extend_bundle(bundle, store).await?;
            }
        }

        Ok(())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl TryBundle for MemoIpld {
    async fn extend_bundle<S: BlockStore>(&self, bundle: &mut Bundle, store: &S) -> Result<()> {
        let (self_cid, self_bytes) = block_serialize::<DagCborCodec, _>(self)?;
        trace!(cid = ?self_cid, "Bundling MemoIpld....");

        bundle.add(self_cid, self_bytes);

        match self.get_first_header(&Header::ContentType) {
            Some(value) => {
                match ContentType::from_str(&value)? {
                    ContentType::Subtext
                    | ContentType::Text
                    | ContentType::Bytes
                    | ContentType::Json
                    | ContentType::Cbor => {
                        bundle.extend::<BodyChunkIpld, _>(&self.body, store).await?;
                    }
                    ContentType::Sphere => {
                        trace!("Bundling sphere revision {self_cid}...");

                        let sphere = Sphere::at(&Link::from(self_cid), store);
                        let mutation = sphere.derive_mutation().await?;
                        let sphere_body = sphere.to_body().await?;

                        let sphere_body_bytes = store.require_block(&self.body).await?;
                        bundle.add(self.body, sphere_body_bytes);

                        if !mutation.content().changes().is_empty() {
                            trace!(cid = ?sphere_body.content, "Bundling content...");
                            ContentIpld::extend_bundle_with_cid(
                                &sphere_body.content,
                                bundle,
                                store,
                            )
                            .await?;
                        }

                        if !mutation.delegations().changes().is_empty()
                            || !mutation.revocations().changes().is_empty()
                        {
                            trace!(cid = ?sphere_body.authority, "Bundling authority...");
                            AuthorityIpld::extend_bundle_with_cid(
                                &sphere_body.authority,
                                bundle,
                                store,
                            )
                            .await?;
                        }

                        if !mutation.identities().changes().is_empty() {
                            trace!(cid = ?sphere_body.address_book, "Bundling address book...");
                            AddressBookIpld::extend_bundle_with_cid(
                                &sphere_body.address_book,
                                bundle,
                                store,
                            )
                            .await?;
                        }
                    }
                    ContentType::Unknown(content_type) => {
                        warn!("Unrecognized content type {:?}; attempting to bundle as body chunks...", content_type);
                        // Fallback to body chunks....
                        bundle.extend::<BodyChunkIpld, _>(&self.body, store).await?;
                    }
                }
            }
            None => {
                warn!("No content type specified; only bundling a single block");
                bundle.add(
                    self.body,
                    store
                        .get_block(&self.body)
                        .await?
                        .ok_or_else(|| anyhow!("Unable to find block for {}", self.body))?,
                );
            }
        };

        Ok(())
    }

    async fn extend_bundle_with_cid<S: BlockStore>(
        cid: &Cid,
        bundle: &mut Bundle,
        store: &S,
    ) -> Result<()> {
        store
            .load::<DagCborCodec, MemoIpld>(cid)
            .await?
            .extend_bundle(bundle, store)
            .await?;
        Ok(())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<K, V> TryBundle for VersionedMapIpld<K, V>
where
    K: VersionedMapKey,
    V: VersionedMapValue + TryBundle,
{
    async fn extend_bundle<S: BlockStore>(&self, bundle: &mut Bundle, store: &S) -> Result<()> {
        trace!("Bundling versioned map...");
        let (self_cid, self_bytes) = block_serialize::<DagCborCodec, _>(self)?;

        ChangelogIpld::<MapOperation<K, V>>::extend_bundle_with_cid(&self.changelog, bundle, store)
            .await?;

        bundle.add(self_cid, self_bytes);

        Ok(())
    }

    async fn extend_bundle_with_cid<S: BlockStore>(
        cid: &Cid,
        bundle: &mut Bundle,
        store: &S,
    ) -> Result<()> {
        let map: Self = store.load::<DagCborCodec, _>(cid).await?;
        map.extend_bundle(bundle, store).await?;
        Ok(())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<T> TryBundle for Link<T>
where
    T: TryBundle + Clone,
{
    async fn extend_bundle<S: BlockStore>(&self, bundle: &mut Bundle, store: &S) -> Result<()> {
        T::extend_bundle_with_cid(self, bundle, store).await
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl TryBundle for Jwt {
    async fn extend_bundle_with_cid<S: BlockStore>(
        cid: &Cid,
        bundle: &mut Bundle,
        store: &S,
    ) -> Result<()> {
        bundle.add(*cid, store.require_block(cid).await?);
        Ok(())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl TryBundle for LinkRecord {
    async fn extend_bundle<S: BlockStore>(&self, bundle: &mut Bundle, store: &S) -> Result<()> {
        let cid = self.to_cid(cid::multihash::Code::Blake3_256)?;
        Self::extend_bundle_with_cid(&cid, bundle, store).await
    }

    async fn extend_bundle_with_cid<S: BlockStore>(
        cid: &Cid,
        bundle: &mut Bundle,
        store: &S,
    ) -> Result<()> {
        trace!("Bundling LinkRecord...");

        let mut remaining = vec![*cid];
        let ucan_store = UcanStore(store.clone());

        while let Some(cid) = remaining.pop() {
            trace!("...with proof {cid}");

            let jwt = ucan_store.require_token(&cid).await?;
            let ucan = Ucan::try_from(jwt.as_str())?;

            bundle.add(cid, store.require_block(&cid).await?);

            if let Some(proofs) = ucan.proofs() {
                for proof_string in proofs {
                    remaining.push(Cid::try_from(proof_string.as_str())?)
                }
            }
        }
        Ok(())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl TryBundle for IdentityIpld {
    async fn extend_bundle<S: BlockStore>(&self, bundle: &mut Bundle, store: &S) -> Result<()> {
        trace!("Bundling IdentityIpld...");
        let (self_cid, self_bytes) = block_serialize::<DagCborCodec, _>(self)?;
        bundle.add(self_cid, self_bytes);
        if let Some(cid) = &self.link_record {
            LinkRecord::extend_bundle_with_cid(cid, bundle, store).await?;
        };
        Ok(())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl TryBundle for DelegationIpld {
    async fn extend_bundle<S: BlockStore>(&self, bundle: &mut Bundle, store: &S) -> Result<()> {
        let (self_cid, self_bytes) = block_serialize::<DagCborCodec, _>(self)?;
        bundle.add(self_cid, self_bytes);
        bundle.add(self.jwt, store.require_block(&self.jwt).await?);
        Ok(())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl TryBundle for RevocationIpld {}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl TryBundle for AuthorityIpld {
    async fn extend_bundle_with_cid<S: BlockStore>(
        cid: &Cid,
        bundle: &mut Bundle,
        store: &S,
    ) -> Result<()> {
        let self_bytes = store.require_block(cid).await?;
        let authority_ipld = block_deserialize::<DagCborCodec, AuthorityIpld>(&self_bytes)?;

        DelegationsIpld::extend_bundle_with_cid(&authority_ipld.delegations, bundle, store).await?;
        RevocationsIpld::extend_bundle_with_cid(&authority_ipld.revocations, bundle, store).await?;

        bundle.add(*cid, self_bytes);

        Ok(())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl TryBundle for AddressBookIpld {
    async fn extend_bundle_with_cid<S: BlockStore>(
        cid: &Cid,
        bundle: &mut Bundle,
        store: &S,
    ) -> Result<()> {
        let self_bytes = store.require_block(cid).await?;
        let address_book_ipld = block_deserialize::<DagCborCodec, AddressBookIpld>(&self_bytes)?;

        IdentitiesIpld::extend_bundle_with_cid(&address_book_ipld.identities, bundle, store)
            .await?;

        bundle.add(*cid, self_bytes);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{helpers::make_valid_link_record, tracing::initialize_tracing};

    use anyhow::Result;
    use cid::Cid;
    use libipld_cbor::DagCborCodec;
    use libipld_core::{ipld::Ipld, raw::RawCodec};
    use noosphere_storage::{block_serialize, BlockStore, MemoryStore, UcanStore};
    use noosphere_ucan::{builder::UcanBuilder, crypto::KeyMaterial};

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use crate::{
        authority::generate_ed25519_key,
        data::{Bundle, ContentIpld, DelegationIpld, MemoIpld, TryBundle},
        view::{Sphere, SphereMutation, Timeline},
    };

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_bundles_an_empty_sphere() {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (sphere, _, _) = Sphere::generate(&owner_did, &mut store).await.unwrap();
        let bundle = MemoIpld::bundle_with_cid(sphere.cid(), &store)
            .await
            .unwrap();

        assert!(bundle.contains(sphere.cid()));

        let memo = sphere.to_memo().await.unwrap();

        assert!(bundle.contains(&memo.body));
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_bundles_a_delegation_with_its_associated_jwt() -> Result<()> {
        let store = MemoryStore::default();
        let key = generate_ed25519_key();
        let did = key.get_did().await.unwrap();

        let jwt = UcanBuilder::default()
            .issued_by(&key)
            .for_audience(&did)
            .with_lifetime(100)
            .with_nonce()
            .build()?
            .sign()
            .await?
            .encode()?;

        let (jwt_cid, _) = block_serialize::<RawCodec, _>(Ipld::Bytes(jwt.as_bytes().to_vec()))?;

        let delegation = DelegationIpld::register("foo", &jwt, &store).await?;

        let (delegation_cid, _) = block_serialize::<DagCborCodec, _>(&delegation)?;

        let bundle = delegation.bundle(&store).await?;

        assert!(bundle.contains(&delegation_cid));
        assert!(bundle.contains(&jwt_cid));

        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_bundles_a_link_record_with_its_associated_proofs() -> Result<()> {
        initialize_tracing(None);

        let store = MemoryStore::default();
        let (_, link_record, link_record_link) =
            make_valid_link_record(&mut UcanStore(store.clone())).await?;

        let proof_cid = Cid::try_from(
            link_record
                .proofs()
                .as_ref()
                .unwrap()
                .first()
                .unwrap()
                .as_str(),
        )?;
        let bundle = link_record.bundle(&store).await?;

        assert!(bundle.contains(&link_record_link));
        assert!(bundle.contains(&proof_cid));

        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_bundles_a_sphere_with_links() {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (sphere, ucan, _) = Sphere::generate(&owner_did, &mut store).await.unwrap();

        let foo_key = String::from("foo");
        let foo_memo = MemoIpld::for_body(&mut store, b"foo").await.unwrap();
        let foo_cid = store.save::<DagCborCodec, _>(&foo_memo).await.unwrap();

        let mut mutation = SphereMutation::new(&owner_did);
        mutation.content_mut().set(&foo_key, &foo_cid.into());

        let mut revision = sphere.apply_mutation(&mutation).await.unwrap();
        let new_cid = revision.sign(&owner_key, Some(&ucan)).await.unwrap();

        let bundle = MemoIpld::bundle_with_cid(&new_cid, &store).await.unwrap();

        assert_eq!(bundle.map().keys().len(), 6);

        let sphere = Sphere::at(&new_cid, &store);

        assert!(bundle.contains(sphere.cid()));

        let memo = sphere.to_memo().await.unwrap();

        assert!(bundle.contains(&memo.body));

        let sphere_ipld = sphere.to_body().await.unwrap();
        let links_cid = sphere_ipld.content;

        assert!(bundle.contains(&links_cid));

        let links_ipld = store
            .load::<DagCborCodec, ContentIpld>(&links_cid)
            .await
            .unwrap();

        assert!(bundle.contains(&links_ipld.changelog));
        assert!(bundle.contains(&foo_cid));
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_bundles_memo_body_content() {
        let mut store = MemoryStore::default();

        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (sphere, authorization, _) = Sphere::generate(&owner_did, &mut store).await.unwrap();

        let body_cid = store
            .save::<RawCodec, _>(Ipld::Bytes(b"foobar".to_vec()))
            .await
            .unwrap();

        let memo = MemoIpld {
            parent: None,
            headers: Vec::new(),
            body: body_cid,
        };
        let memo_cid = store.save::<DagCborCodec, _>(memo).await.unwrap();
        let key = "foo".to_string();

        let mut mutation = SphereMutation::new(&owner_did);

        mutation.content_mut().set(&key, &memo_cid.into());

        let mut revision = sphere.apply_mutation(&mutation).await.unwrap();

        let sphere_revision = revision
            .sign(&owner_key, Some(&authorization))
            .await
            .unwrap();

        let bundle = MemoIpld::bundle_with_cid(&sphere_revision, &store)
            .await
            .unwrap();

        assert!(bundle.contains(&body_cid));
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_only_bundles_the_revision_delta() {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (sphere, ucan, _) = Sphere::generate(&owner_did, &mut store).await.unwrap();

        let foo_key = String::from("foo");
        let foo_memo = MemoIpld::for_body(&mut store, b"foo").await.unwrap();
        let foo_cid = store.save::<DagCborCodec, _>(&foo_memo).await.unwrap();
        let mut first_mutation = SphereMutation::new(&owner_did);
        first_mutation.content_mut().set(&foo_key, &foo_cid.into());

        let mut revision = sphere.apply_mutation(&first_mutation).await.unwrap();
        let new_cid = revision.sign(&owner_key, Some(&ucan)).await.unwrap();

        let sphere = Sphere::at(&new_cid, &store);

        let bar_key = String::from("bar");
        let bar_memo = MemoIpld::for_body(&mut store, b"bar").await.unwrap();
        let bar_cid = store.save::<DagCborCodec, _>(&bar_memo).await.unwrap();

        let mut second_mutation = SphereMutation::new(&owner_did);
        second_mutation.content_mut().set(&bar_key, &bar_cid.into());

        let mut revision = sphere.apply_mutation(&second_mutation).await.unwrap();
        let new_cid = revision.sign(&owner_key, Some(&ucan)).await.unwrap();

        let bundle = MemoIpld::bundle_with_cid(&new_cid, &store).await.unwrap();

        assert_eq!(bundle.map().keys().len(), 6);
        assert!(!bundle.contains(&foo_cid));
        assert!(bundle.contains(&bar_cid));
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_bundles_all_revisions_in_a_timeslice() {
        let mut store = MemoryStore::default();
        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (sphere, ucan, _) = Sphere::generate(&owner_did, &mut store).await.unwrap();

        let original_cid = *sphere.cid();

        let foo_key = String::from("foo");
        let foo_memo = MemoIpld::for_body(&mut store, b"foo").await.unwrap();
        let foo_cid = store.save::<DagCborCodec, _>(&foo_memo).await.unwrap();
        let mut first_mutation = SphereMutation::new(&owner_did);
        first_mutation.content_mut().set(&foo_key, &foo_cid.into());

        let mut revision = sphere.apply_mutation(&first_mutation).await.unwrap();
        let second_cid = revision.sign(&owner_key, Some(&ucan)).await.unwrap();

        let sphere = Sphere::at(&second_cid, &store);

        let bar_key = String::from("bar");
        let bar_memo = MemoIpld::for_body(&mut store, b"bar").await.unwrap();
        let bar_cid = store.save::<DagCborCodec, _>(&bar_memo).await.unwrap();
        let mut second_mutation = SphereMutation::new(&owner_did);

        second_mutation.content_mut().set(&bar_key, &bar_cid.into());

        let mut revision = sphere.apply_mutation(&second_mutation).await.unwrap();
        let final_cid = revision.sign(&owner_key, Some(&ucan)).await.unwrap();

        let timeline = Timeline::new(&store);

        let bundle = Bundle::from_timeslice(&timeline.slice(&final_cid, Some(&second_cid)), &store)
            .await
            .unwrap();

        assert_eq!(bundle.map().keys().len(), 12);

        assert!(bundle.contains(&foo_cid));
        assert!(bundle.contains(&bar_cid));
        assert!(bundle.contains(&final_cid));
        assert!(bundle.contains(&second_cid));
        assert!(!bundle.contains(&original_cid));
    }
}
