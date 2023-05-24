use crate::native::workspace::Workspace;
use anyhow::{anyhow, Result};
use noosphere_sphere::{SphereSync, SyncRecovery};
use noosphere_storage::MemoryStore;

pub async fn sync(workspace: &Workspace) -> Result<()> {
    workspace.ensure_sphere_initialized()?;

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
        context.sync(SyncRecovery::None).await?;
    }

    info!("Sync complete, rendering updated workspace...");

    workspace.render().await?;

    info!("Done!");

    Ok(())
}
