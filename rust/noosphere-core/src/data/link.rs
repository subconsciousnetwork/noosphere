use anyhow::{anyhow, Result};
use libipld_cbor::DagCborCodec;
use libipld_core::{
    codec::{Codec, Decode, Encode},
    raw::RawCodec,
};
use noosphere_common::ConditionalSend;
use std::fmt::Debug;
use std::{
    fmt::{Display, Formatter},
    io::{Read, Seek, Write},
    str::FromStr,
};
use std::{hash::Hash, marker::PhantomData, ops::Deref};

use cid::Cid;
use noosphere_storage::BlockStore;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use noosphere_collections::hamt::Hash as HamtHash;

/// A [Link] is a [Cid] with a type attached. The type represents the data that
/// the [Cid] refers to. This is a helpful construct to use to ensure that data
/// structures whose fields or elements may be [Cid]s can still retain strong
/// typing. A [Link] transparently represents its inner [Cid], so a data
/// structure that uses [Link]s can safely be interpretted in terms of [Cid]s,
/// and vice-versa.
#[derive(Ord, PartialOrd, Serialize, Deserialize, Clone)]
// NOTE: Required because libipld special-cases unit structs and errors
// SEE: https://github.com/ipld/libipld/blob/65e0b38520f62cfb2b67ebe658846d86dac2f73e/core/src/serde/ser.rs#L192
#[serde(from = "Cid", into = "Cid")]
#[repr(transparent)]
pub struct Link<T>
where
    T: Clone,
{
    /// The wrapped [Cid] of this [Link]
    pub cid: Cid,
    linked_type: PhantomData<T>,
}

impl<T> Copy for Link<T> where T: Clone {}

impl<T> Debug for Link<T>
where
    T: Clone,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Link")
            .field("cid", &self.cid.to_string())
            .field("linked_type", &self.linked_type)
            .finish()
    }
}

impl<T> Deref for Link<T>
where
    T: Clone,
{
    type Target = Cid;

    fn deref(&self) -> &Self::Target {
        &self.cid
    }
}

impl<T> Hash for Link<T>
where
    T: Clone,
{
    fn hash<H: std::hash::Hasher>(&self, hasher: &mut H) {
        Hash::hash(&self.cid, hasher)
    }
}

impl<T> PartialEq for Link<T>
where
    T: Clone,
{
    fn eq(&self, other: &Self) -> bool {
        self.cid == other.cid
    }
}

impl<T> Eq for Link<T> where T: Clone {}

impl<T> HamtHash for Link<T>
where
    T: Clone,
{
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.cid.hash().hash(state);
    }
}

impl<T> Link<T>
where
    T: Clone,
{
    /// Wrap a given [Cid] in a typed [Link]
    pub fn new(cid: Cid) -> Self {
        Link {
            cid,
            linked_type: PhantomData,
        }
    }
}

impl<T> Display for Link<T>
where
    T: Clone,
{
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        Display::fmt(&self.cid, f)
    }
}

impl<C: Codec, T> Encode<C> for Link<T>
where
    Cid: Encode<C>,
    T: Clone,
{
    fn encode<W: Write>(&self, c: C, w: &mut W) -> Result<()> {
        self.cid.encode(c, w)
    }
}

impl<C: Codec, T> Decode<C> for Link<T>
where
    Cid: Decode<C>,
    T: Clone,
{
    fn decode<R: Read + Seek>(c: C, r: &mut R) -> Result<Self> {
        Ok(Self::new(Cid::decode(c, r)?))
    }
}

impl<T> AsRef<Cid> for Link<T>
where
    T: Clone,
{
    fn as_ref(&self) -> &Cid {
        &self.cid
    }
}

impl<T> From<Cid> for Link<T>
where
    T: Clone,
{
    fn from(cid: Cid) -> Self {
        Self::new(cid)
    }
}

impl<T> From<&Cid> for Link<T>
where
    T: Clone,
{
    fn from(cid: &Cid) -> Self {
        Self::new(*cid)
    }
}

impl<T> From<Link<T>> for Cid
where
    T: Clone,
{
    fn from(link: Link<T>) -> Self {
        link.cid
    }
}

impl<T> FromStr for Link<T>
where
    T: Clone,
{
    type Err = <Cid as FromStr>::Err;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Cid::from_str(s)?.into())
    }
}

impl<T> From<Link<T>> for String
where
    T: Clone,
{
    fn from(value: Link<T>) -> Self {
        From::from(value.cid)
    }
}

impl<T> Link<T>
where
    T: Serialize + DeserializeOwned + Clone + ConditionalSend,
{
    /// Given a [BlockStore], attempt to load a value for the [Cid] of this
    /// [Link]. The loaded block will be interpretted as the type that is
    /// attached to the [Cid] by this [Link], and then returned.
    pub async fn load_from<S: BlockStore>(&self, store: &S) -> Result<T> {
        match self.codec() {
            codec_id if codec_id == u64::from(DagCborCodec) => {
                store.load::<DagCborCodec, _>(self).await
            }
            codec_id if codec_id == u64::from(RawCodec) => store.load::<RawCodec, _>(self).await,
            codec_id => Err(anyhow!("Unsupported codec {}", codec_id)),
        }
    }
}

#[cfg(test)]
mod tests {
    use cid::Cid;
    use libipld_cbor::DagCborCodec;
    use noosphere_storage::{BlockStore, MemoryStore};
    use serde::{Deserialize, Serialize};
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    use crate::data::MemoIpld;

    use super::Link;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_interpret_referenced_block_as_attached_type() {
        let mut store = MemoryStore::default();
        let cid = store
            .save::<DagCborCodec, _>(&MemoIpld {
                parent: None,
                headers: vec![("Foo".into(), "Bar".into())],
                body: Cid::default(),
            })
            .await
            .unwrap();

        let link = Link::<MemoIpld>::new(cid);

        let memo = link.load_from(&store).await.unwrap();

        assert_eq!(memo.get_first_header("Foo"), Some(String::from("Bar")))
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_transparently_serializes_and_deserializes_as_a_cid() {
        #[derive(Serialize, Deserialize)]
        struct UsesLink {
            pub link: Link<MemoIpld>,
        }

        #[derive(Serialize, Deserialize)]
        struct UsesCid {
            pub link: Cid,
        }

        let mut store = MemoryStore::default();

        let memo_cid = store
            .save::<DagCborCodec, _>(&MemoIpld {
                parent: None,
                headers: vec![("Foo".into(), "Bar".into())],
                body: Cid::default(),
            })
            .await
            .unwrap();

        let uses_link_cid = store
            .save::<DagCborCodec, _>(&UsesLink {
                link: Link::new(memo_cid),
            })
            .await
            .unwrap();

        let loaded_uses_cid = store
            .load::<DagCborCodec, UsesCid>(&uses_link_cid)
            .await
            .unwrap();

        assert_eq!(loaded_uses_cid.link, memo_cid);

        let loaded_uses_link = store
            .load::<DagCborCodec, UsesLink>(&uses_link_cid)
            .await
            .unwrap();

        assert_eq!(loaded_uses_link.link, Link::<MemoIpld>::new(memo_cid));
    }
}
