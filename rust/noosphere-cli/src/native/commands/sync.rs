use crate::native::{content::Content, workspace::Workspace};
use anyhow::{anyhow, Result};
use noosphere_sphere::{SphereSync, SyncRecovery};

pub async fn sync(auto_retry: u32, workspace: &Workspace) -> Result<()> {
    workspace.ensure_sphere_initialized()?;

    match Content::read_changes(workspace).await? {
        Some((_, content_changes, _)) if !content_changes.is_empty() => {
            return Err(anyhow!(
                "You have unsaved local changes; save or revert them before syncing!"
            ));
        }
        _ => (),
    };

    {
        let mut context = workspace.sphere_context().await?;
        context.sync(SyncRecovery::Retry(auto_retry)).await?;
    }

    info!("Sync complete, rendering updated workspace...");

    workspace.render().await?;

    info!("Done!");

    Ok(())
}
