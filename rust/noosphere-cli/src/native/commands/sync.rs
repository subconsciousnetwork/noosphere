use crate::native::{commands::serve::tracing::initialize_tracing, workspace::Workspace};
use anyhow::{anyhow, Result};
use noosphere_storage::memory::MemoryStore;

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
        let context = workspace.sphere_context().await?;
        let mut context = context.lock().await;

        context.sync().await?;
    }

    println!("Sync complete, rendering updated workspace...");

    workspace.render().await?;

    println!("Done!");

    Ok(())
}
