use std::ops::{Deref, DerefMut};

use anyhow::Result;
use noosphere_storage::interface::{StorageProvider, Store};

use crate::gateway::commands::BLOCK_STORE;

pub struct Blocks<Storage: Store>(pub Storage);

impl<Storage: Store> Blocks<Storage> {
    pub async fn from_storage_provider<Provider>(provider: &Provider) -> Result<Blocks<Storage>>
    where
        Provider: StorageProvider<Storage>,
    {
        Ok(Blocks(provider.get_store(BLOCK_STORE).await?))
    }

    pub fn into_store(self) -> Storage {
        self.0
    }
}

impl<Storage: Store> Deref for Blocks<Storage> {
    type Target = Storage;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<Storage: Store> DerefMut for Blocks<Storage> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
