use std::ops::{Deref, DerefMut};

use anyhow::Result;
use noosphere_storage::interface::{StorageProvider, Store};

use crate::gateway::commands::BLOCK_STORE;

pub struct BlockStore<Storage: Store>(pub Storage);

impl<Storage: Store> BlockStore<Storage> {
    pub async fn from_storage_provider<Provider>(provider: &Provider) -> Result<BlockStore<Storage>>
    where
        Provider: StorageProvider<Storage>,
    {
        Ok(BlockStore(provider.get_store(BLOCK_STORE).await?))
    }
}

impl<Storage: Store> Deref for BlockStore<Storage> {
    type Target = Storage;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<Storage: Store> DerefMut for BlockStore<Storage> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
