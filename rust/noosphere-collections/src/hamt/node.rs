// Adapted for Noosphere from https://github.com/filecoin-project/ref-fvm
// Source copyright and license:
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::Result;
use async_recursion::async_recursion;
use async_stream::try_stream;
use noosphere_cbor::TryDagCbor;
use noosphere_storage::interface::{DagCborStore, Store};
use std::borrow::Borrow;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::pin::Pin;
use tokio_stream::Stream;

use async_once_cell::OnceCell;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use forest_hash_utils::Hash;

use super::bitfield::Bitfield;
use super::hash_bits::HashBits;
use super::pointer::Pointer;
use super::{HashAlgorithm, KeyValuePair, TargetConditionalSendSync, MAX_ARRAY_WIDTH};

/// Node in Hamt tree which contains bitfield of set indexes and pointers to nodes
#[derive(Debug, Clone)]
pub struct Node<K: TargetConditionalSendSync, V: TargetConditionalSendSync, H> {
    pub(crate) bitfield: Bitfield,
    pub(crate) pointers: Vec<Pointer<K, V, H>>,
    hash: PhantomData<H>,
}

impl<K: PartialEq + TargetConditionalSendSync, V: PartialEq + TargetConditionalSendSync, H>
    PartialEq for Node<K, V, H>
{
    fn eq(&self, other: &Self) -> bool {
        (self.bitfield == other.bitfield) && (self.pointers == other.pointers)
    }
}

impl<K, V, H> Serialize for Node<K, V, H>
where
    K: Serialize + TargetConditionalSendSync,
    V: Serialize + TargetConditionalSendSync,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        (&self.bitfield, &self.pointers).serialize(serializer)
    }
}

impl<'de, K, V, H> Deserialize<'de> for Node<K, V, H>
where
    K: DeserializeOwned + TargetConditionalSendSync,
    V: DeserializeOwned + TargetConditionalSendSync,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (bitfield, pointers) = Deserialize::deserialize(deserializer)?;
        Ok(Node {
            bitfield,
            pointers,
            hash: Default::default(),
        })
    }
}

impl<K, V, H> Default for Node<K, V, H>
where
    K: TargetConditionalSendSync,
    V: TargetConditionalSendSync,
{
    fn default() -> Self {
        Node {
            bitfield: Bitfield::zero(),
            pointers: Vec::new(),
            hash: Default::default(),
        }
    }
}

