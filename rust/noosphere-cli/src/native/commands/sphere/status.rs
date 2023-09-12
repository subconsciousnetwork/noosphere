use std::collections::BTreeMap;

use crate::native::{content::Content, Workspace};
use anyhow::Result;
use noosphere_core::context::{HasSphereContext, SphereCursor};
use noosphere_core::data::ContentType;

fn status_section(
    name: &str,
    entries: &BTreeMap<String, Option<ContentType>>,
    section: &mut Vec<(String, String, String)>,
    max_name_length: &mut usize,
    max_content_type_length: &mut usize,
) {
    for (slug, content_type) in entries {
        let content_type = content_type
            .as_ref()
            .map(|content_type| content_type.to_string())
            .unwrap_or_else(|| "Unknown".into());

        *max_name_length = *max_name_length.max(&mut slug.len());
        *max_content_type_length = *max_content_type_length.max(&mut content_type.len());

        section.push((slug.to_string(), content_type, String::from(name)));
    }
}

/// Get the current status of the workspace, reporting the content that has
/// changed in some way (if any)
pub async fn status(only_id: bool, workspace: &Workspace) -> Result<()> {
    workspace.ensure_sphere_initialized()?;

    let identity = workspace.sphere_identity().await?;

    if only_id {
        info!("{identity}");
        return Ok(());
    }

    info!("This sphere's identity is {identity}");

    let sphere_context = workspace.sphere_context().await?;
    let cid = SphereCursor::latest(sphere_context).version().await?;
    info!("The latest (saved) version of your sphere is {cid}\n");

    // TODO(#556): No need to pack new blocks into a memory store at this step;
    // maybe [Content::read_changes] can be optimized for this path
    let (_, content_changes, _) = match Content::read_changes(workspace).await? {
        Some(changes) => changes,
        None => {
            info!("No new changes to sphere content!");
            return Ok(());
        }
    };

    info!("Here is a summary of the current changes to sphere content:\n");

    let mut content = Vec::new();

    let mut max_name_length = 7usize;
    let mut max_content_type_length = 16usize;

    status_section(
        "Updated",
        &content_changes.updated,
        &mut content,
        &mut max_name_length,
        &mut max_content_type_length,
    );

    status_section(
        "New",
        &content_changes.new,
        &mut content,
        &mut max_name_length,
        &mut max_content_type_length,
    );

    status_section(
        "Removed",
        &content_changes.removed,
        &mut content,
        &mut max_name_length,
        &mut max_content_type_length,
    );

    if !content.is_empty() {
        info!(
            "{:max_name_length$}  {:max_content_type_length$}  STATUS",
            "NAME", "CONTENT-TYPE"
        );

        for (slug, content_type, status) in content {
            info!("{slug:max_name_length$}  {content_type:max_content_type_length$}  {status}");
        }
    } else {
        info!("No content has changed since the last save!")
    }

    Ok(())
}
