use crate::native::workspace::Workspace;
use anyhow::{anyhow, Result};
use noosphere_core::tracing::initialize_tracing;
use noosphere_sphere::SphereSync;
use noosphere_storage::MemoryStore;

pub async fn sync(workspace: &Workspace) -> Result<()> {
    initialize_tracing();

    let mut memory_store = MemoryStore::default();

    match workspace
        .get_file_content_changes(&mut memory_store)
        .await?
    {
        Some((_, content_changes)) if !content_changes.is_empty() => {
            return Err(anyhow!(
                "You have unsaved local changes; save or revert them before syncing!"
            ));
        }
        _ => (),
    };

    {
        let mut context = workspace.sphere_context().await?;
        context.sync().await?;
    }

    println!("Sync complete, rendering updated workspace...");

    workspace.render().await?;

    println!("Done!");

    Ok(())
}