impl<K, V, H> Node<K, V, H>
where
    K: Hash + Eq + PartialOrd + Serialize + DeserializeOwned + TargetConditionalSendSync,
    H: HashAlgorithm + TargetConditionalSendSync,
    V: Serialize + DeserializeOwned + TargetConditionalSendSync,
{
    pub async fn set<S: Store>(
        &mut self,
        key: K,
        value: V,
        store: &S,
        bit_width: u32,
        overwrite: bool,
    ) -> Result<(Option<V>, bool)>
    where
        V: PartialEq,
    {
        self.modify_value(
            HashBits::new(H::hash(&key)),
            bit_width,
            0,
            key,
            value,
            store,
            overwrite,
        )
        .await
    }

    #[inline]
    pub async fn get<Q: ?Sized + TargetConditionalSendSync, S: Store>(
        &self,
        k: &Q,
        store: &S,
        bit_width: u32,
    ) -> Result<Option<&V>>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        Ok(self.search(k, store, bit_width).await?.map(|kv| kv.value()))
    }

    #[inline]
    pub async fn remove_entry<Q: ?Sized, S>(
        &mut self,
        k: &Q,
        store: &S,
        bit_width: u32,
    ) -> Result<Option<(K, V)>>
    where
        K: Borrow<Q>,
        Q: Eq + Hash + TargetConditionalSendSync,
        S: Store,
    {
        self.rm_value(HashBits::new(H::hash(k)), bit_width, 0, k, store)
            .await
    }

    pub fn is_empty(&self) -> bool {
        self.pointers.is_empty()
    }

    pub(crate) fn stream<'a, S>(
        &'a self,
        store: &'a S,
    ) -> Pin<Box<dyn Stream<Item = Result<(&'a K, &'a V)>> + 'a>>
    where
        S: Store,
    {
        Box::pin(try_stream! {
            for p in &self.pointers {
                match p {
                    Pointer::Link { cid, cache } => {
                        if let Some(cached_node) = cache.get() {
                            let stream = cached_node.stream(store);
                            tokio::pin!(stream);
                            for await item in stream {
                                yield item?;
                            }
                        } else {
                            let node = match store.load(cid).await {
                                Ok(node) => Ok(node),
                                Err(error) => {
                                    #[cfg(feature = "ignore-dead-links")]
                                    continue;
                                    #[cfg(not(feature = "ignore-dead-links"))]
                                    Err(error)
                                }
                            }?;

                            // Ignore error intentionally, the cache value will always be the same
                            let cache_node = cache.get_or_init(async { node }).await;
                            let stream = cache_node.stream(store);
                            tokio::pin!(stream);
                            for await item in stream {
                                yield item?;
                            }
                        }
                    }
                    Pointer::Dirty(n) => {
                        let stream = n.stream(store);
                        tokio::pin!(stream);
                        for await item in stream {
                            yield item?;
                        }
                    }
                    Pointer::Values(kvs) => {
                        for kv in kvs {
                            yield (kv.key(), kv.value());
                        }
                    }
                }
            }
        })
    }

    /// Search for a key.
    async fn search<Q: ?Sized + TargetConditionalSendSync, S: Store>(
        &self,
        q: &Q,
        store: &S,
        bit_width: u32,
    ) -> Result<Option<&KeyValuePair<K, V>>>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        self.get_value(HashBits::new(H::hash(q)), bit_width, 0, q, store)
            .await
    }

    #[cfg_attr(target_arch="wasm32", async_recursion(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_recursion)]
    async fn get_value<Q: ?Sized + TargetConditionalSendSync, S: Store>(
        &self,
        mut hashed_key: HashBits,
        bit_width: u32,
        depth: u64,
        key: &Q,
        store: &S,
    ) -> Result<Option<&KeyValuePair<K, V>>>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        let idx = hashed_key.next(bit_width)?;

        if !self.bitfield.test_bit(idx) {
            return Ok(None);
        }

        let cindex = self.index_for_bit_pos(idx);
        let child = self.get_child(cindex);
        match child {
            Pointer::Link { cid, cache } => {
                if let Some(cached_node) = cache.get() {
                    // Link node is cached
                    cached_node
                        .get_value(hashed_key, bit_width, depth + 1, key, store)
                        .await
                } else {
                    let node = match store.load(cid).await {
                        Ok(node) => node,
                        Err(error) => {
                            #[cfg(not(feature = "ignore-dead-links"))]
                            return Err(error);

                            #[cfg(feature = "ignore-dead-links")]
                            continue;
                        }
                    };

                    // Intentionally ignoring error, cache will always be the same.
                    let cache_node = cache.get_or_init(async { node }).await;
                    cache_node
                        .get_value(hashed_key, bit_width, depth + 1, key, store)
                        .await
                }
            }
            Pointer::Dirty(n) => {
                n.get_value(hashed_key, bit_width, depth + 1, key, store)
                    .await
            }
            Pointer::Values(vals) => Ok(vals.iter().find(|kv| key.eq(kv.key().borrow()))),
        }
    }

    /// Internal method to modify values.
    #[allow(clippy::too_many_arguments)]
    #[cfg_attr(target_arch="wasm32", async_recursion(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_recursion)]
    async fn modify_value<S: Store>(
        &mut self,
        mut hashed_key: HashBits,
        bit_width: u32,
        depth: u64,
        key: K,
        value: V,
        store: &S,
        overwrite: bool,
    ) -> Result<(Option<V>, bool)>
    where
        V: PartialEq + TargetConditionalSendSync,
    {
        let idx = hashed_key.next(bit_width)?;

        // No existing values at this point.
        if !self.bitfield.test_bit(idx) {
            self.insert_child(idx, key, value);
            return Ok((None, true));
        }

        let cindex = self.index_for_bit_pos(idx);
        let child = self.get_child_mut(cindex);

        match child {
            Pointer::Link { cid, cache } => {
                cache
                    .get_or_try_init(async { store.load(cid).await })
                    .await?;
                let child_node = cache.get_mut().expect("filled line above");

                let (old, modified) = child_node
                    .modify_value(
                        hashed_key,
                        bit_width,
                        depth + 1,
                        key,
                        value,
                        store,
                        overwrite,
                    )
                    .await?;
                if modified {
                    *child = Pointer::Dirty(std::mem::take(child_node));
                }
                Ok((old, modified))
            }
            Pointer::Dirty(n) => Ok(n
                .modify_value(
                    hashed_key,
                    bit_width,
                    depth + 1,
                    key,
                    value,
                    store,
                    overwrite,
                )
                .await?),
            Pointer::Values(vals) => {
                // Update, if the key already exists.
                if let Some(i) = vals.iter().position(|p| p.key() == &key) {
                    if overwrite {
                        // If value changed, the parent nodes need to be marked as dirty.
                        // ! The assumption here is that `PartialEq` is implemented correctly,
                        // ! and that if that is true, the serialized bytes are equal.
                        // ! To be absolutely sure, can serialize each value and compare or
                        // ! refactor the Hamt to not be type safe and serialize on entry and
                        // ! exit. These both come at costs, and this isn't a concern.
                        let value_changed = vals[i].value() != &value;
                        return Ok((
                            Some(std::mem::replace(vals[i].value_mut(), value)),
                            value_changed,
                        ));
                    } else {
                        // Can't overwrite, return None and false that the Node was not modified.
                        return Ok((None, false));
                    }
                }

                // If the array is full, create a subshard and insert everything
                if vals.len() >= MAX_ARRAY_WIDTH {
                    let mut sub = Node::<K, V, H>::default();
                    let consumed = hashed_key.consumed;
                    let modified = sub
                        .modify_value(
                            hashed_key,
                            bit_width,
                            depth + 1,
                            key,
                            value,
                            store,
                            overwrite,
                        )
                        .await?;
                    let kvs = std::mem::take(vals);

                    for p in kvs.into_iter() {
                        let hash = H::hash(p.key());
                        let (key, value) = p.take();
                        sub.modify_value(
                            HashBits::new_at_index(hash, consumed),
                            bit_width,
                            depth + 1,
                            key,
                            value,
                            store,
                            overwrite,
                        )
                        .await?;
                    }

                    *child = Pointer::Dirty(Box::new(sub));

                    return Ok(modified);
                }

                // Otherwise insert the element into the array in order.
                let max = vals.len();
                let idx = vals.iter().position(|c| c.key() > &key).unwrap_or(max);

                let np = KeyValuePair::new(key, value);
                vals.insert(idx, np);

                Ok((None, true))
            }
        }
    }

    /// Internal method to delete entries.
    #[cfg_attr(target_arch="wasm32", async_recursion(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_recursion)]
    async fn rm_value<Q: ?Sized + TargetConditionalSendSync, S: Store>(
        &mut self,
        mut hashed_key: HashBits,
        bit_width: u32,
        depth: u64,
        key: &Q,
        store: &S,
    ) -> Result<Option<(K, V)>>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        let idx = hashed_key.next(bit_width)?;

        // No existing values at this point.
        if !self.bitfield.test_bit(idx) {
            return Ok(None);
        }

        let cindex = self.index_for_bit_pos(idx);
        let child = self.get_child_mut(cindex);

        match child {
            Pointer::Link { cid, cache } => {
                cache
                    .get_or_try_init(async { store.load(cid).await })
                    .await?;
                let child_node = cache.get_mut().expect("filled line above");

                let deleted = child_node
                    .rm_value(hashed_key, bit_width, depth + 1, key, store)
                    .await?;
                if deleted.is_some() {
                    *child = Pointer::Dirty(std::mem::take(child_node));
                    // Clean to retrieve canonical form
                    child.clean()?;
                }

                Ok(deleted)
            }
            Pointer::Dirty(n) => {
                // Delete value and return deleted value
                let deleted = n
                    .rm_value(hashed_key, bit_width, depth + 1, key, store)
                    .await?;

                // Clean to ensure canonical form
                child.clean()?;
                Ok(deleted)
            }
            Pointer::Values(vals) => {
                // Delete value
                for (i, p) in vals.iter().enumerate() {
                    if key.eq(p.key().borrow()) {
                        let old = if vals.len() == 1 {
                            if let Pointer::Values(new_v) = self.rm_child(cindex, idx) {
                                new_v.into_iter().next().unwrap()
                            } else {
                                unreachable!()
                            }
                        } else {
                            vals.remove(i)
                        };
                        let (key, value) = old.take();
                        return Ok(Some((key, value)));
                    }
                }

                Ok(None)
            }
        }
    }

    #[cfg_attr(target_arch="wasm32", async_recursion(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_recursion)]
    pub async fn flush<S: Store>(&mut self, store: &mut S) -> Result<()> {
        for pointer in &mut self.pointers {
            if let Pointer::Dirty(node) = pointer {
                // Flush cached sub node to clear it's cache
                node.flush(store).await?;

                // Put node in blockstore and retrieve Cid
                let node_bytes = node.try_into_dag_cbor()?;
                let cid = store.write_cbor(&node_bytes).await?;

                // Can keep the flushed node in link cache
                let cache = OnceCell::new_with(Some(std::mem::take(node)));

                // Replace cached node with Cid link
                *pointer = Pointer::Link { cid, cache };
            }
        }

        Ok(())
    }

    fn rm_child(&mut self, i: usize, idx: u32) -> Pointer<K, V, H> {
        self.bitfield.clear_bit(idx);
        self.pointers.remove(i)
    }

    fn insert_child(&mut self, idx: u32, key: K, value: V) {
        let i = self.index_for_bit_pos(idx);
        self.bitfield.set_bit(idx);
        self.pointers.insert(i, Pointer::from_key_value(key, value))
    }

    fn index_for_bit_pos(&self, bp: u32) -> usize {
        let mask = Bitfield::zero().set_bits_le(bp);
        assert_eq!(mask.count_ones(), bp as usize);
        mask.and(&self.bitfield).count_ones()
    }

    fn get_child_mut(&mut self, i: usize) -> &mut Pointer<K, V, H> {
        &mut self.pointers[i]
    }

    fn get_child(&self, i: usize) -> &Pointer<K, V, H> {
        &self.pointers[i]
    }
}
