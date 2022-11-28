use crate::Storage;
use anyhow::Result;

#[cfg(not(target_arch = "wasm32"))]
use crate::{NativeStorage, NativeStorageInit, NativeStore};

#[cfg(not(target_arch = "wasm32"))]
pub async fn make_disposable_store() -> Result<NativeStore> {
    let temp_dir = std::env::temp_dir();
    let temp_name: String = witty_phrase_generator::WPGen::new()
        .with_words(3)
        .unwrap()
        .into_iter()
        .map(String::from)
        .collect();
    let db = sled::open(temp_dir.join(temp_name)).unwrap();
    let provider = NativeStorage::new(NativeStorageInit::Db(db))?;
    provider.get_block_store("foo").await
}

#[cfg(target_arch = "wasm32")]
use crate::{WebStorage, WebStore};

#[cfg(target_arch = "wasm32")]
pub async fn make_disposable_store() -> Result<WebStore> {
    let temp_name: String = witty_phrase_generator::WPGen::new()
        .with_words(3)
        .unwrap()
        .into_iter()
        .map(|word| String::from(word))
        .collect();

    let provider = WebStorage::new(&temp_name).await?;
    provider.get_block_store(crate::db::BLOCK_STORE).await
}
