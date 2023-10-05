use crate::{storage::Storage, BlockStore, KeyValueStore, SPHERE_DB_STORE_NAMES};
use anyhow::{anyhow, ensure, Result};
use async_trait::async_trait;
use bytes::Bytes;
use cid::Cid;
use iroh::bytes::{
    baomap::{Map, MapEntry, Store},
    util::BlobFormat,
    Hash,
};
use iroh_io::AsyncSliceReaderExt;
use libipld_cbor::DagCborCodec;
use libipld_core::{
    codec::{Codec, Decode},
    ipld::Ipld,
    serde::{from_ipld, to_ipld},
};
use noosphere_common::ConditionalSend;
use redb::{Database, ReadableTable, TableDefinition};
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;
use std::{io::Cursor, path::PathBuf};

#[derive(Clone, Debug)]
pub struct IrohStorage {
    rt: iroh::bytes::util::runtime::Handle,
    path: PathBuf,
    db: Arc<Database>,
}

impl IrohStorage {
    pub fn new(path: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&path)?;
        let canonicalized = path.canonicalize()?;
        let rt = iroh::bytes::util::runtime::Handle::from_current(1)?;

        let redb_path = path.join("meta.redb");
        let db = if redb_path.exists() {
            Database::open(redb_path)?
        } else {
            Database::create(redb_path)?
        };

        Ok(IrohStorage {
            rt,
            path: canonicalized,
            db: Arc::new(db),
        })
    }
}

#[async_trait]
impl crate::Space for IrohStorage {
    async fn get_space_usage(&self) -> Result<u64> {
        todo!()
    }
}

#[async_trait]
impl Storage for IrohStorage {
    type BlockStore = IrohStore;
    type KeyValueStore = RedbStore;

    async fn get_block_store(&self, name: &str) -> Result<Self::BlockStore> {
        if SPHERE_DB_STORE_NAMES
            .iter()
            .find(|val| **val == name)
            .is_none()
        {
            return Err(anyhow!("No such store named {}", name));
        }

        IrohStore::new(&self.path, &self.rt, name).await
    }

    async fn get_key_value_store(&self, name: &str) -> Result<Self::KeyValueStore> {
        // Current implementation is to use a single DB for all stores, with each store
        // being represented by a single Table. If there is too much write contention
        // between the different kv stores, this would need to be replaced with having a
        // DB per store.

        if let Some(name) = SPHERE_DB_STORE_NAMES.iter().find(|val| **val == name) {
            let db = RedbStore::new(self.db.clone(), *name).await?;
            Ok(db)
        } else {
            Err(anyhow!("No such store named {}", name))
        }
    }
}

#[derive(Debug, Clone)]
pub struct IrohStore {
    db: iroh::baomap::flat::Store,
}

impl IrohStore {
    async fn new(
        root: &PathBuf,
        rt: &iroh::bytes::util::runtime::Handle,
        name: &str,
    ) -> Result<Self> {
        let complete_path = root.join(name).join("complete");
        let partial_path = root.join(name).join("partial");
        let meta_path = root.join(name).join("meta");

        std::fs::create_dir_all(&complete_path)?;
        std::fs::create_dir_all(&partial_path)?;
        std::fs::create_dir_all(&meta_path)?;

        let db =
            iroh::baomap::flat::Store::load(complete_path, partial_path, meta_path, rt).await?;

        Ok(Self { db })
    }
}

#[async_trait]
impl BlockStore for IrohStore {
    /// Given a block and its [Cid], persist the block in storage.
    async fn put_block(&mut self, cid: &Cid, block: &[u8]) -> Result<()> {
        let expected_hash = Hash::from_cid_bytes(&cid.to_bytes())?;
        let tag = self
            .db
            .import_bytes(Bytes::copy_from_slice(block), BlobFormat::RAW)
            .await?;

        ensure!(tag.hash() == &expected_hash, "hash missmatch");

        Ok(())
    }

    /// Given the [Cid] of a block, retrieve the block bytes from storage.
    async fn get_block(&self, cid: &Cid) -> Result<Option<Vec<u8>>> {
        let hash = Hash::from_cid_bytes(&cid.to_bytes())?;
        match self.db.get(&hash) {
            Some(entry) => {
                let mut reader = entry.data_reader().await?;
                let bytes = reader.read_to_end().await?;
                Ok(Some(bytes.to_vec()))
            }
            None => Ok(None),
        }
    }
}

#[derive(Clone)]
pub struct RedbStore {
    table: TableDefinition<'static, &'static [u8], &'static [u8]>,
    db: Arc<Database>,
}

impl RedbStore {
    async fn new(db: Arc<Database>, name: &'static str) -> Result<Self> {
        let table = TableDefinition::new(name);

        // Make sure the table exists
        let db0 = db.clone();
        tokio::task::spawn_blocking(move || {
            let write_tx = db0.begin_write()?;
            {
                let _table = write_tx.open_table(table)?;
            }
            write_tx.commit()?;
            anyhow::Ok(())
        })
        .await??;

        Ok(Self { table, db })
    }
}

#[async_trait]
impl KeyValueStore for RedbStore {
    async fn set_key<K, V>(&mut self, key: K, value: V) -> Result<()>
    where
        K: AsRef<[u8]> + ConditionalSend,
        V: Serialize + ConditionalSend,
    {
        let ipld = to_ipld(value)?;
        let codec = DagCborCodec;
        let cbor = codec.encode(&ipld)?;

        let db = self.db.clone();
        let key_bytes: Vec<u8> = K::as_ref(&key).to_vec(); // sad face
        let table = self.table;
        tokio::task::spawn_blocking(move || {
            let write_tx = db.begin_write()?;
            {
                let mut table = write_tx.open_table(table)?;
                table.insert(&key_bytes[..], &cbor[..])?;
            }
            write_tx.commit()?;
            anyhow::Ok(())
        })
        .await??;

        Ok(())
    }

    async fn get_key<K, V>(&self, key: K) -> Result<Option<V>>
    where
        K: AsRef<[u8]> + ConditionalSend,
        V: DeserializeOwned + ConditionalSend,
    {
        let db = self.db.clone();
        let key_bytes: Vec<u8> = K::as_ref(&key).to_vec(); // sad face
        let table = self.table;
        let res: Option<Ipld> = tokio::task::spawn_blocking(move || {
            let read_tx = db.begin_read()?;
            let table = read_tx.open_table(table)?;
            let maybe_guard = table.get(&key_bytes[..])?;
            match maybe_guard {
                Some(guard) => {
                    let value = Ipld::decode(DagCborCodec, &mut Cursor::new(guard.value()))?;
                    anyhow::Ok(Some(value))
                }
                None => Ok(None),
            }
        })
        .await??;

        let res = res.map(from_ipld).transpose()?;
        Ok(res)
    }

    async fn unset_key<K>(&mut self, key: K) -> Result<()>
    where
        K: AsRef<[u8]> + ConditionalSend,
    {
        let db = self.db.clone();
        let key_bytes: Vec<u8> = K::as_ref(&key).to_vec(); // sad face
        let table = self.table;
        tokio::task::spawn_blocking(move || {
            let write_tx = db.begin_write()?;
            {
                let mut table = write_tx.open_table(table)?;
                table.remove(&key_bytes[..])?;
            }
            write_tx.commit()?;
            anyhow::Ok(())
        })
        .await??;

        Ok(())
    }
}
