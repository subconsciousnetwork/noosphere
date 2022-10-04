#[macro_use]
extern crate tracing;

pub mod db;
pub mod interface;
pub mod memory;
pub mod tracking;
pub mod ucan;

pub const BLOCK_STORE: &str = "blocks";

#[cfg(not(target_arch = "wasm32"))]
pub mod native;
#[cfg(target_arch = "wasm32")]
pub mod web;

#[cfg(test)]
pub mod helpers;

#[cfg(test)]
mod tests {
    use crate::{helpers::make_disposable_store, interface::BlockStore};

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
