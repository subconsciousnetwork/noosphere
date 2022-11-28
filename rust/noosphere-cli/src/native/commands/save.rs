use anyhow::{anyhow, Result};
use cid::Cid;
use libipld_cbor::DagCborCodec;
use noosphere_core::{authority::Author, data::Header};
use noosphere_fs::SphereFs;
use noosphere_storage::{BlockStore, MemoryStore};

use crate::native::workspace::{FileReference, Workspace};

/// TODO(#105): We may want to change this to take an optional list of paths to
/// consider, and allow the user to rely on their shell for glob filtering
pub async fn save(workspace: &Workspace) -> Result<()> {
    let mut memory_store = MemoryStore::default();
    let mut db = workspace.db().await?;

    let (content, content_changes) = match workspace
        .get_file_content_changes(&mut memory_store)
        .await?
    {
        Some((content, content_changes)) if !content_changes.is_empty() => {
            (content, content_changes)
        }
        _ => {
            return Err(anyhow!("No changes to save"));
        }
    };

    let content_entries = memory_store.entries.lock().await;

    for (cid_bytes, block) in content_entries.iter() {
        let cid = Cid::try_from(cid_bytes.as_slice())?;
        db.put_block(&cid, block).await?;
        db.put_links::<DagCborCodec>(&cid, block).await?;
    }

    let sphere_did = workspace.sphere_identity().await?;
    let latest_sphere_cid = db.require_version(&sphere_did).await?;
    let author = Author {
        key: workspace.key().await?,
        authorization: Some(workspace.authorization().await?),
    };

    let mut fs = SphereFs::at(&sphere_did, &latest_sphere_cid, &author, &db).await?;

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
            println!("Saving {}...", slug);
            let headers = extension
                .as_ref()
                .map(|extension| vec![(Header::FileExtension.to_string(), extension.clone())]);

            fs.link(slug, &content_type.to_string(), cid, headers)
                .await?;
        }
    }

    for (slug, _) in &content_changes.removed {
        println!("Removing {}...", slug);
        fs.remove(slug).await?;
    }

    let cid = fs.save(None).await?;

    println!("Save complete!\nThe latest sphere revision is {}", cid);
    Ok(())
}
