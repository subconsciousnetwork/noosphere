use anyhow::{anyhow, Result};
use cid::Cid;
use libipld_cbor::DagCborCodec;
use noosphere_core::data::Header;
use noosphere_sphere::{HasMutableSphereContext, SphereContentWrite, SphereCursor};
use noosphere_storage::BlockStore;

use crate::native::{
    content::{Content, FileReference},
    workspace::Workspace,
};

/// TODO(#105): We may want to change this to take an optional list of paths to
/// consider, and allow the user to rely on their shell for glob filtering
pub async fn save(render_depth: Option<u32>, workspace: &Workspace) -> Result<()> {
    workspace.ensure_sphere_initialized()?;

    let mut db = workspace.db().await?;

    let sphere_context = workspace.sphere_context().await?;

    let mut has_unsaved_changes = sphere_context.has_unsaved_changes().await?;

    if let Some((content, content_changes, memory_store)) = Content::read_changes(workspace).await?
    {
        let content_entries = memory_store.entries.lock().await;

        for (cid_bytes, block) in content_entries.iter() {
            let cid = Cid::try_from(cid_bytes.as_slice())?;
            db.put_block(&cid, block).await?;
            db.put_links::<DagCborCodec>(&cid, block).await?;
        }

        let mut sphere_context = workspace.sphere_context().await?;

        for (slug, _) in content_changes
            .new
            .iter()
            .chain(content_changes.updated.iter())
        {
            if let Some(FileReference {
                cid,
                content_type,
                extension,
            }) = content.matched.get(slug)
            {
                info!("Saving '{slug}'...");

                let headers = extension
                    .as_ref()
                    .map(|extension| vec![(Header::FileExtension.to_string(), extension.clone())]);

                sphere_context
                    .link(slug, content_type, cid, headers)
                    .await?;
            }
        }

        for slug in content_changes.removed.keys() {
            info!("Removing '{slug}'...");
            sphere_context.remove(slug).await?;
        }

        has_unsaved_changes = true;
    }

    if has_unsaved_changes {
        let cid = SphereCursor::latest(sphere_context).save(None).await?;
        info!("Save complete!\nThe latest sphere revision is {cid}");

        workspace.render(render_depth, false).await?;

        Ok(())
    } else {
        Err(anyhow!("No changes to save"))
    }
}
