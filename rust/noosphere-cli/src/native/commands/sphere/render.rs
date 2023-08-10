use crate::workspace::Workspace;
use anyhow::Result;

/// Render the workspace up to a specified maximum render depth
pub async fn render(render_depth: Option<u32>, workspace: &Workspace) -> Result<()> {
    workspace.render(render_depth, true).await?;
    Ok(())
}
