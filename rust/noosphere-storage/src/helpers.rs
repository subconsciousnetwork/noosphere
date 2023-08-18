use crate::Storage;
use anyhow::Result;

#[cfg(not(target_arch = "wasm32"))]
use crate::{NativeStorage, NativeStorageInit, NativeStore};

#[cfg(not(target_arch = "wasm32"))]
pub async fn make_disposable_store() -> Result<NativeStore> {
    let provider = make_disposable_storage().await?;
    provider.get_block_store("foo").await
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn make_disposable_storage() -> Result<NativeStorage> {
    let temp_dir = std::env::temp_dir();
    let temp_name: String = witty_phrase_generator::WPGen::new()
        .with_words(3)
        .unwrap()
        .into_iter()
        .map(String::from)
        .collect();
    let db = sled::open(temp_dir.join(temp_name)).unwrap();
    NativeStorage::new(NativeStorageInit::Db(db))
}

#[cfg(target_arch = "wasm32")]
use crate::{WebStorage, WebStore};

#[cfg(target_arch = "wasm32")]
pub async fn make_disposable_store() -> Result<WebStore> {
    let provider = make_disposable_storage().await?;
    provider.get_block_store(crate::db::BLOCK_STORE).await
}

#[cfg(target_arch = "wasm32")]
pub async fn make_disposable_storage() -> Result<WebStorage> {
    let temp_name: String = witty_phrase_generator::WPGen::new()
        .with_words(3)
        .unwrap()
        .into_iter()
        .map(|word| String::from(word))
        .collect();

    WebStorage::new(&temp_name).await
}
