use anyhow::{anyhow, Result};
use cid::Cid;
use globset::Glob;
use libipld_cbor::DagCborCodec;
use noosphere_core::{authority::Author, data::Header};
use noosphere_fs::SphereFs;
use noosphere_storage::{interface::BlockStore, memory::MemoryStore};

use crate::native::workspace::{FileReference, Workspace};

pub async fn save(matching: Option<Glob>, workspace: &Workspace) -> Result<()> {
    workspace.expect_local_directories()?;

    let mut memory_store = MemoryStore::default();
    let mut db = workspace.get_local_db().await?;

    let pattern = matching.map(|glob| glob.compile_matcher());

    let (content, content_changes) = match workspace
        .get_local_content_changes(pattern, &db, &mut memory_store)
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

    let sphere_did = workspace.get_local_identity().await?;
    let latest_sphere_cid = db.require_version(&sphere_did).await?;
    let author = Author {
        key: workspace.get_local_key().await?,
        authorization: Some(workspace.get_local_authorization().await?),
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
