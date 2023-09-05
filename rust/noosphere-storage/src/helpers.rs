use crate::Storage;
use anyhow::Result;

#[cfg(not(target_arch = "wasm32"))]
use crate::{SledStorage, SledStorageInit, SledStore};

#[cfg(not(target_arch = "wasm32"))]
pub async fn make_disposable_store() -> Result<SledStore> {
    let temp_dir = std::env::temp_dir();
    let temp_name: String = witty_phrase_generator::WPGen::new()
        .with_words(3)
        .unwrap()
        .into_iter()
        .map(String::from)
        .collect();
    let provider = SledStorage::new(SledStorageInit::Path(temp_dir.join(temp_name)))?;
    provider.get_block_store("foo").await
}

#[cfg(target_arch = "wasm32")]
use crate::{IndexedDbStorage, IndexedDbStore};

#[cfg(target_arch = "wasm32")]
pub async fn make_disposable_store() -> Result<IndexedDbStore> {
    let temp_name: String = witty_phrase_generator::WPGen::new()
        .with_words(3)
        .unwrap()
        .into_iter()
        .map(|word| String::from(word))
        .collect();

    let provider = IndexedDbStorage::new(&temp_name).await?;
    provider.get_block_store(crate::db::BLOCK_STORE).await
}
