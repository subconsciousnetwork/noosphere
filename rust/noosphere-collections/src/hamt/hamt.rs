// Adapted for Noosphere from https://github.com/filecoin-project/ref-fvm
// Source copyright and license:
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::Result;
use noosphere_cbor::TryDagCbor;
use noosphere_storage::interface::{DagCborStore, Store};
use std::borrow::Borrow;
use std::marker::PhantomData;
use std::pin::Pin;
use tokio_stream::{Stream, StreamExt};

use cid::Cid;
use forest_hash_utils::{BytesKey, Hash};
use serde::de::DeserializeOwned;
use serde::{Serialize, Serializer};

use crate::hamt::node::Node;
use crate::hamt::{HashAlgorithm, Sha256};

pub const MAX_ARRAY_WIDTH: usize = 3;

/// Default bit width for indexing a hash at each depth level
pub const DEFAULT_BIT_WIDTH: u32 = 8;

#[cfg(not(target_arch = "wasm32"))]
pub trait TargetConditionalSendSync: Send + Sync {}

#[cfg(not(target_arch = "wasm32"))]
impl<S: Send + Sync> TargetConditionalSendSync for S {}

#[cfg(target_arch = "wasm32")]
pub trait TargetConditionalSendSync {}

#[cfg(target_arch = "wasm32")]
impl<S> TargetConditionalSendSync for S {}

/// Implementation of the HAMT data structure for IPLD.
///
/// # Examples
///
/// ```
/// use noosphere_collections::hamt::Hamt;
/// use noosphere_storage::memory::MemoryStore;
///
/// async_std::task::block_on(async {
///     let store = MemoryStore::default();
///
///     let mut map: Hamt<_, _, usize> = Hamt::new(store);
///     map.set(1, "a".to_string()).await.unwrap();
///     assert_eq!(map.get(&1).await.unwrap(), Some(&"a".to_string()));
///     assert_eq!(map.delete(&1).await.unwrap(), Some((1, "a".to_string())));
///     assert_eq!(map.get::<_>(&1).await.unwrap(), None);
///     let cid = map.flush().await.unwrap();
/// });
/// ```
#[derive(Debug)]
pub struct Hamt<BS, V, K = BytesKey, H = Sha256>
where
    K: TargetConditionalSendSync,
    V: TargetConditionalSendSync,
{
    root: Node<K, V, H>,
    store: BS,

    bit_width: u32,
    hash: PhantomData<H>,
}

impl<BS, V, K, H> Serialize for Hamt<BS, V, K, H>
where
    K: Serialize + TargetConditionalSendSync,
    V: Serialize + TargetConditionalSendSync,
    H: HashAlgorithm,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.root.serialize(serializer)
    }
}

impl<
        K: PartialEq + TargetConditionalSendSync,
        V: PartialEq + TargetConditionalSendSync,
        S: Store,
        H: HashAlgorithm,
    > PartialEq for Hamt<S, V, K, H>
{
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root
    }
}

