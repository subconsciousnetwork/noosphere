use anyhow::Result;
use async_trait::async_trait;
use cid::Cid;
use noosphere::data::MemoIpld;
use noosphere_fs::SphereFs;
use noosphere_storage::interface::{DagCborStore, Store};
use std::{collections::BTreeMap, sync::Arc};

use tokio::sync::Mutex;

use crate::{
    slashlink::Slashlink,
    transclude::{Transclude, Transcluder},
};

pub struct HtmlSubtextTranscluder {
    cache: Arc<Mutex<BTreeMap<String, Transclude>>>,
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl Transcluder for HtmlSubtextTranscluder {
    async fn make_transclude<S: Store>(
        &self,
        _host_sphere: &Cid,
        _host_content: &Cid,
        guest_sphere: &Cid,
        link: &Slashlink,
        block_store: &S,
    ) -> Result<Transclude> {
        let fs = SphereFs::new()
        let guest_content_memo: MemoIpld = block_store.load(guest_content).await?;
    }
}
