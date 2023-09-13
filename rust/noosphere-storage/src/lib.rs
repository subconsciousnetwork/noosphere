#![warn(missing_docs)]

//! This crate contains generic interfaces and concrete implementations to
//! support a common API for data persistance in Noosphere on many different
//! platforms. Current platforms include native targets (via disk-persisted K/V
//! store) and web browsers (via IndexedDB).

#[macro_use]
extern crate tracing;

mod block;
mod implementation;
mod key_value;

mod backup;
mod db;
mod encoding;
mod fs;
mod migration;
mod ops;
mod retry;
mod storage;
mod store;
mod tap;
mod temp;
mod ucan;

pub use crate::ucan::*;
pub use backup::*;
pub use block::*;
pub use db::*;
pub use encoding::*;
pub use fs::*;
pub use implementation::*;
pub use key_value::*;
pub use migration::*;
pub use ops::*;
pub use retry::*;
pub use storage::*;
pub use store::*;
pub use tap::*;
pub use temp::*;

mod space;
pub use space::*;

#[cfg(test)]
mod inner {
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "rocksdb")))]
    pub type PreferredPlatformStorage = crate::SledStorage;
    #[cfg(all(not(target_arch = "wasm32"), feature = "rocksdb"))]
    pub type PreferredPlatformStorage = crate::RocksDbStorage;
    #[cfg(target_arch = "wasm32")]
    pub type PreferredPlatformStorage = crate::IndexedDbStorage;
}
#[cfg(test)]
pub use inner::*;

#[cfg(test)]
mod tests {
    use crate::{block::BlockStore, PreferredPlatformStorage, Storage, TempStorage, BLOCK_STORE};

    use libipld_cbor::DagCborCodec;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_store_and_retrieve_bytes() -> anyhow::Result<()> {
        let storage = TempStorage::<PreferredPlatformStorage>::new().await?;
        let mut store = storage.get_block_store(BLOCK_STORE).await?;
        let bytes = b"I love every kind of cat";

        let cid = store.save::<DagCborCodec, _>(bytes).await?;
        let retrieved = store.load::<DagCborCodec, Vec<u8>>(&cid).await?;

        assert_eq!(retrieved, bytes);
        Ok(())
    }
}
