use anyhow::{anyhow, Result};
use cid::Cid;
use globset::Glob;
use libipld_cbor::DagCborCodec;
use noosphere_fs::SphereFs;
use noosphere_storage::{interface::BlockStore, memory::MemoryStore};
use ucan::crypto::KeyMaterial;

use crate::native::workspace::Workspace;

pub async fn save(matching: Option<Glob>, workspace: &Workspace) -> Result<()> {
    workspace.expect_local_directories()?;

    let mut memory_store = MemoryStore::default();
    let mut db = workspace.get_local_db().await?;

    let pattern = matching.map(|glob| glob.compile_matcher());

    let (content, content_changes) = workspace
        .get_local_content_changes(pattern, &db, &mut memory_store)
        .await?;

    if content_changes.is_empty() {
        return Err(anyhow!("No changes to save"));
    }

    let content_entries = memory_store.entries.lock().await;

    for (cid_bytes, block) in content_entries.iter() {
        let cid = Cid::try_from(cid_bytes.as_slice())?;
        db.put_block(&cid, block).await?;
        db.put_links::<DagCborCodec>(&cid, block).await?;
    }

    let my_key = workspace.get_local_key().await?;
    let my_did = my_key.get_did().await?;
    let sphere_did = workspace.get_local_identity().await?;
    let latest_sphere_cid = db.require_version(&sphere_did).await?;
    let authorization = workspace.get_local_authorization().await?;

    let mut fs = SphereFs::at(&sphere_did, &latest_sphere_cid, Some(&my_did), &db);

    for (slug, _) in content_changes
        .new
        .iter()
        .chain(content_changes.updated.iter())
    {
        if let Some((content_type, cid)) = content.matched.get(slug) {
            println!("Updating {}...", slug);
            // TODO(#87): Note and preserve original file extension in a header
            fs.link(slug, &content_type.to_string(), cid, None).await?;
        }
    }

    for (slug, _) in &content_changes.removed {
        println!("Removing {}...", slug);
        fs.remove(slug).await?;
    }

    let cid = fs.save(&my_key, Some(&authorization), None).await?;

    println!("Save complete!\nThe latest sphere revision is {}", cid);
    Ok(())
}
