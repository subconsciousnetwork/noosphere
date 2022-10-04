use std::ops::{Deref, DerefMut};

use anyhow::Result;
use noosphere_storage::{
    interface::{BlockStore, StorageProvider, Store},
    BLOCK_STORE,
};

#[derive(Clone)]
pub struct Blocks<S: Store>(pub S);

impl<S: Store> Blocks<S> {
    pub async fn from_storage_provider<Provider>(provider: &Provider) -> Result<Blocks<S>>
    where
        Provider: StorageProvider<S>,
    {
        Ok(Blocks(provider.get_store(BLOCK_STORE).await?))
    }

    pub fn into_store(self) -> impl BlockStore {
        self.0
    }
}

impl<S: Store> Deref for Blocks<S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<S: Store> DerefMut for Blocks<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