impl<BS, V, K, H> Hamt<BS, V, K, H>
where
    K: Hash + Eq + PartialOrd + Serialize + DeserializeOwned + TargetConditionalSendSync,
    V: Serialize + DeserializeOwned + TargetConditionalSendSync + PartialEq,
    BS: Store,
    H: HashAlgorithm,
{
    pub fn new(store: BS) -> Self {
        Self::new_with_bit_width(store, DEFAULT_BIT_WIDTH)
    }

    /// Construct hamt with a bit width
    pub fn new_with_bit_width(store: BS, bit_width: u32) -> Self {
        Self {
            root: Node::default(),
            store,
            bit_width,
            hash: Default::default(),
        }
    }

    /// Lazily instantiate a hamt from this root Cid.
    pub async fn load(cid: &Cid, store: BS) -> Result<Self> {
        Self::load_with_bit_width(cid, store, DEFAULT_BIT_WIDTH).await
    }

    /// Lazily instantiate a hamt from this root Cid with a specified bit width.
    pub async fn load_with_bit_width(cid: &Cid, store: BS, bit_width: u32) -> Result<Self> {
        let root: Node<K, V, H> = store.load(cid).await?;
        Ok(Self {
            root,
            store,
            bit_width,
            hash: Default::default(),
        })
    }

    /// Sets the root based on the Cid of the root node using the Hamt store
    pub async fn set_root(&mut self, cid: &Cid) -> Result<()> {
        self.root = self.store.load(cid).await?;

        Ok(())
    }

    /// Returns a reference to the underlying store of the Hamt.
    pub fn store(&self) -> &BS {
        &self.store
    }

    /// Inserts a key-value pair into the HAMT.
    ///
    /// If the HAMT did not have this key present, `None` is returned.
    ///
    /// If the HAMT did have this key present, the value is updated, and the old
    /// value is returned. The key is not updated, though;
    ///
    /// # Examples
    ///
    /// ```
    /// use noosphere_collections::hamt::Hamt;
    /// use noosphere_storage::memory::MemoryStore;
    ///
    /// async_std::task::block_on(async {
    ///     let store = MemoryStore::default();
    ///
    ///     let mut map: Hamt<_, _, usize> = Hamt::new(store);
    ///     map.set(37, "a".to_string()).await.unwrap();
    ///     assert_eq!(map.is_empty(), false);
    ///
    ///     map.set(37, "b".to_string()).await.unwrap();
    ///     map.set(37, "c".to_string()).await.unwrap();
    /// })
    /// ```
    pub async fn set(&mut self, key: K, value: V) -> Result<Option<V>> {
        self.root
            .set(key, value, self.store.borrow(), self.bit_width, true)
            .await
            .map(|(r, _)| r)
    }

    /// Inserts a key-value pair into the HAMT only if that key does not already exist.
    ///
    /// If the HAMT did not have this key present, `true` is returned and the key/value is added.
    ///
    /// If the HAMT did have this key present, this function will return false
    ///
    /// # Examples
    ///
    /// ```
    /// use noosphere_collections::hamt::Hamt;
    /// use noosphere_storage::memory::MemoryStore;
    ///
    /// async_std::task::block_on(async {
    ///     let store = MemoryStore::default();
    ///
    ///     let mut map: Hamt<_, _, usize> = Hamt::new(store);
    ///     let a = map.set_if_absent(37, "a".to_string()).await.unwrap();
    ///     assert_eq!(map.is_empty(), false);
    ///     assert_eq!(a, true);
    ///
    ///     let b = map.set_if_absent(37, "b".to_string()).await.unwrap();
    ///     assert_eq!(b, false);
    ///     assert_eq!(map.get(&37).await.unwrap(), Some(&"a".to_string()));
    ///
    ///     let c = map.set_if_absent(30, "c".to_string()).await.unwrap();
    ///     assert_eq!(c, true);
    /// });
    /// ```
    pub async fn set_if_absent(&mut self, key: K, value: V) -> Result<bool>
    where
        V: PartialEq,
    {
        self.root
            .set(key, value, self.store.borrow(), self.bit_width, false)
            .await
            .map(|(_, set)| set)
    }

    /// Returns a reference to the value corresponding to the key.
    ///
    /// The key may be any borrowed form of the map's key type, but
    /// `Hash` and `Eq` on the borrowed form *must* match those for
    /// the key type.
    ///
    /// # Examples
    ///
    /// ```
    /// use noosphere_collections::hamt::Hamt;
    /// use noosphere_storage::memory::MemoryStore;
    ///
    /// async_std::task::block_on(async {
    ///     let store = MemoryStore::default();
    ///
    ///     let mut map: Hamt<_, _, usize> = Hamt::new(store);
    ///     map.set(1, "a".to_string()).await.unwrap();
    ///     assert_eq!(map.get(&1).await.unwrap(), Some(&"a".to_string()));
    ///     assert_eq!(map.get(&2).await.unwrap(), None);
    /// })
    /// ```
    #[inline]
    pub async fn get<Q: ?Sized>(&self, k: &Q) -> Result<Option<&V>>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + TargetConditionalSendSync,
        V: DeserializeOwned,
    {
        match self
            .root
            .get(k, self.store.borrow(), self.bit_width)
            .await?
        {
            Some(v) => Ok(Some(v)),
            None => Ok(None),
        }
    }

    /// Returns `true` if a value exists for the given key in the HAMT.
    ///
    /// The key may be any borrowed form of the map's key type, but
    /// `Hash` and `Eq` on the borrowed form *must* match those for
    /// the key type.
    ///
    /// # Examples
    ///
    /// ```
    /// use noosphere_collections::hamt::Hamt;
    /// use noosphere_storage::memory::MemoryStore;
    ///
    /// async_std::task::block_on(async {
    ///     let store = MemoryStore::default();
    ///
    ///     let mut map: Hamt<_, _, usize> = Hamt::new(store);
    ///     map.set(1, "a".to_string()).await.unwrap();
    ///     assert_eq!(map.contains_key(&1).await.unwrap(), true);
    ///     assert_eq!(map.contains_key(&2).await.unwrap(), false);
    /// });
    /// ```
    #[inline]
    pub async fn contains_key<Q: ?Sized>(&self, k: &Q) -> Result<bool>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + TargetConditionalSendSync,
    {
        Ok(self
            .root
            .get(k, self.store.borrow(), self.bit_width)
            .await?
            .is_some())
    }

    /// Removes a key from the HAMT, returning the value at the key if the key
    /// was previously in the HAMT.
    ///
    /// The key may be any borrowed form of the HAMT's key type, but
    /// `Hash` and `Eq` on the borrowed form *must* match those for
    /// the key type.
    ///
    /// # Examples
    ///
    /// ```
    /// use noosphere_collections::hamt::Hamt;
    /// use noosphere_storage::memory::MemoryStore;
    ///
    /// async_std::task::block_on(async {
    ///     let store = MemoryStore::default();
    ///
    ///     let mut map: Hamt<_, _, usize> = Hamt::new(store);
    ///     map.set(1, "a".to_string()).await.unwrap();
    ///     assert_eq!(map.delete(&1).await.unwrap(), Some((1, "a".to_string())));
    ///     assert_eq!(map.delete(&1).await.unwrap(), None);
    /// });
    /// ```
    pub async fn delete<Q: ?Sized>(&mut self, k: &Q) -> Result<Option<(K, V)>>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + TargetConditionalSendSync,
    {
        self.root
            .remove_entry(k, self.store.borrow(), self.bit_width)
            .await
    }

    /// Flush root and return Cid for hamt
    pub async fn flush(&mut self) -> Result<Cid> {
        self.root.flush(&mut self.store).await?;
        let bytes = self.root.try_into_dag_cbor()?;
        Ok(self.store.write_cbor(&bytes).await?)
    }

    /// Returns true if the HAMT has no entries
    pub fn is_empty(&self) -> bool {
        self.root.is_empty()
    }

    /// Iterates over each KV in the Hamt and runs a function on the values.
    ///
    /// This function will constrain all values to be of the same type
    ///
    /// # Examples
    ///
    /// ```
    /// use noosphere_collections::hamt::Hamt;
    /// use noosphere_storage::memory::MemoryStore;
    ///
    /// async_std::task::block_on(async {
    ///     let store = MemoryStore::default();
    ///
    ///     let mut map: Hamt<_, _, usize> = Hamt::new(store);
    ///     map.set(1, 1).await.unwrap();
    ///     map.set(4, 2).await.unwrap();
    ///
    ///     let mut total = 0;
    ///     map.for_each(|_, v: &u64| {
    ///         total += v;
    ///         Ok(())
    ///     }).await.unwrap();
    ///     assert_eq!(total, 3);
    /// });
    /// ```
    #[inline]
    pub async fn for_each<F>(&self, mut f: F) -> Result<()>
    where
        V: DeserializeOwned,
        F: FnMut(&K, &V) -> anyhow::Result<()>,
    {
        // self.root.for_each(self.store.borrow(), &mut f).await
        let mut stream = self.stream();

        while let Some(Ok((key, value))) = stream.next().await {
            f(key, value)?;
        }

        Ok(())
        // for item
    }

    pub fn stream<'a>(&'a self) -> Pin<Box<dyn Stream<Item = Result<(&'a K, &'a V)>> + 'a>> {
        self.root.stream(&self.store)
    }

    /// Consumes this HAMT and returns the Blockstore it owns.
    pub fn into_store(self) -> BS {
        self.store
    }
}
