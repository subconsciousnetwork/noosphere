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

pub async fn status(workspace: &Workspace) -> Result<()> {
    let identity = workspace.sphere_identity().await?;

    println!("This sphere's identity is {identity}");
    println!("Here is a summary of the current changes to sphere content:\n");

    let mut memory_store = MemoryStore::default();

    let (_, mut changes) = match workspace
        .get_file_content_changes(&mut memory_store)
        .await?
    {
        Some((content, content_changes)) if !content_changes.is_empty() => {
            (content, content_changes)
        }
        _ => {
            println!("No new changes to sphere content!");
            return Ok(());
        }
    };

    let mut content = Vec::new();

    let mut max_name_length = 7usize;
    let mut max_content_type_length = 16usize;

    status_section(
        "Updated",
        &mut changes.updated,
        &mut content,
        &mut max_name_length,
        &mut max_content_type_length,
    );

    status_section(
        "New",
        &mut changes.new,
        &mut content,
        &mut max_name_length,
        &mut max_content_type_length,
    );

    status_section(
        "Removed",
        &mut changes.removed,
        &mut content,
        &mut max_name_length,
        &mut max_content_type_length,
    );

    println!(
        "{:max_name_length$}  {:max_content_type_length$}  STATUS",
        "NAME", "CONTENT-TYPE"
    );

    for (slug, content_type, status) in content {
        println!(
            "{:max_name_length$}  {:max_content_type_length$}  {}",
            slug, content_type, status
        );
    }

    Ok(())
}
