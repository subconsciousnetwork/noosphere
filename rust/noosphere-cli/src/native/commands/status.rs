use std::collections::BTreeMap;

use crate::native::Workspace;
use anyhow::Result;
use noosphere_core::data::ContentType;
use noosphere_storage::MemoryStore;

pub fn status_section(
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

pub async fn status(attr: String, workspace: &Workspace) -> Result<()> {
    workspace.ensure_sphere_initialized()?;

    let identity = workspace.sphere_identity().await?;

    if attr == "id" {
        info!("{identity}");
        return Ok(());
    } else if !attr.is_empty() {
        error!("Unknown attribute requested: {attr}")
    }


    info!("This sphere's identity is {identity}");
    info!("Here is a summary of the current changes to sphere content:\n");

    let mut memory_store = MemoryStore::default();

    let (_, changes) = match workspace
        .get_file_content_changes(&mut memory_store)
        .await?
    {
        Some((content, content_changes)) if !content_changes.is_empty() => {
            (content, content_changes)
        }
        _ => {
            info!("No new changes to sphere content!");
            return Ok(());
        }
    };

    let mut content = Vec::new();

    let mut max_name_length = 7usize;
    let mut max_content_type_length = 16usize;

    status_section(
        "Updated",
        &changes.updated,
        &mut content,
        &mut max_name_length,
        &mut max_content_type_length,
    );

    status_section(
        "New",
        &changes.new,
        &mut content,
        &mut max_name_length,
        &mut max_content_type_length,
    );

    status_section(
        "Removed",
        &changes.removed,
        &mut content,
        &mut max_name_length,
        &mut max_content_type_length,
    );

    info!(
        "{:max_name_length$}  {:max_content_type_length$}  STATUS",
        "NAME", "CONTENT-TYPE"
    );

    for (slug, content_type, status) in content {
        info!("{slug:max_name_length$}  {content_type:max_content_type_length$}  {status}");
    }

    Ok(())
}
