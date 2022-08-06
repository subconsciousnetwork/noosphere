use anyhow::{anyhow, Result};
use async_trait::async_trait;
use cid::{
    multihash::{Code, MultihashDigest},
    Cid,
};
use noosphere_cbor::{TryDagCbor, TryDagCborSendSync};

const DAG_CBOR_CODEC: u64 = 0x71;

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait StorageProvider<S: Store> {
    async fn get_store(&self, name: &str) -> Result<S>;
}

#[cfg(not(target_arch = "wasm32"))]
pub trait StoreConditionalSendSync: Send + Sync {}

#[cfg(target_arch = "wasm32")]
pub trait StoreConditionalSendSync {}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait Store: Clone + StoreConditionalSendSync {
    /// Read the bytes stored against a given key
    async fn read(&self, key: &[u8]) -> Result<Option<Vec<u8>>>;

    /// Writes bytes to local storage against a given key, and returns the previous
    /// value stored against that key if any
    async fn write(&mut self, key: &[u8], bytes: &[u8]) -> Result<Option<Vec<u8>>>;

    /// Returns true if the given key can be read from local storage
    async fn contains(&self, key: &[u8]) -> Result<bool>;

    /// Remove a value given a key, returning the removed value if any
    async fn remove(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>>;

    /// Flushes pending writes if there are any
    async fn flush(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait DagCborStore: Store {
    /// Saves a serializable type and returns the CID of the DAG-CBOR bytes
    async fn save<Type: TryDagCborSendSync>(&mut self, data: &Type) -> Result<Cid> {
        let cbor = data.try_into_dag_cbor()?;
        self.write_cbor(&cbor).await
    }

    /// Loads a deserializable type from storage
    async fn load<Type: TryDagCborSendSync>(&self, cid: &Cid) -> Result<Type> {
        let bytes = self.require_cbor(cid).await?;
        Type::try_from_dag_cbor(&bytes)
    }

    /// Helper to conditionally load an Option<Cid>
    async fn load_some<Type: TryDagCborSendSync>(
        &self,
        maybe_cid: Option<&Cid>,
    ) -> Result<Option<Type>> {
        Ok(match maybe_cid {
            Some(cid) => Some(self.load(cid).await?),
            None => None,
        })
    }

    /// Read optionally available DAG-CBOR bytes from the store given a CID
    async fn read_cbor(&self, cid: &Cid) -> Result<Option<Vec<u8>>> {
        Ok(self.read(&cid.to_bytes()).await?)
    }

    /// Read DAG-CBOR bytes, but return an error if none are found
    async fn require_cbor(&self, cid: &Cid) -> Result<Vec<u8>> {
        match self.read_cbor(cid).await? {
            Some(bytes) => Ok(bytes),
            None => Err(anyhow!("No bytes found for {:?}", cid)),
        }
    }

    /// Writes DAG-CBOR bytes to storage and returns the CID of those bytes
    async fn write_cbor(&mut self, bytes: &[u8]) -> Result<Cid> {
        let cid = Self::make_cid(bytes);
        trace!("Writing CBOR for {}", cid);
        if !self.contains_cbor(&cid).await? {
            self.write(&cid.to_bytes(), bytes).await?;
        }
        Ok(cid)
    }

    /// Returns true if bytes are stored against the given CID in local storage
    async fn contains_cbor(&self, cid: &Cid) -> Result<bool> {
        self.contains(&cid.to_bytes()).await
    }

    /// Removes a value from storage by CID and returns the removed value
    async fn remove_dag(&mut self, cid: &Cid) -> Result<Option<Vec<u8>>> {
        self.remove(&cid.to_bytes()).await
    }

    /// Helper to derive a CID from a byte slice (assumed to be encoded DAG-CBOR)
    fn make_cid(bytes: &[u8]) -> Cid {
        Cid::new_v1(DAG_CBOR_CODEC, Code::Blake2b256.digest(bytes))
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait KeyValueStore: Store {
    async fn set<V: TryDagCborSendSync>(&mut self, key: &str, value: &V) -> Result<()> {
        let bytes = value.try_into_dag_cbor()?;
        self.write(key.as_ref(), &bytes).await?;
        Ok(())
    }

    async fn get<V: TryDagCbor>(&self, key: &str) -> Result<Option<V>> {
        Ok(match self.read(key.as_ref()).await? {
            Some(bytes) => Some(V::try_from_dag_cbor(&bytes)?),
            None => None,
        })
    }
}

/// Blanket implementations for anything that implements Store
impl<S: Store> KeyValueStore for S {}

impl<S: Store> DagCborStore for S {}

impl<S: Store> StoreConditionalSendSync for S {}
