// Adapted for Noosphere from https://github.com/filecoin-project/ref-fvm
// Source copyright and license:
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::{anyhow, Result};
use cid::Cid;
use libipld_cbor::DagCborCodec;
use noosphere_storage::BlockStore;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::sync::OnceCell;

use super::TargetConditionalSendSync;

#[derive(Debug, Serialize, Deserialize, Eq, Clone)]
pub struct KeyValuePair<K, V> {
    key: K,
    link: Cid,

    #[serde(skip)]
    #[serde(default = "OnceCell::new")]
    value: OnceCell<V>,
}

impl<K, V> PartialEq for KeyValuePair<K, V>
where
    K: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self.link == other.link
    }
}

impl<K: TargetConditionalSendSync, V: TargetConditionalSendSync> KeyValuePair<K, V>
where
    V: Serialize + DeserializeOwned,
{
    pub fn key(&self) -> &K {
        &self.key
    }

    pub async fn get_value<S>(&self, store: &S) -> Result<&V>
    where
        S: BlockStore,
    {
        self.value
            .get_or_try_init(|| async { store.load::<DagCborCodec, V>(&self.link).await })
            .await
    }

    pub async fn overwrite_value<S>(&mut self, value: V, store: &mut S) -> Result<V>
    where
        S: BlockStore,
    {
        self.get_value(store).await?;
        self.link = store.save::<DagCborCodec, V>(value).await?;
        self.value
            .take()
            .ok_or_else(|| anyhow!("Expected previous value not found"))
    }

    pub async fn take<S>(mut self, store: &S) -> Result<(K, V)>
    where
        S: BlockStore,
    {
        self.get_value(store).await?;

        Ok((
            self.key,
            self.value
                .take()
                .ok_or_else(|| anyhow!("Failed to load value"))?,
        ))
    }

    pub async fn new<S>(key: K, value: V, store: &mut S) -> Result<Self>
    where
        S: BlockStore,
    {
        let link = store.save::<DagCborCodec, _>(&value).await?;

        Ok(KeyValuePair {
            key,
            link,
            value: OnceCell::new_with(Some(value)),
        })
    }
}
