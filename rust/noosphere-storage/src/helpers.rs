use crate::interface::StorageProvider;
use anyhow::Result;

#[cfg(not(target_arch = "wasm32"))]
use crate::native::{NativeStorageInit, NativeStorageProvider, NativeStore};

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
    let provider = NativeStorageProvider::new(NativeStorageInit::Db(db))?;
    provider.get_store("foo").await
}

#[cfg(target_arch = "wasm32")]
use crate::web::{WebStorageProvider, WebStore};

#[cfg(target_arch = "wasm32")]
pub async fn make_disposable_store() -> Result<WebStore> {
    let temp_name: String = witty_phrase_generator::WPGen::new()
        .with_words(3)
        .unwrap()
        .into_iter()
        .map(|word| String::from(word))
        .collect();

    let provider = WebStorageProvider::new(1, &temp_name, &vec!["foo"]).await?;
    provider.get_store("foo").await
}
