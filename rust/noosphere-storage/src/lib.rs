//! This crate contains generic interfaces and concrete implementations to
//! support a common API for data persistance in Noosphere on many different
//! platforms. Current platforms include native targets (via disk-persisted K/V
//! store) and web browsers (via IndexedDB).

#[macro_use]
extern crate tracing;

mod block;
mod implementation;
mod key_value;

mod db;
mod encoding;
mod retry;
mod storage;
mod store;
mod tap;
mod ucan;

pub use crate::ucan::*;
pub use block::*;
pub use db::*;
pub use encoding::*;
pub use implementation::*;
pub use key_value::*;
pub use retry::*;
pub use storage::*;
pub use store::*;
pub use tap::*;

#[cfg(test)]
pub mod helpers;

#[cfg(test)]
mod tests {
    use crate::{block::BlockStore, helpers::make_disposable_store};

    use libipld_cbor::DagCborCodec;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_store_and_retrieve_bytes() {
        let mut storage = make_disposable_store().await.unwrap();
        let bytes = b"I love every kind of cat";

        let cid = storage.save::<DagCborCodec, _>(bytes).await.unwrap();
        let retrieved = storage.load::<DagCborCodec, Vec<u8>>(&cid).await.unwrap();

        assert_eq!(retrieved, bytes);
    }
}
