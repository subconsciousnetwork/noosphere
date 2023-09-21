use std::{io::Cursor, pin::Pin};

use crate::{block::BlockStore, key_value::KeyValueStore};
use anyhow::Result;
use async_trait::async_trait;
use cid::Cid;
use futures::Stream;
use libipld_cbor::DagCborCodec;
use libipld_core::{
    codec::{Codec, Decode},
    ipld::Ipld,
    serde::{from_ipld, to_ipld},
};
use noosphere_common::{ConditionalSend, ConditionalSync};
use serde::{de::DeserializeOwned, Serialize};

/// A primitive interface for storage backends. A storage backend does not
/// necessarily need to implement this trait to be used in Noosphere, but if it
/// does it automatically benefits from trait implementations for [BlockStore]
/// and [KeyValueStore], making a single [Store] implementation into a universal
/// backend.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait Store: Clone + ConditionalSync {
    /// Read the bytes stored against a given key
    async fn read(&self, key: &[u8]) -> Result<Option<Vec<u8>>>;

    /// Writes bytes to local storage against a given key, and returns the previous
    /// value stored against that key if any
    async fn write(&mut self, key: &[u8], bytes: &[u8]) -> Result<Option<Vec<u8>>>;

    /// Remove a value given a key, returning the removed value if any
    async fn remove(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>>;

    /// Flushes pending writes if there are any
    async fn flush(&self) -> Result<()> {
        Ok(())
    }
}

/// An async stream of key/value pairs from an [IterableStore].
#[cfg(not(target_arch = "wasm32"))]
pub type IterableStoreStream<'a> = dyn Stream<Item = Result<(Vec<u8>, Vec<u8>)>> + Send + 'a;
/// An async stream of key/value pairs from an [IterableStore].
#[cfg(target_arch = "wasm32")]
pub type IterableStoreStream<'a> = dyn Stream<Item = Result<(Vec<u8>, Vec<u8>)>> + 'a;

/// A store that can iterate over all of its entries.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait IterableStore {
    /// Retrieve all key/value pairs from this store as an async stream.
    fn get_all_entries(&self) -> Pin<Box<IterableStoreStream<'_>>>;
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S> BlockStore for S
where
    S: Store,
{
    async fn put_block(&mut self, cid: &Cid, block: &[u8]) -> Result<()> {
        self.write(&cid.to_bytes(), block).await?;
        Ok(())
    }

    async fn get_block(&self, cid: &Cid) -> Result<Option<Vec<u8>>> {
        self.read(&cid.to_bytes()).await
    }

    async fn flush(&self) -> Result<()> {
        Store::flush(self).await
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S> KeyValueStore for S
where
    S: Store,
{
    async fn set_key<K, V>(&mut self, key: K, value: V) -> Result<()>
    where
        K: AsRef<[u8]> + ConditionalSend,
        V: Serialize + ConditionalSend,
    {
        let ipld = to_ipld(value)?;
        let codec = DagCborCodec;
        let cbor = codec.encode(&ipld)?;
        let key_bytes = K::as_ref(&key);
        self.write(key_bytes, &cbor).await?;
        Ok(())
    }

    async fn unset_key<K>(&mut self, key: K) -> Result<()>
    where
        K: AsRef<[u8]> + ConditionalSend,
    {
        let key_bytes = K::as_ref(&key);
        self.remove(key_bytes).await?;
        Ok(())
    }

    async fn get_key<K, V>(&self, key: K) -> Result<Option<V>>
    where
        K: AsRef<[u8]> + ConditionalSend,
        V: DeserializeOwned + ConditionalSend,
    {
        let key_bytes = K::as_ref(&key);
        Ok(match self.read(key_bytes).await? {
            Some(bytes) => Some(from_ipld(Ipld::decode(
                DagCborCodec,
                &mut Cursor::new(bytes),
            )?)?),
            None => None,
        })
    }

    async fn flush(&self) -> Result<()> {
        Store::flush(self).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NonPersistentStorage, PreferredPlatformStorage, Storage, LINK_STORE};
    use std::collections::HashMap;
    use tokio_stream::StreamExt;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    pub async fn iterable_stores_get_all_entries() -> Result<()> {
        let storage = NonPersistentStorage::<PreferredPlatformStorage>::new().await?;
        let mut store = storage.get_key_value_store(LINK_STORE).await?;
        store.write(&[1], &[11]).await?;
        store.write(&[2], &[22]).await?;
        store.write(&[3], &[33]).await?;
        let mut stream = store.get_all_entries();

        let mut results = HashMap::new();
        while let Some((key, value)) = stream.try_next().await? {
            results.insert(key, value);
        }
        assert_eq!(results.len(), 3);
        assert_eq!(results.get(&vec![1]), Some(&vec![11u8]));
        assert_eq!(results.get(&vec![2]), Some(&vec![22u8]));
        assert_eq!(results.get(&vec![3]), Some(&vec![33u8]));
        Ok(())
    }
}
