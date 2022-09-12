use anyhow::Result;
use async_trait::async_trait;
use cid::Cid;
use noosphere::data::MemoIpld;
use noosphere_fs::SphereFs;
use noosphere_storage::interface::{DagCborStore, Store};
use std::{collections::BTreeMap, sync::Arc};
use subtext::{block::Block, primitive::Entity};
use tokio_stream::StreamExt;

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
        guest_sphere_fs: &SphereFs<S>,
        slashlink: &Slashlink,
    ) -> Result<Option<Transclude>> {
        let slug = match &slashlink.link {
            crate::slashlink::Link::Slug(slug) => slug,
            crate::slashlink::Link::None => todo!(),
        };

        match guest_sphere_fs.read(slug).await? {
            Some(file) => {
                let subtext_ast_stream =
                    subtext::stream::<Block<Entity>, _, _>(file.contents).await;

                tokio::pin!(subtext_ast_stream);

                let mut first_content_block = None;
                let mut leading_header_blocks = Vec::new();

                while let Some(Ok(block)) = subtext_ast_stream.next().await {
                    match block {
                        Block::Seperator(_) => continue,
                        header @ Block::Header(_) => leading_header_blocks.push(header),
                        any_other @ _ => {
                            first_content_block = Some(any_other);
                            break;
                        }
                    }
                }

                if let Some(content_block) = first_content_block {
                    match content_block {
                        Block::Header(_) => todo!(),
                        Block::Paragraph(_) => todo!(),
                        Block::Quote(_) => todo!(),
                        Block::List(_) => todo!(),
                        Block::Link(_) => todo!(),
                        Block::Seperator(_) => todo!(),
                    }
                }
            }
            None => {}
        }

        Ok(None)
    }
}
