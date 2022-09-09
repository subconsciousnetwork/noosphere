use anyhow::Result;
use async_trait::async_trait;
use cid::Cid;
use noosphere_storage::interface::Store;

use crate::slashlink::Slashlink;

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait Resolver {
    async fn resolve<S: Store>(
        &self,
        sphere_revision: &Cid,
        link: &Slashlink,
        block_store: &S,
    ) -> Result<Cid>;
}
